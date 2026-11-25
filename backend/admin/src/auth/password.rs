use crate::error::AppError;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

pub const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const DERIVED_KEY_LEN: usize = 32;
const PREFIX: &str = "pbkdf2-sha256-peppered";

fn compute_peppered_secret(password: &str, pepper: &str) -> Result<Zeroizing<Vec<u8>>, AppError> {
    if pepper.is_empty() {
        return Err(AppError::internal("PASSWORD_PEPPER is empty"));
    }
    let mut mac = HmacSha256::new_from_slice(pepper.as_bytes())
        .map_err(|_| AppError::internal("invalid PASSWORD_PEPPER"))?;
    mac.update(password.as_bytes());
    Ok(Zeroizing::new(mac.finalize().into_bytes().to_vec()))
}

#[cfg(not(target_arch = "wasm32"))]
async fn derive_pbkdf2_sha256(
    peppered: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<[u8; DERIVED_KEY_LEN], AppError> {
    let mut derived = [0u8; DERIVED_KEY_LEN];
    pbkdf2::pbkdf2_hmac::<Sha256>(peppered, salt, iterations, &mut derived);
    Ok(derived)
}

#[cfg(target_arch = "wasm32")]
async fn derive_pbkdf2_sha256(
    peppered: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<[u8; DERIVED_KEY_LEN], AppError> {
    worker::send::SendFuture::new(async move {
        use js_sys::{Array, Object, Reflect, Uint8Array};
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

        let global: web_sys::WorkerGlobalScope = js_sys::global()
            .dyn_into()
            .map_err(|_| AppError::internal("failed to acquire WorkerGlobalScope"))?;
        let subtle = global
            .crypto()
            .map_err(|_| AppError::internal("WebCrypto unavailable"))?
            .subtle();

        let key_bytes = Uint8Array::from(peppered);
        let usages = Array::of1(&JsValue::from_str("deriveBits"));
        let key_promise = subtle
            .import_key_with_str("raw", key_bytes.as_ref(), "PBKDF2", false, usages.as_ref())
            .map_err(|_| AppError::internal("WebCrypto importKey failed"))?;
        let crypto_key: web_sys::CryptoKey = JsFuture::from(key_promise)
            .await
            .map_err(|_| AppError::internal("WebCrypto importKey promise rejected"))?
            .dyn_into()
            .map_err(|_| AppError::internal("WebCrypto returned invalid CryptoKey"))?;

        let salt_arr = Uint8Array::from(salt);
        let params = Object::new();
        Reflect::set(
            &params,
            &JsValue::from_str("name"),
            &JsValue::from_str("PBKDF2"),
        )
        .map_err(|_| AppError::internal("failed to build Pbkdf2Params"))?;
        Reflect::set(
            &params,
            &JsValue::from_str("hash"),
            &JsValue::from_str("SHA-256"),
        )
        .map_err(|_| AppError::internal("failed to set hash"))?;
        Reflect::set(
            &params,
            &JsValue::from_str("iterations"),
            &JsValue::from_f64(f64::from(iterations)),
        )
        .map_err(|_| AppError::internal("failed to set iterations"))?;
        Reflect::set(&params, &JsValue::from_str("salt"), salt_arr.as_ref())
            .map_err(|_| AppError::internal("failed to set salt"))?;

        let bits_promise = subtle
            .derive_bits_with_object(&params, &crypto_key, (DERIVED_KEY_LEN * 8) as u32)
            .map_err(|_| AppError::internal("WebCrypto deriveBits failed"))?;
        let bits_buf = JsFuture::from(bits_promise)
            .await
            .map_err(|_| AppError::internal("WebCrypto deriveBits rejected"))?;

        let out_arr = Uint8Array::new(&bits_buf);
        if out_arr.length() as usize != DERIVED_KEY_LEN {
            return Err(AppError::internal("unexpected derived bit length"));
        }
        let mut out = [0u8; DERIVED_KEY_LEN];
        out_arr.copy_to(&mut out);
        Ok(out)
    })
    .await
}

pub async fn hash_password(password: &str, pepper: &str) -> Result<String, AppError> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let peppered = compute_peppered_secret(password, pepper)?;
    let derived = derive_pbkdf2_sha256(&peppered, &salt, PBKDF2_ITERATIONS).await?;
    Ok(format!(
        "${PREFIX}$i={PBKDF2_ITERATIONS}${}${}",
        STANDARD_NO_PAD.encode(salt),
        STANDARD_NO_PAD.encode(derived)
    ))
}

pub async fn verify_password(
    password: &str,
    stored_hash: &str,
    pepper: &str,
) -> Result<bool, AppError> {
    let parts: Vec<&str> = stored_hash.split('$').collect();
    if parts.len() != 5 || !parts[0].is_empty() || parts[1] != PREFIX {
        return Err(AppError::internal(
            "unsupported stored password hash format",
        ));
    }

    let iter_str = parts[2]
        .strip_prefix("i=")
        .ok_or_else(|| AppError::internal("invalid iteration field in hash"))?;
    let iterations: u32 = iter_str
        .parse()
        .map_err(|_| AppError::internal("invalid iteration count"))?;
    if iterations != PBKDF2_ITERATIONS {
        return Err(AppError::internal("unsupported PBKDF2 iteration count"));
    }

    let salt = STANDARD_NO_PAD
        .decode(parts[3])
        .map_err(|_| AppError::internal("invalid base64 salt"))?;
    let expected = STANDARD_NO_PAD
        .decode(parts[4])
        .map_err(|_| AppError::internal("invalid base64 digest"))?;
    if expected.len() != DERIVED_KEY_LEN {
        return Err(AppError::internal("unexpected stored digest length"));
    }

    let peppered = compute_peppered_secret(password, pepper)?;
    let actual = derive_pbkdf2_sha256(&peppered, &salt, iterations).await?;
    Ok(bool::from(actual.ct_eq(expected.as_slice())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_cli_vector_match() {
        tokio_less_block_on(async {
            let pepper = "dev-pepper-secret";
            let pw = "correct-horse-battery-staple";
            let hashed = hash_password(pw, pepper).await.unwrap();
            assert!(verify_password(pw, &hashed, pepper).await.unwrap());
            assert!(!verify_password("wrong-password", &hashed, pepper)
                .await
                .unwrap());
            assert!(!verify_password(pw, &hashed, "wrong-pepper").await.unwrap());
        });
    }

    fn tokio_less_block_on<F: std::future::Future>(fut: F) -> F::Output {
        use std::pin::pin;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone(ptr: *const ()) -> RawWaker {
            RawWaker::new(ptr, &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut pinned = pin!(fut);
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(val) => val,
            Poll::Pending => panic!("host pbkdf2 future must resolve synchronously"),
        }
    }
}
