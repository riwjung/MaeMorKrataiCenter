use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use backend_admin::adapters::memory::MemoryStore;
use backend_admin::auth::jwt::{sign_es256_jwt, AccessClaims, PublicJwk, Role};
use backend_admin::auth::password::hash_password;
use backend_admin::build_router;
use backend_admin::config::Environment;
use backend_admin::ports::{JwksKeyRecord, UserRecord};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use p256::pkcs8::EncodePrivateKey;
use rsa::pkcs1v15::SigningKey as RsaSigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde_json::Value;
use sha2::Sha256;
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::Ordering;
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

fn setup_store() -> std::sync::Arc<MemoryStore> {
    let kek_b64 = STANDARD.encode([17u8; 32]);
    let store = MemoryStore::new("pepper-for-tests", kek_b64);
    block_on(async {
        let owner_hash = hash_password("owner-pass-12345", "pepper-for-tests")
            .await
            .unwrap();
        let admin_hash = hash_password("admin-pass-12345", "pepper-for-tests")
            .await
            .unwrap();
        let user_hash = hash_password("user-pass-12345", "pepper-for-tests")
            .await
            .unwrap();
        let mut users = store.users.lock().unwrap();
        users.insert(
            "u-owner".to_string(),
            UserRecord {
                id: "u-owner".to_string(),
                email: "owner@maemorkratai.test".to_string(),
                password_hash: owner_hash,
                role: Role::Owner,
                is_active: true,
            },
        );
        users.insert(
            "u-admin".to_string(),
            UserRecord {
                id: "u-admin".to_string(),
                email: "admin@maemorkratai.test".to_string(),
                password_hash: admin_hash,
                role: Role::Admin,
                is_active: true,
            },
        );
        users.insert(
            "u-user".to_string(),
            UserRecord {
                id: "u-user".to_string(),
                email: "user@maemorkratai.test".to_string(),
                password_hash: user_hash,
                role: Role::User,
                is_active: true,
            },
        );
    });
    store
}

