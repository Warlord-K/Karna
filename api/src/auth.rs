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

/// Cookie names NextAuth uses (HTTPS vs HTTP).
const SECURE_COOKIE: &str = "__Secure-authjs.session-token";
const INSECURE_COOKIE: &str = "authjs.session-token";

/// Axum middleware that extracts the user ID from a NextAuth v5 JWE token.
///
/// NextAuth v5 with JWT strategy produces a JWE (dir + A256CBC-HS512).
/// The encryption key is derived via HKDF-SHA256 where the **salt** is the
/// cookie name and the **info** includes the salt:
///   `Auth.js Generated Encryption Key (<cookie_name>)`
pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_disabled = std::env::var("AUTH_DISABLED").unwrap_or_default() == "true";

    if auth_disabled {
        let uid = Uuid::parse_str(DEFAULT_USER_ID).unwrap();
        req.extensions_mut().insert(UserId(uid));
        return Ok(next.run(req).await);
    }

    // Extract token + the cookie name (needed as HKDF salt)
    let (token, salt) = extract_token(&req).ok_or(StatusCode::UNAUTHORIZED)?;

    let auth_secret =
        std::env::var("AUTH_SECRET").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let payload = decrypt_jwe(&token, &auth_secret, &salt).map_err(|e| {
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

/// Extract the JWE token and cookie name from the request.
/// NextAuth may chunk large tokens across multiple cookies (.0, .1, etc).
fn extract_token(req: &Request) -> Option<(String, String)> {
    // Try Authorization header first (salt defaults to secure cookie name)
    if let Some(bearer) = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        let decoded = urlencoding::decode(bearer).unwrap_or_else(|_| bearer.into());
        return Some((decoded.into_owned(), SECURE_COOKIE.to_string()));
    }

    // Parse cookies
    let cookie_header = req.headers().get("cookie")?.to_str().ok()?;
    let cookies: Vec<(&str, &str)> = cookie_header
        .split(';')
        .filter_map(|c| {
            let c = c.trim();
            let (name, val) = c.split_once('=')?;
            Some((name, val))
        })
        .collect();

    // Try exact cookie names first, then check for chunked cookies (.0, .1, ...)
    for base_name in &[SECURE_COOKIE, INSECURE_COOKIE] {
        // Direct match
        if let Some((_, val)) = cookies.iter().find(|(name, _)| *name == *base_name) {
            let decoded = urlencoding::decode(val).unwrap_or_else(|_| (*val).into());
            return Some((decoded.into_owned(), base_name.to_string()));
        }

        // Chunked: __Secure-authjs.session-token.0, .1, .2, ...
        let mut chunks: Vec<(usize, &str)> = Vec::new();
        for (name, val) in &cookies {
            if let Some(suffix) = name.strip_prefix(base_name) {
                if let Some(idx_str) = suffix.strip_prefix('.') {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        chunks.push((idx, val));
                    }
                }
            }
        }

        if !chunks.is_empty() {
            chunks.sort_by_key(|(idx, _)| *idx);
            let combined: String = chunks.into_iter().map(|(_, v)| v).collect();
            let decoded =
                urlencoding::decode(&combined).unwrap_or_else(|_| combined.clone().into());
            return Some((decoded.into_owned(), base_name.to_string()));
        }
    }

    None
}

/// Decrypt a NextAuth v5 JWE token (compact serialization).
///
/// Algorithm: dir + A256CBC-HS512 (RFC 7518 §5.2.5)
/// Key derivation: HKDF-SHA256(
///   ikm  = AUTH_SECRET,
///   salt = cookie_name,                                    // e.g. "__Secure-authjs.session-token"
///   info = "Auth.js Generated Encryption Key (<cookie_name>)",
///   len  = 64 bytes
/// )
fn decrypt_jwe(token: &str, secret: &str, salt: &str) -> anyhow::Result<Vec<u8>> {
    use aes::Aes256;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};
    use hmac::{Hmac, Mac};
    use sha2::Sha512;

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 5 {
        anyhow::bail!("Invalid JWE: expected 5 parts, got {}", parts.len());
    }

    let header_b64 = parts[0];
    // parts[1] = encrypted key (empty for "dir" algorithm)
    let iv_b64 = parts[2];
    let ciphertext_b64 = parts[3];
    let tag_b64 = parts[4];

    let iv = URL_SAFE_NO_PAD.decode(iv_b64)?;
    let ciphertext = URL_SAFE_NO_PAD.decode(ciphertext_b64)?;
    let tag = URL_SAFE_NO_PAD.decode(tag_b64)?;

    // Derive 64-byte key via HKDF-SHA256
    // salt = cookie name, info = "Auth.js Generated Encryption Key (<salt>)"
    let info = format!("Auth.js Generated Encryption Key ({salt})");
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(salt.as_bytes()), secret.as_bytes());
    let mut key = [0u8; 64];
    hk.expand(info.as_bytes(), &mut key)
        .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;

    // A256CBC-HS512: first 32 bytes = HMAC key, last 32 bytes = AES key
    let mac_key = &key[..32];
    let enc_key = &key[32..];

    // Verify HMAC-SHA-512 tag (RFC 7518 §5.2.2.1)
    // Input: AAD || IV || ciphertext || AL (AAD bit length as 64-bit BE)
    let aad = header_b64.as_bytes();
    let al = (aad.len() as u64 * 8).to_be_bytes();

    let mut hmac = Hmac::<Sha512>::new_from_slice(mac_key)
        .map_err(|_| anyhow::anyhow!("HMAC init failed"))?;
    hmac.update(aad);
    hmac.update(&iv);
    hmac.update(&ciphertext);
    hmac.update(&al);
    let mac_result = hmac.finalize().into_bytes();

    // Tag = first 32 bytes (T_LEN = 256 bits for HS512)
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
