use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

pub const READER_TOKEN_TTL_HOURS: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderTokenClaims {
    pub book_id: String,
    pub exp: i64,
}

#[derive(Debug, Error)]
pub enum ReaderTokenError {
    #[error("reader token expired")]
    Expired,
}

pub fn issue_token(secret: &str, book_id: &str, ttl_hours: i64) -> Result<String> {
    issue_token_at(secret, book_id, ttl_hours, Utc::now())
}

fn issue_token_at(
    secret: &str,
    book_id: &str,
    ttl_hours: i64,
    issued_at: DateTime<Utc>,
) -> Result<String> {
    let claims = ReaderTokenClaims {
        book_id: book_id.to_string(),
        exp: (issued_at + Duration::hours(ttl_hours)).timestamp(),
    };
    let payload = serde_json::to_vec(&claims)?;
    let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
    let signature = sign(secret, encoded_payload.as_bytes())?;
    Ok(format!("{encoded_payload}.{signature}"))
}

pub fn reader_url(frontend_base_url: &str, token: &str) -> String {
    format!(
        "{}/reader/{}",
        frontend_base_url.trim_end_matches('/'),
        token
    )
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
        return Err(ReaderTokenError::Expired.into());
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

    #[test]
    fn freshly_issued_one_hour_token_expires_exactly_one_hour_later() {
        let issued_at = Utc::now();
        let token = issue_token_at("secret", "book-1", 1, issued_at).unwrap();
        let claims = verify_token("secret", &token).unwrap();
        assert_eq!(claims.exp - issued_at.timestamp(), 60 * 60);
    }

    #[test]
    fn reader_url_uses_frontend_base_url() {
        assert_eq!(
            reader_url("https://books.example.com/", "signed-token"),
            "https://books.example.com/reader/signed-token"
        );
    }
}
