use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderTokenClaims {
    pub book_id: String,
    pub exp: i64,
}

pub fn issue_token(secret: &str, book_id: &str, ttl_hours: i64) -> Result<String> {
    let claims = ReaderTokenClaims {
        book_id: book_id.to_string(),
        exp: (Utc::now() + Duration::hours(ttl_hours)).timestamp(),
    };
    let payload = serde_json::to_vec(&claims)?;
    let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
    let signature = sign(secret, encoded_payload.as_bytes())?;
    Ok(format!("{encoded_payload}.{signature}"))
}

pub fn verify_token(secret: &str, token: &str) -> Result<ReaderTokenClaims> {
    let (payload, signature) = token
        .split_once('.')
        .ok_or_else(|| anyhow!("invalid token format"))?;
    let expected = sign(secret, payload.as_bytes())?;
    if expected != signature {
        return Err(anyhow!("invalid token signature"));
    }
    let claims: ReaderTokenClaims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload)?)?;
    if claims.exp < Utc::now().timestamp() {
        return Err(anyhow!("reader token expired"));
    }
    Ok(claims)
}

fn sign(secret: &str, payload: &[u8]) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip() {
        let token = issue_token("secret", "book-1", 1).unwrap();
        let claims = verify_token("secret", &token).unwrap();
        assert_eq!(claims.book_id, "book-1");
    }
}
