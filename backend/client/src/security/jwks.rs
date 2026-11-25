use crate::error::ClientError;
use crate::middleware::security::RequestId;
use crate::ports::ClientPorts;
use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use axum::response::Response;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::{EncodedPoint, PublicKey as P256PublicKey};
use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaVerifyingKey};
use rsa::{BigUint, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
struct JwtHeader {
    alg: String,
    kid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicJwk {
    pub kty: String,
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub key_use: Option<String>,
    pub alg: String,
    pub kid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwksDocument {
    pub keys: Vec<PublicJwk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedJwtClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    #[serde(default)]
    pub role: Option<String>,
    pub jti: String,
    pub iat: u64,
    pub exp: u64,
    #[serde(default)]
    pub nbf: Option<u64>,
}

#[async_trait]
pub trait JwksSource: Send + Sync {
    async fn fetch_jwks(&self, jwks_url: &str) -> Result<Vec<PublicJwk>, ClientError>;
}

pub struct WorkerJwksSource;

#[cfg(target_arch = "wasm32")]
#[async_trait]
impl JwksSource for WorkerJwksSource {
    async fn fetch_jwks(&self, jwks_url: &str) -> Result<Vec<PublicJwk>, ClientError> {
        use worker::send::SendFuture;

        let url = worker::Url::parse(jwks_url)
            .map_err(|_| ClientError::service_unavailable("invalid JWKS URL"))?;
        let mut resp = SendFuture::new(worker::Fetch::Url(url).send())
            .await
            .map_err(|_| ClientError::service_unavailable("JWKS endpoint unreachable"))?;
        if resp.status_code() != 200 {
            return Err(ClientError::service_unavailable(
                "JWKS endpoint returned non-200 status",
            ));
        }
        let text = SendFuture::new(resp.text())
            .await
            .map_err(|_| ClientError::service_unavailable("JWKS body read failed"))?;
        let doc: JwksDocument = serde_json::from_str(&text)
            .map_err(|_| ClientError::service_unavailable("JWKS decode failed"))?;
        Ok(doc.keys)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl JwksSource for WorkerJwksSource {
    async fn fetch_jwks(&self, _jwks_url: &str) -> Result<Vec<PublicJwk>, ClientError> {
        Err(ClientError::service_unavailable(
            "WorkerJwksSource requires wasm32 runtime",
        ))
    }
}

#[derive(Clone)]
pub struct JwksVerifier {
    source: Arc<dyn JwksSource>,
}

impl JwksVerifier {
    pub fn new(source: Arc<dyn JwksSource>) -> Self {
        Self { source }
    }

    pub async fn verify_bearer_token(
        &self,
        ports: &ClientPorts,
        token: &str,
    ) -> Result<VerifiedJwtClaims, ClientError> {
        let keys = self
            .source
            .fetch_jwks(&ports.config.jwks_url)
            .await
            .map_err(|_| ClientError::unauthorized("Unable to verify token."))?;

        verify_jwt_against_keys(
            token,
            &keys,
            &ports.config.jwt_issuer,
            &ports.config.jwt_audience,
            ports.clock.now_secs(),
        )
    }
}

pub fn verify_jwt_against_keys(
    token: &str,
    jwks: &[PublicJwk],
    expected_iss: &str,
    expected_aud: &str,
    now_secs: u64,
) -> Result<VerifiedJwtClaims, ClientError> {
    let mut parts = token.split('.');
    let (Some(header_b64), Some(claims_b64), Some(sig_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ClientError::unauthorized("Invalid token."));
    };

    let header_bytes = URL_SAFE_NO_PAD
        .decode(header_b64)
        .map_err(|_| ClientError::unauthorized("Invalid token."))?;
    let header: JwtHeader = serde_json::from_slice(&header_bytes)
        .map_err(|_| ClientError::unauthorized("Invalid token."))?;

    if header.alg != "ES256" && header.alg != "RS256" {
        return Err(ClientError::unauthorized("Invalid token."));
    }

    let jwk = jwks
        .iter()
        .find(|k| k.kid == header.kid)
        .ok_or_else(|| ClientError::unauthorized("Invalid token."))?;
    if jwk.alg != header.alg {
        return Err(ClientError::unauthorized("Invalid token."));
    }

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| ClientError::unauthorized("Invalid token."))?;
    let signing_input = format!("{header_b64}.{claims_b64}");

    match header.alg.as_str() {
        "ES256" => {
            if jwk.kty != "EC" || jwk.crv.as_deref() != Some("P-256") {
                return Err(ClientError::unauthorized("Invalid token."));
            }
            let x = URL_SAFE_NO_PAD
                .decode(
                    jwk.x
                        .as_deref()
                        .ok_or_else(|| ClientError::unauthorized("Invalid token."))?,
                )
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            let y = URL_SAFE_NO_PAD
                .decode(
                    jwk.y
                        .as_deref()
                        .ok_or_else(|| ClientError::unauthorized("Invalid token."))?,
                )
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            if x.len() != 32 || y.len() != 32 {
                return Err(ClientError::unauthorized("Invalid token."));
            }
            let mut sec1 = Vec::with_capacity(65);
            sec1.push(0x04);
            sec1.extend_from_slice(&x);
            sec1.extend_from_slice(&y);
            let point = EncodedPoint::from_bytes(&sec1)
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            let pub_key = Option::<P256PublicKey>::from(P256PublicKey::from_encoded_point(&point))
                .ok_or_else(|| ClientError::unauthorized("Invalid token."))?;
            let vk = P256VerifyingKey::from(&pub_key);
            let sig = P256Signature::from_slice(&sig_bytes)
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            vk.verify(signing_input.as_bytes(), &sig)
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
        }
        "RS256" => {
            if jwk.kty != "RSA" {
                return Err(ClientError::unauthorized("Invalid token."));
            }
            let n_bytes = URL_SAFE_NO_PAD
                .decode(
                    jwk.n
                        .as_deref()
                        .ok_or_else(|| ClientError::unauthorized("Invalid token."))?,
                )
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            let e_bytes = URL_SAFE_NO_PAD
                .decode(
                    jwk.e
                        .as_deref()
                        .ok_or_else(|| ClientError::unauthorized("Invalid token."))?,
                )
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            let rsa_pub = RsaPublicKey::new(
                BigUint::from_bytes_be(&n_bytes),
                BigUint::from_bytes_be(&e_bytes),
            )
            .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            let vk = RsaVerifyingKey::<Sha256>::new(rsa_pub);
            let sig = RsaSignature::try_from(sig_bytes.as_slice())
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
            vk.verify(signing_input.as_bytes(), &sig)
                .map_err(|_| ClientError::unauthorized("Invalid token."))?;
        }
        _ => return Err(ClientError::unauthorized("Invalid token.")),
    }

    let claims_bytes = URL_SAFE_NO_PAD
        .decode(claims_b64)
        .map_err(|_| ClientError::unauthorized("Invalid token."))?;
    let claims: VerifiedJwtClaims = serde_json::from_slice(&claims_bytes)
        .map_err(|_| ClientError::unauthorized("Invalid token."))?;

    if claims.iss != expected_iss || claims.aud != expected_aud {
        return Err(ClientError::unauthorized("Invalid token."));
    }
    if claims.exp <= now_secs {
        return Err(ClientError::unauthorized("Invalid token."));
    }
    if let Some(nbf) = claims.nbf {
        if nbf > now_secs {
            return Err(ClientError::unauthorized("Invalid token."));
        }
    }
    Ok(claims)
}

#[derive(Clone)]
pub struct JwksRouterState {
    pub ports: ClientPorts,
    pub verifier: JwksVerifier,
}

impl FromRequestParts<JwksRouterState> for VerifiedJwtClaims {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &JwksRouterState,
    ) -> Result<Self, Self::Rejection> {
        let req_id = parts
            .extensions
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| "unassigned".to_string());

        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                ClientError::unauthorized("Authentication required.")
                    .into_response_with_request_id(&req_id)
            })?;

        state
            .verifier
            .verify_bearer_token(&state.ports, token)
            .await
            .map_err(|err| err.into_response_with_request_id(&req_id))
    }
}
