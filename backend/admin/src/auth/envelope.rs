use crate::error::AppError;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const ENVELOPE_PREFIX: &str = "v1";

fn decode_kek(kek_b64: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, AppError> {
    let raw = STANDARD
        .decode(kek_b64.trim())
        .map_err(|_| AppError::internal("invalid KEY_ENCRYPTION_KEY base64 encoding"))?;
    if raw.len() != KEY_LEN {
        return Err(AppError::internal("KEY_ENCRYPTION_KEY must be 32 bytes"));
    }
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&raw);
    Ok(key)
}

fn build_aad(kid: &str, alg: &str) -> Vec<u8> {
    format!("{kid}|{alg}").into_bytes()
}

pub fn encrypt_private_key(
    kek_b64: &str,
    kid: &str,
    alg: &str,
    plaintext_pkcs8_der: &[u8],
) -> Result<String, AppError> {
    let key_bytes = decode_kek(kek_b64)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key_bytes.as_ref()));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let aad = build_aad(kid, alg);

    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext_pkcs8_der,
                aad: &aad,
            },
        )
        .map_err(|_| AppError::internal("failed to encrypt signing key"))?;

    Ok(format!(
        "{ENVELOPE_PREFIX}${}${}",
        STANDARD.encode(nonce_bytes),
        STANDARD.encode(ciphertext)
    ))
}

pub fn decrypt_private_key(
    kek_b64: &str,
    kid: &str,
    alg: &str,
    envelope: &str,
) -> Result<Zeroizing<Vec<u8>>, AppError> {
    let parts: Vec<&str> = envelope.split('$').collect();
    if parts.len() != 3 || parts[0] != ENVELOPE_PREFIX {
        return Err(AppError::internal(
            "unsupported encrypted key envelope format",
        ));
    }

    let nonce_bytes = STANDARD
        .decode(parts[1])
        .map_err(|_| AppError::internal("invalid key envelope nonce"))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(AppError::internal("invalid key envelope nonce length"));
    }

    let ciphertext = STANDARD
        .decode(parts[2])
        .map_err(|_| AppError::internal("invalid key envelope ciphertext"))?;

    let key_bytes = decode_kek(kek_b64)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key_bytes.as_ref()));
    let aad = build_aad(kid, alg);

    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| AppError::internal("KEY_ENCRYPTION_KEY decryption failed"))?;

    Ok(Zeroizing::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip_and_aad_binding() {
        let kek = STANDARD.encode([42u8; 32]);
        let secret = b"pkcs8-private-key-material";
        let sealed = encrypt_private_key(&kek, "kid-1", "ES256", secret).unwrap();

        let opened = decrypt_private_key(&kek, "kid-1", "ES256", &sealed).unwrap();
        assert_eq!(opened.as_slice(), secret);

        // Wrong kid or alg fails AEAD tag check
        assert!(decrypt_private_key(&kek, "kid-2", "ES256", &sealed).is_err());
        assert!(decrypt_private_key(&kek, "kid-1", "RS256", &sealed).is_err());

        // Wrong KEK fails AEAD tag check
        let wrong_kek = STANDARD.encode([99u8; 32]);
        assert!(decrypt_private_key(&wrong_kek, "kid-1", "ES256", &sealed).is_err());
    }
}