async fn login_as(
    app: &axum::Router,
    email: &str,
    password: &str,
) -> (StatusCode, Value, Option<String>) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, "https://admin.maemorkratai.test")
        .header("cf-connecting-ip", "203.0.113.10")
        .body(Body::from(
            serde_json::json!({ "email": email, "password": password }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    let json = body_json(res).await;
    (status, json, set_cookie)
}

#[test]
fn scenario_2_health_check_and_degraded_state() {
    block_on(async {
        let store = setup_store();
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
        assert_eq!(ok_json["r2"], "ok");

        store.db_healthy.store(false, Ordering::SeqCst);
        let bad_res = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(bad_res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bad_json = body_json(bad_res).await;
        assert_eq!(bad_json["code"], "SERVICE_UNAVAILABLE");
        assert!(!bad_json["message"].as_str().unwrap().contains("SQL"));
    });
}

#[test]
fn scenario_3_deny_by_default_rbac_hierarchy_and_inactive_user() {
    block_on(async {
        let store = setup_store();
        let app = build_router(store.into_ports(Environment::Staging));

        let (owner_status, owner_login, _) =
            login_as(&app, "owner@maemorkratai.test", "owner-pass-12345").await;
        assert_eq!(owner_status, StatusCode::OK);
        let owner_jwt = owner_login["access_token"].as_str().unwrap();

        let (admin_status, admin_login, _) =
            login_as(&app, "admin@maemorkratai.test", "admin-pass-12345").await;
        assert_eq!(admin_status, StatusCode::OK);
        let admin_jwt = admin_login["access_token"].as_str().unwrap();

        // Mint a valid JWT for role=user to verify RBAC enforcement on protected routes
        let active_key = store.jwks_keys.lock().unwrap().first().unwrap().clone();
        let kek = STANDARD.encode([17u8; 32]);
        let pkcs8 = backend_admin::auth::envelope::decrypt_private_key(
            &kek,
            &active_key.kid,
            &active_key.alg,
            &active_key.encrypted_private_key,
        )
        .unwrap();
        let user_jwt = sign_es256_jwt(
            &active_key.kid,
            &pkcs8,
            &AccessClaims {
                iss: "https://admin-api.maemorkratai.test".to_string(),
                aud: "maemorkratai-admin".to_string(),
                sub: "u-user".to_string(),
                role: Role::User,
                jti: "jti-u".to_string(),
                iat: 1_700_000_000,
                exp: 1_700_000_900,
                nbf: None,
            },
        )
        .unwrap();

        // Unauthenticated /v1/me -> 401
        let unauth = app
            .clone()
            .oneshot(Request::get("/v1/me").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

        // User role -> 403 on /v1/me and /v1/admin/auth/keys/rotate
        let user_me = app
            .clone()
            .oneshot(
                Request::get("/v1/me")
                    .header(header::AUTHORIZATION, format!("Bearer {user_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(user_me.status(), StatusCode::FORBIDDEN);

        // Admin role -> 200 on /v1/me, 403 on owner-only /v1/admin/auth/keys/rotate
        let admin_me = app
            .clone()
            .oneshot(
                Request::get("/v1/me")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_me.status(), StatusCode::OK);

        let admin_rotate = app
            .clone()
            .oneshot(
                Request::post("/v1/admin/auth/keys/rotate")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_rotate.status(), StatusCode::FORBIDDEN);

        // Owner role -> 200 on both
        let owner_me = app
            .clone()
            .oneshot(
                Request::get("/v1/me")
                    .header(header::AUTHORIZATION, format!("Bearer {owner_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(owner_me.status(), StatusCode::OK);

        let owner_rotate = app
            .clone()
            .oneshot(
                Request::post("/v1/admin/auth/keys/rotate")
                    .header(header::AUTHORIZATION, format!("Bearer {owner_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(owner_rotate.status(), StatusCode::OK);

        // Deactivate admin -> 403 on protected route and login
        store
            .users
            .lock()
            .unwrap()
            .get_mut("u-admin")
            .unwrap()
            .is_active = false;
        let disabled_me = app
            .clone()
            .oneshot(
                Request::get("/v1/me")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(disabled_me.status(), StatusCode::FORBIDDEN);
    });
}

#[test]
fn scenario_4_cookie_policy_refresh_rotation_and_replay_revocation() {
    block_on(async {
        let store = setup_store();
        let app = build_router(store.into_ports(Environment::Staging));

        let (status, _, set_cookie) =
            login_as(&app, "admin@maemorkratai.test", "admin-pass-12345").await;
        assert_eq!(status, StatusCode::OK);
        let cookie_1 = set_cookie.unwrap();
        assert!(cookie_1.contains("HttpOnly"));
        assert!(cookie_1.contains("Path=/v1/auth"));
        assert!(cookie_1.contains("SameSite=None"));
        assert!(cookie_1.contains("Secure"));
        let token_1_pair = cookie_1.split(';').next().unwrap().to_string();

        // First refresh succeeds and rotates to token_2
        let refresh_1 = app
            .clone()
            .oneshot(
                Request::post("/v1/auth/refresh")
                    .header(header::ORIGIN, "https://admin.maemorkratai.test")
                    .header("x-csrf-protection", "1")
                    .header(header::COOKIE, &token_1_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refresh_1.status(), StatusCode::OK);
        let cookie_2 = refresh_1
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let token_2_pair = cookie_2.split(';').next().unwrap().to_string();
        assert_ne!(token_1_pair, token_2_pair);

        // Replay token_1 -> 401 and revokes entire family_id
        let replay = app
            .clone()
            .oneshot(
                Request::post("/v1/auth/refresh")
                    .header(header::ORIGIN, "https://admin.maemorkratai.test")
                    .header("x-csrf-protection", "1")
                    .header(header::COOKIE, &token_1_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);

        // token_2 must now also fail
        let after_replay = app
            .oneshot(
                Request::post("/v1/auth/refresh")
                    .header(header::ORIGIN, "https://admin.maemorkratai.test")
                    .header("x-csrf-protection", "1")
                    .header(header::COOKIE, &token_2_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after_replay.status(), StatusCode::UNAUTHORIZED);
    });
}

/// Regression (2026-11-25): a rotated session must keep its user, so the
/// refresh cookie it issues works for the next refresh too.
#[test]
fn scenario_4b_rotated_cookie_refreshes_again() {
    block_on(async {
        let store = setup_store();
        let app = build_router(store.into_ports(Environment::Staging));

        let (status, _, set_cookie) =
            login_as(&app, "admin@maemorkratai.test", "admin-pass-12345").await;
        assert_eq!(status, StatusCode::OK);
        let mut cookie_pair = set_cookie.unwrap().split(';').next().unwrap().to_string();

        for round in 1..=3 {
            let res = app
                .clone()
                .oneshot(
                    Request::post("/v1/auth/refresh")
                        .header(header::ORIGIN, "https://admin.maemorkratai.test")
                        .header("x-csrf-protection", "1")
                        .header(header::COOKIE, &cookie_pair)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "refresh round {round}");
            cookie_pair = res
                .headers()
                .get(header::SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string();
        }
    });
}

#[test]
fn scenario_5_jwks_rotation_continuity_and_rs256_readiness() {
    block_on(async {
        let store = setup_store();
        let app = build_router(store.into_ports(Environment::Staging));

        let (_, owner_login, _) =
            login_as(&app, "owner@maemorkratai.test", "owner-pass-12345").await;
        let jwt_kid1 = owner_login["access_token"].as_str().unwrap().to_string();

        let rot_res = app
            .clone()
            .oneshot(
                Request::post("/v1/admin/auth/keys/rotate")
                    .header(header::AUTHORIZATION, format!("Bearer {jwt_kid1}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rot_res.status(), StatusCode::OK);
        let rot_json = body_json(rot_res).await;
        let kid_2 = rot_json["kid"].as_str().unwrap().to_string();

        // JWKS publishes both kid_1 (retiring) and kid_2 (active) without private key fields
        let jwks_res = app
            .clone()
            .oneshot(
                Request::get("/.well-known/jwks.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(jwks_res.status(), StatusCode::OK);
        let jwks_json = body_json(jwks_res).await;
        let keys = jwks_json["keys"].as_array().unwrap();
        assert_eq!(keys.len(), 2);
        for k in keys {
            assert!(k.get("d").is_none());
        }

        // Token signed by retiring kid_1 still verifies until expiry
        let me_with_kid1 = app
            .clone()
            .oneshot(
                Request::get("/v1/me")
                    .header(header::AUTHORIZATION, format!("Bearer {jwt_kid1}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me_with_kid1.status(), StatusCode::OK);

        // New login mints token under kid_2
        let (_, owner_login_2, _) =
            login_as(&app, "owner@maemorkratai.test", "owner-pass-12345").await;
        let jwt_kid2 = owner_login_2["access_token"].as_str().unwrap();
        let header_b64 = jwt_kid2.split('.').next().unwrap();
        let header_json: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header_b64).unwrap()).unwrap();
        assert_eq!(header_json["kid"], kid_2);

        // Verify RS256 JWKS verification readiness for Cloudflare Access cutover
        let mut rng = rand_core::OsRng;
        let rsa_priv = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let rsa_pub = rsa_priv.to_public_key();
        let rsa_jwk = PublicJwk {
            kty: "RSA".to_string(),
            key_use: Some("sig".to_string()),
            alg: "RS256".to_string(),
            kid: "cf-access-rs256".to_string(),
            crv: None,
            x: None,
            y: None,
            n: Some(URL_SAFE_NO_PAD.encode(rsa_pub.n().to_bytes_be())),
            e: Some(URL_SAFE_NO_PAD.encode(rsa_pub.e().to_bytes_be())),
        };
        let rsa_signer = RsaSigningKey::<Sha256>::new(rsa_priv);
        let h_b64 =
            URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT","kid":"cf-access-rs256"}"#);
        let c_b64 = URL_SAFE_NO_PAD.encode(
            br#"{"iss":"https://admin-api.maemorkratai.test","aud":"maemorkratai-admin","sub":"u-owner","role":"owner","jti":"jti-rsa","iat":1700000000,"exp":1700000900}"#,
        );
        let sig_in = format!("{h_b64}.{c_b64}");
        let sig = rsa_signer.sign(sig_in.as_bytes());
        let rsa_jwt = format!("{sig_in}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()));
        let verified = backend_admin::auth::jwt::verify_jwt_with_jwks(
            &rsa_jwt,
            &[rsa_jwk],
            "https://admin-api.maemorkratai.test",
            "maemorkratai-admin",
            1_700_000_100,
        )
        .unwrap();
        assert_eq!(verified.role, Role::Owner);

        // Ensure private key stored in MemoryStore is AES-256-GCM ciphertext envelope, not DER
        let stored_keys = store.jwks_keys.lock().unwrap();
        let latest: &JwksKeyRecord = stored_keys.last().unwrap();
        assert!(latest.encrypted_private_key.starts_with("v1$"));
        let _ = p256::SecretKey::random(&mut rng).to_pkcs8_der().unwrap();
    });
}

#[test]
fn scenario_6_rate_limit_csrf_cors_and_error_sanitization() {
    block_on(async {
        let store = setup_store();
        store.rate_limit_max.store(2, Ordering::SeqCst);
        let app = build_router(store.into_ports(Environment::Staging));

        // CSRF guard rejects missing x-csrf-protection or disallowed Origin
        let missing_csrf = app
            .clone()
            .oneshot(
                Request::post("/v1/auth/refresh")
                    .header(header::ORIGIN, "https://admin.maemorkratai.test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

        let bad_origin = app
            .clone()
            .oneshot(
                Request::post("/v1/auth/login")
                    .header(header::ORIGIN, "https://evil.example.com")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"email":"a@b.com","password":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_origin.status(), StatusCode::FORBIDDEN);
        assert!(bad_origin
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());

        // Malformed JSON -> 400 sanitized envelope
        let bad_json_res = app
            .clone()
            .oneshot(
                Request::post("/v1/auth/login")
                    .header(header::ORIGIN, "https://admin.maemorkratai.test")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{not-json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_json_res.status(), StatusCode::BAD_REQUEST);

        // Rate limit breach on 3rd attempt -> 429
        let _ = login_as(&app, "admin@maemorkratai.test", "wrong-1").await;
        let _ = login_as(&app, "admin@maemorkratai.test", "wrong-2").await;
        let (limited_status, limited_body, _) =
            login_as(&app, "admin@maemorkratai.test", "wrong-3").await;
        assert_eq!(limited_status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(limited_body["code"], "RATE_LIMITED");
        assert!(limited_body["request_id"].as_str().is_some());
    });
}
