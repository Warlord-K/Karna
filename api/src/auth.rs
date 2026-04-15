use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use uuid::Uuid;

/// Key in request extensions — set by auth middleware.
#[derive(Clone, Debug)]
pub struct UserId(pub Uuid);

/// Default user ID when AUTH_DISABLED=true (matches frontend DEFAULT_USER_ID).
const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Axum middleware that extracts the user ID from a NextAuth v5 JWE token.
///
/// NextAuth v5 with JWT strategy produces a JWE (A256CBC-HS512, dir algorithm).
/// The encryption key is derived from AUTH_SECRET via HKDF-SHA256 with
/// info = "Auth.js Generated Encryption Key".
pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_disabled = std::env::var("AUTH_DISABLED").unwrap_or_default() == "true";

    if auth_disabled {
        let uid = Uuid::parse_str(DEFAULT_USER_ID).unwrap();
        req.extensions_mut().insert(UserId(uid));
        return Ok(next.run(req).await);
    }

    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            // Also check cookies for __Secure-authjs.session-token or authjs.session-token
            req.headers()
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .and_then(|cookies| {
                    for cookie in cookies.split(';') {
                        let cookie = cookie.trim();
                        for name in &[
                            "__Secure-authjs.session-token=",
                            "authjs.session-token=",
                        ] {
                            if let Some(val) = cookie.strip_prefix(name) {
                                return Some(val);
                            }
                        }
                    }
                    None
                })
        });

    let token = match token {
        Some(t) => t.to_string(),
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let auth_secret = std::env::var("AUTH_SECRET").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let payload = decrypt_jwe(&token, &auth_secret).map_err(|e| {
        tracing::warn!("JWT decrypt failed: {e}");
        StatusCode::UNAUTHORIZED
    })?;

    let claims: serde_json::Value =
        serde_json::from_slice(&payload).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id_str = claims
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let uid = Uuid::parse_str(user_id_str).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Check expiration
    if let Some(exp) = claims.get("exp").and_then(|v| v.as_i64()) {
        let now = chrono::Utc::now().timestamp();
        if now > exp {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    req.extensions_mut().insert(UserId(uid));
    Ok(next.run(req).await)
}

/// Decrypt a NextAuth v5 JWE token (compact serialization).
///
/// Algorithm: dir + A256CBC-HS512
/// Key derivation: HKDF-SHA256(ikm=secret, salt="", info="Auth.js Generated Encryption Key")
fn decrypt_jwe(token: &str, secret: &str) -> anyhow::Result<Vec<u8>> {
    use aes::Aes256;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};
    use hmac::{Hmac, Mac};
    use sha2::Sha512;

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 5 {
        anyhow::bail!("Invalid JWE: expected 5 parts, got {}", parts.len());
    }

    let _header_b64 = parts[0];
    // parts[1] = encrypted key (empty for "dir" algorithm)
    let iv_b64 = parts[2];
    let ciphertext_b64 = parts[3];
    let tag_b64 = parts[4];

    let iv = URL_SAFE_NO_PAD.decode(iv_b64)?;
    let ciphertext = URL_SAFE_NO_PAD.decode(ciphertext_b64)?;
    let tag = URL_SAFE_NO_PAD.decode(tag_b64)?;

    // Derive 64-byte key via HKDF
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b""), secret.as_bytes());
    let mut key = [0u8; 64];
    hk.expand(b"Auth.js Generated Encryption Key", &mut key)
        .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;

    // A256CBC-HS512: first 32 bytes = HMAC key, last 32 bytes = AES key
    let mac_key = &key[..32];
    let enc_key = &key[32..];

    // Verify HMAC tag
    // AAD = BASE64URL(header)
    let aad = _header_b64.as_bytes();
    let al = (aad.len() as u64 * 8).to_be_bytes(); // bit length

    let mut hmac = Hmac::<Sha512>::new_from_slice(mac_key)
        .map_err(|_| anyhow::anyhow!("HMAC init failed"))?;
    hmac.update(aad);
    hmac.update(&iv);
    hmac.update(&ciphertext);
    hmac.update(&al);
    let mac_result = hmac.finalize().into_bytes();

    // Tag is first 32 bytes of HMAC-SHA-512 output
    let computed_tag = &mac_result[..32];
    if !constant_time_eq(computed_tag, &tag) {
        anyhow::bail!("JWE tag verification failed");
    }

    // Decrypt AES-256-CBC
    type Aes256CbcDec = cbc::Decryptor<Aes256>;
    let decryptor = Aes256CbcDec::new_from_slices(enc_key, &iv)
        .map_err(|_| anyhow::anyhow!("AES init failed"))?;

    let mut buf = ciphertext;
    let plaintext = decryptor
        .decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
        .map_err(|_| anyhow::anyhow!("AES decrypt failed"))?;

    Ok(plaintext.to_vec())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
