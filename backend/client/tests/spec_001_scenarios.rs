use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use backend_client::adapters::memory::MemoryClientStore;
use backend_client::build_router;
use backend_client::config::Environment;
use backend_client::error::ClientError;
use backend_client::security::jwks::{JwksSource, JwksVerifier, PublicJwk};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use p256::ecdsa::{Signature as P256Signature, SigningKey as P256SigningKey};
use rand_core::OsRng;
use rsa::pkcs1v15::SigningKey as RsaSigningKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde_json::Value;
use sha2::Sha256;
use signature::{SignatureEncoding, Signer};
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use tower::ServiceExt;

fn block_on<F: Future>(fut: F) -> F::Output {
    fn noop(_: *const ()) {}
    fn clone(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut pinned = pin!(fut);
    loop {
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn scenario_1_wrangler_has_zero_private_storage_and_zero_migrations() {
    let wrangler = include_str!("../wrangler.toml");
    let non_comment: String = wrangler
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!non_comment.contains("PRIVATE_STORAGE"));
    assert!(!non_comment.contains("migrations_dir"));
    assert!(non_comment.contains("PUBLIC_ASSETS"));
    assert!(non_comment.contains("PUBLIC_RATE_LIMITER"));
}

#[test]
fn scenario_2_health_check_and_degraded_d1_or_r2() {
    block_on(async {
        let store = MemoryClientStore::new();
        let app = build_router(store.into_ports(Environment::Staging));

        let ok_res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(ok_res.status(), StatusCode::OK);
        assert_eq!(
            ok_res
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap(),
            "nosniff"
        );
        let ok_json = body_json(ok_res).await;
        assert_eq!(ok_json["d1"], "ok");
        assert_eq!(ok_json["public_r2"], "ok");

        store.public_r2_healthy.store(false, Ordering::SeqCst);
        let bad_res = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(bad_res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bad_json = body_json(bad_res).await;
        assert_eq!(bad_json["code"], "SERVICE_UNAVAILABLE");
        assert!(!bad_json["message"]
            .as_str()
            .unwrap()
            .contains("PUBLIC_ASSETS"));
    });
}

#[test]
fn scenario_3_strict_cors_allowlist_and_rejection() {
    block_on(async {
        let store = MemoryClientStore::new();
        let app = build_router(store.into_ports(Environment::Staging));

        let allowed = app
            .clone()
            .oneshot(
                Request::get("/health")
                    .header(header::ORIGIN, "https://staging.maemorkratai.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);
        assert_eq!(
            allowed
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://staging.maemorkratai.com"
        );
        assert!(allowed
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .is_none());

        let disallowed_get = app
            .clone()
            .oneshot(
                Request::get("/health")
                    .header(header::ORIGIN, "https://evil.example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(disallowed_get
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());

        let disallowed_post = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/health")
                    .header(header::ORIGIN, "https://evil.example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(disallowed_post.status(), StatusCode::FORBIDDEN);
        assert!(disallowed_post
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    });
}

#[test]
fn scenario_4_rate_limit_body_limit_and_sanitized_errors() {
    block_on(async {
        let store = MemoryClientStore::new();
        store.rate_limit_max.store(2, Ordering::SeqCst);
        let app = build_router(store.into_ports(Environment::Staging));

        // Oversize body -> 413 sanitized envelope
        let oversized = vec![b'a'; 40_000];
        let too_big = app
            .clone()
            .oneshot(
                Request::post("/health")
                    .header(header::ORIGIN, "https://staging.maemorkratai.com")
                    .header("cf-connecting-ip", "198.51.100.5")
                    .body(Body::from(oversized))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(too_big.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let too_big_json = body_json(too_big).await;
        assert_eq!(too_big_json["code"], "PAYLOAD_TOO_LARGE");

        // Second request passes rate limit, third hits 429
        let second = app
            .clone()
            .oneshot(
                Request::get("/health")
                    .header("cf-connecting-ip", "198.51.100.5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);

        let third = app
            .oneshot(
                Request::get("/health")
                    .header("cf-connecting-ip", "198.51.100.5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(third.status(), StatusCode::TOO_MANY_REQUESTS);
        let limited_json = body_json(third).await;
        assert_eq!(limited_json["code"], "RATE_LIMITED");
        assert!(limited_json["request_id"].as_str().is_some());
    });
}

struct StaticJwksSource {
    keys: Vec<PublicJwk>,
}

#[async_trait]
impl JwksSource for StaticJwksSource {
    async fn fetch_jwks(&self, _jwks_url: &str) -> Result<Vec<PublicJwk>, ClientError> {
        Ok(self.keys.clone())
    }
}

#[test]
fn scenario_5_jwks_verifier_supports_es256_and_rs256_and_fails_closed() {
    block_on(async {
        let store = MemoryClientStore::new();
        let ports = store.into_ports(Environment::Staging);

        // ES256 fixture
        let ec_sk = P256SigningKey::random(&mut OsRng);
        let ec_vk = ec_sk.verifying_key();
        let pt = ec_vk.to_encoded_point(false);
        let ec_jwk = PublicJwk {
            kty: "EC".to_string(),
            key_use: Some("sig".to_string()),
            alg: "ES256".to_string(),
            kid: "ec-1".to_string(),
            crv: Some("P-256".to_string()),
            x: Some(URL_SAFE_NO_PAD.encode(pt.x().unwrap())),
            y: Some(URL_SAFE_NO_PAD.encode(pt.y().unwrap())),
            n: None,
            e: None,
        };
        let ec_header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256","typ":"JWT","kid":"ec-1"}"#);
        let valid_claims = URL_SAFE_NO_PAD.encode(
            br#"{"iss":"https://admin-api-staging.maemorkratai.com","aud":"maemorkratai-client","sub":"cust-1","jti":"j1","iat":1700000000,"exp":1700000900}"#,
        );
        let ec_input = format!("{ec_header}.{valid_claims}");
        let ec_sig: P256Signature = ec_sk.sign(ec_input.as_bytes());
        let ec_jwt = format!("{ec_input}.{}", URL_SAFE_NO_PAD.encode(ec_sig.to_bytes()));

        // RS256 fixture
        let rsa_sk = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let rsa_pk = rsa_sk.to_public_key();
        let rsa_jwk = PublicJwk {
            kty: "RSA".to_string(),
            key_use: Some("sig".to_string()),
            alg: "RS256".to_string(),
            kid: "rsa-1".to_string(),
            crv: None,
            x: None,
            y: None,
            n: Some(URL_SAFE_NO_PAD.encode(rsa_pk.n().to_bytes_be())),
            e: Some(URL_SAFE_NO_PAD.encode(rsa_pk.e().to_bytes_be())),
        };
        let rsa_signer = RsaSigningKey::<Sha256>::new(rsa_sk);
        let rsa_header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT","kid":"rsa-1"}"#);
        let rsa_input = format!("{rsa_header}.{valid_claims}");
        let rsa_sig = rsa_signer.sign(rsa_input.as_bytes());
        let rsa_jwt = format!("{rsa_input}.{}", URL_SAFE_NO_PAD.encode(rsa_sig.to_bytes()));

        let verifier = JwksVerifier::new(Arc::new(StaticJwksSource {
            keys: vec![ec_jwk, rsa_jwk],
        }));

        let ec_verified = verifier.verify_bearer_token(&ports, &ec_jwt).await.unwrap();
        assert_eq!(ec_verified.sub, "cust-1");

        let rsa_verified = verifier
            .verify_bearer_token(&ports, &rsa_jwt)
            .await
            .unwrap();
        assert_eq!(rsa_verified.sub, "cust-1");

        // Expired token -> 401
        let exp_claims = URL_SAFE_NO_PAD.encode(
            br#"{"iss":"https://admin-api-staging.maemorkratai.com","aud":"maemorkratai-client","sub":"cust-1","jti":"j2","iat":1699999000,"exp":1699999999}"#,
        );
        let exp_input = format!("{ec_header}.{exp_claims}");
        let exp_sig: P256Signature = ec_sk.sign(exp_input.as_bytes());
        let exp_jwt = format!("{exp_input}.{}", URL_SAFE_NO_PAD.encode(exp_sig.to_bytes()));
        let exp_err = verifier
            .verify_bearer_token(&ports, &exp_jwt)
            .await
            .unwrap_err();
        assert_eq!(exp_err.status, StatusCode::UNAUTHORIZED);

        // Algorithm confusion (alg: none) -> 401
        let none_header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT","kid":"ec-1"}"#);
        let forged = format!("{none_header}.{valid_claims}.");
        let forged_err = verifier
            .verify_bearer_token(&ports, &forged)
            .await
            .unwrap_err();
        assert_eq!(forged_err.status, StatusCode::UNAUTHORIZED);
    });
}
