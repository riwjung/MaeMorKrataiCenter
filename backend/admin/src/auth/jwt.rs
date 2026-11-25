use crate::error::AppError;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature as P256Signature, SigningKey, VerifyingKey as P256VerifyingKey};
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use p256::{EncodedPoint, PublicKey as P256PublicKey};
use rand_core::OsRng;
use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaVerifyingKey};
use rsa::{BigUint, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User = 1,
    Admin = 2,
    Owner = 3,
}

impl Role {
    pub fn parse(raw: &str) -> Result<Self, AppError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "user" => Ok(Self::User),
            "admin" => Ok(Self::Admin),
            "owner" => Ok(Self::Owner),
            _ => Err(AppError::unauthorized("invalid role claim")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Owner => "owner",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtHeader {
    pub alg: String,
    pub typ: String,
    pub kid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub role: Role,
    pub jti: String,
    pub iat: u64,
    pub exp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>,
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

pub struct GeneratedKeypair {
    pub kid: String,
    pub alg: String,
    pub public_jwk: PublicJwk,
    pub pkcs8_private_der: Zeroizing<Vec<u8>>,
}

pub fn generate_es256_keypair() -> Result<GeneratedKeypair, AppError> {
    let signing_key = SigningKey::random(&mut OsRng);
    let pkcs8 = signing_key
        .to_pkcs8_der()
        .map_err(|_| AppError::internal("failed to encode ES256 private key"))?;
    let verifying_key = signing_key.verifying_key();
    let point = verifying_key.to_encoded_point(false);
    let x = point
        .x()
        .ok_or_else(|| AppError::internal("missing EC x coordinate"))?;
    let y = point
        .y()
        .ok_or_else(|| AppError::internal("missing EC y coordinate"))?;

    let kid = format!("es256-{}", Uuid::new_v4());
    let public_jwk = PublicJwk {
        kty: "EC".to_string(),
        key_use: Some("sig".to_string()),
        alg: "ES256".to_string(),
        kid: kid.clone(),
        crv: Some("P-256".to_string()),
        x: Some(URL_SAFE_NO_PAD.encode(x)),
        y: Some(URL_SAFE_NO_PAD.encode(y)),
        n: None,
        e: None,
    };

    Ok(GeneratedKeypair {
        kid,
        alg: "ES256".to_string(),
        public_jwk,
        pkcs8_private_der: Zeroizing::new(pkcs8.as_bytes().to_vec()),
    })
}

pub fn sign_es256_jwt(
    kid: &str,
    pkcs8_private_der: &[u8],
    claims: &AccessClaims,
) -> Result<String, AppError> {
    let signing_key = SigningKey::from_pkcs8_der(pkcs8_private_der)
        .map_err(|_| AppError::internal("failed to parse ES256 private key"))?;

    let header = JwtHeader {
        alg: "ES256".to_string(),
        typ: "JWT".to_string(),
        kid: kid.to_string(),
    };
    let header_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&header)
            .map_err(|_| AppError::internal("failed to encode JWT header"))?,
    );
    let claims_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(claims)
            .map_err(|_| AppError::internal("failed to encode JWT claims"))?,
    );
    let signing_input = format!("{header_b64}.{claims_b64}");

    let signature: P256Signature = signing_key.sign(signing_input.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
    Ok(format!("{signing_input}.{sig_b64}"))
}

pub fn verify_jwt_with_jwks(
    token: &str,
    jwks: &[PublicJwk],
    expected_iss: &str,
    expected_aud: &str,
    now_secs: u64,
) -> Result<AccessClaims, AppError> {
    let mut parts = token.split('.');
    let (Some(header_b64), Some(claims_b64), Some(sig_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(AppError::unauthorized("invalid token"));
    };

    let header_bytes = URL_SAFE_NO_PAD
        .decode(header_b64)
        .map_err(|_| AppError::unauthorized("invalid token"))?;
    let header: JwtHeader = serde_json::from_slice(&header_bytes)
        .map_err(|_| AppError::unauthorized("invalid token"))?;

    if header.alg != "ES256" && header.alg != "RS256" {
        return Err(AppError::unauthorized("invalid token"));
    }

    let jwk = jwks
        .iter()
        .find(|k| k.kid == header.kid)
        .ok_or_else(|| AppError::unauthorized("invalid token"))?;
    if jwk.alg != header.alg {
        return Err(AppError::unauthorized("invalid token"));
    }

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| AppError::unauthorized("invalid token"))?;
    let signing_input = format!("{header_b64}.{claims_b64}");

    match header.alg.as_str() {
        "ES256" => {
            if jwk.kty != "EC" || jwk.crv.as_deref() != Some("P-256") {
                return Err(AppError::unauthorized("invalid token"));
            }
            let x = URL_SAFE_NO_PAD
                .decode(
                    jwk.x
                        .as_deref()
                        .ok_or_else(|| AppError::unauthorized("invalid token"))?,
                )
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            let y = URL_SAFE_NO_PAD
                .decode(
                    jwk.y
                        .as_deref()
                        .ok_or_else(|| AppError::unauthorized("invalid token"))?,
                )
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            if x.len() != 32 || y.len() != 32 {
                return Err(AppError::unauthorized("invalid token"));
            }
            let mut sec1 = Vec::with_capacity(65);
            sec1.push(0x04);
            sec1.extend_from_slice(&x);
            sec1.extend_from_slice(&y);
            let point = EncodedPoint::from_bytes(&sec1)
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            let pub_key = Option::<P256PublicKey>::from(P256PublicKey::from_encoded_point(&point))
                .ok_or_else(|| AppError::unauthorized("invalid token"))?;
            let vk = P256VerifyingKey::from(&pub_key);
            let sig = P256Signature::from_slice(&sig_bytes)
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            vk.verify(signing_input.as_bytes(), &sig)
                .map_err(|_| AppError::unauthorized("invalid token"))?;
        }
        "RS256" => {
            if jwk.kty != "RSA" {
                return Err(AppError::unauthorized("invalid token"));
            }
            let n_bytes = URL_SAFE_NO_PAD
                .decode(
                    jwk.n
                        .as_deref()
                        .ok_or_else(|| AppError::unauthorized("invalid token"))?,
                )
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            let e_bytes = URL_SAFE_NO_PAD
                .decode(
                    jwk.e
                        .as_deref()
                        .ok_or_else(|| AppError::unauthorized("invalid token"))?,
                )
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            let rsa_pub = RsaPublicKey::new(
                BigUint::from_bytes_be(&n_bytes),
                BigUint::from_bytes_be(&e_bytes),
            )
            .map_err(|_| AppError::unauthorized("invalid token"))?;
            let vk = RsaVerifyingKey::<Sha256>::new(rsa_pub);
            let sig = RsaSignature::try_from(sig_bytes.as_slice())
                .map_err(|_| AppError::unauthorized("invalid token"))?;
            vk.verify(signing_input.as_bytes(), &sig)
                .map_err(|_| AppError::unauthorized("invalid token"))?;
        }
        _ => return Err(AppError::unauthorized("invalid token")),
    }

    let claims_bytes = URL_SAFE_NO_PAD
        .decode(claims_b64)
        .map_err(|_| AppError::unauthorized("invalid token"))?;
    let claims: AccessClaims = serde_json::from_slice(&claims_bytes)
        .map_err(|_| AppError::unauthorized("invalid token"))?;

    if claims.iss != expected_iss || claims.aud != expected_aud {
        return Err(AppError::unauthorized("invalid token"));
    }
    if claims.exp <= now_secs {
        return Err(AppError::unauthorized("invalid token"));
    }
    if let Some(nbf) = claims.nbf {
        if nbf > now_secs {
            return Err(AppError::unauthorized("invalid token"));
        }
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn es256_sign_and_verify_roundtrip() {
        let kp = generate_es256_keypair().unwrap();
        let claims = AccessClaims {
            iss: "https://issuer.test".to_string(),
            aud: "admin".to_string(),
            sub: "user-1".to_string(),
            role: Role::Admin,
            jti: "jti-1".to_string(),
            iat: 1000,
            exp: 1900,
            nbf: None,
        };
        let token = sign_es256_jwt(&kp.kid, &kp.pkcs8_private_der, &claims).unwrap();
        let verified = verify_jwt_with_jwks(
            &token,
            std::slice::from_ref(&kp.public_jwk),
            "https://issuer.test",
            "admin",
            1500,
        )
        .unwrap();
        assert_eq!(verified.sub, "user-1");
        assert_eq!(verified.role, Role::Admin);

        // Expired token rejected
        assert!(verify_jwt_with_jwks(
            &token,
            &[kp.public_jwk],
            "https://issuer.test",
            "admin",
            1900,
        )
        .is_err());
    }

    #[test]
    fn rejects_alg_none_and_hs256_confusion() {
        let kp = generate_es256_keypair().unwrap();
        let header = URL_SAFE_NO_PAD
            .encode(format!(r#"{{"alg":"none","typ":"JWT","kid":"{}"}}"#, kp.kid).as_bytes());
        let claims = URL_SAFE_NO_PAD.encode(
            br#"{"iss":"https://issuer.test","aud":"admin","sub":"u1","role":"owner","jti":"j1","iat":1,"exp":9999999999}"#,
        );
        let forged = format!("{header}.{claims}.");
        assert!(verify_jwt_with_jwks(
            &forged,
            &[kp.public_jwk],
            "https://issuer.test",
            "admin",
            100,
        )
        .is_err());
    }
}
