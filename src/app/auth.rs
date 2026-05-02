use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    app::{errors::api_error, state::AppState},
    core::config::Config,
};

type HmacSha256 = Hmac<Sha256>;
const ACCESS_TOKEN_LIFETIME_HOURS: i64 = 8;
const REFRESH_TOKEN_LIFETIME_DAYS: i64 = 30;

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/web/session", get(web_session))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    access_token: String,
    access_token_expires_at: String,
    refresh_token: String,
    refresh_token_expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct WebSessionResponse {
    username: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    iat: i64,
    exp: i64,
    token_type: String,
    jti: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenType {
    Access,
    Refresh,
}

impl TokenType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Refresh => "refresh",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "access" => Some(Self::Access),
            "refresh" => Some(Self::Refresh),
            _ => None,
        }
    }
}

pub async fn login(State(state): State<AppState>, Json(payload): Json<LoginRequest>) -> Response {
    if payload.username != state.config.web_auth_username
        || payload.password != state.config.web_auth_password
    {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Invalid username or password.",
        );
    }

    let token_pair = issue_token_pair(&state.config, &payload.username)
        .expect("token issuance should succeed with a configured signing secret");

    (StatusCode::OK, Json(token_pair)).into_response()
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Response {
    let claims = verify_token(&state.config, &payload.refresh_token, TokenType::Refresh)
        .map_err(|error| unauthorized(error.code(), error.message()));

    let claims = match claims {
        Ok(claims) => claims,
        Err(response) => return response,
    };

    (
        StatusCode::OK,
        Json(
            issue_token_pair(&state.config, &claims.sub)
                .expect("token issuance should succeed with a configured signing secret"),
        ),
    )
        .into_response()
}

pub async fn web_session(auth: AuthenticatedOperator) -> Json<WebSessionResponse> {
    Json(WebSessionResponse {
        username: auth.username,
    })
}

pub struct AuthenticatedOperator {
    pub username: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedOperator {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts.headers.get(header::AUTHORIZATION))
            .ok_or_else(|| unauthorized("missing_token", "Authentication is required."))?;

        let claims = verify_token(&state.config, token, TokenType::Access)
            .map_err(|error| unauthorized(error.code(), error.message()))?;

        Ok(Self {
            username: claims.sub,
        })
    }
}

fn bearer_token(header_value: Option<&axum::http::HeaderValue>) -> Option<&str> {
    let header_value = header_value?.to_str().ok()?;
    header_value.strip_prefix("Bearer ")
}

fn issue_token_pair(config: &Config, username: &str) -> Result<LoginResponse, AuthError> {
    let access_token_expires_at = Utc::now() + Duration::hours(ACCESS_TOKEN_LIFETIME_HOURS);
    let refresh_token_expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_LIFETIME_DAYS);
    let access_token = issue_token(
        config,
        username,
        access_token_expires_at.timestamp(),
        TokenType::Access,
    )?;
    let refresh_token = issue_token(
        config,
        username,
        refresh_token_expires_at.timestamp(),
        TokenType::Refresh,
    )?;

    Ok(LoginResponse {
        access_token,
        access_token_expires_at: access_token_expires_at.to_rfc3339(),
        refresh_token,
        refresh_token_expires_at: refresh_token_expires_at.to_rfc3339(),
    })
}

fn issue_token(
    config: &Config,
    username: &str,
    exp: i64,
    token_type: TokenType,
) -> Result<String, AuthError> {
    let issued_at = Utc::now().timestamp();
    let claims = JwtClaims {
        sub: username.to_string(),
        iat: issued_at,
        exp,
        token_type: token_type.as_str().to_string(),
        jti: next_token_id(token_type),
    };

    encode_jwt(&config.jwt_signing_secret, &claims)
}

fn next_token_id(token_type: TokenType) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", token_type.as_str())
}

fn verify_token(
    config: &Config,
    token: &str,
    expected_type: TokenType,
) -> Result<JwtClaims, AuthError> {
    let claims = decode_jwt(&config.jwt_signing_secret, token)?;
    if claims.exp <= Utc::now().timestamp() {
        return Err(AuthError::ExpiredToken);
    }
    if TokenType::from_str(&claims.token_type) != Some(expected_type) {
        return Err(AuthError::InvalidToken);
    }
    Ok(claims)
}

fn encode_jwt(secret: &str, claims: &JwtClaims) -> Result<String, AuthError> {
    let header = serde_json::json!({
        "alg": "HS256",
        "typ": "JWT",
    });
    let header = serde_json::to_vec(&header).map_err(AuthError::Json)?;
    let claims = serde_json::to_vec(claims).map_err(AuthError::Json)?;
    let encoded_header = URL_SAFE_NO_PAD.encode(header);
    let encoded_claims = URL_SAFE_NO_PAD.encode(claims);
    let signing_input = format!("{encoded_header}.{encoded_claims}");
    let signature = sign(secret, signing_input.as_bytes())?;
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

fn decode_jwt(secret: &str, token: &str) -> Result<JwtClaims, AuthError> {
    let mut segments = token.split('.');
    let encoded_header = segments.next().ok_or(AuthError::InvalidToken)?;
    let encoded_claims = segments.next().ok_or(AuthError::InvalidToken)?;
    let encoded_signature = segments.next().ok_or(AuthError::InvalidToken)?;
    if segments.next().is_some() {
        return Err(AuthError::InvalidToken);
    }

    let signing_input = format!("{encoded_header}.{encoded_claims}");
    let signature = URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .map_err(|_| AuthError::InvalidToken)?;
    verify_signature(secret, signing_input.as_bytes(), &signature)?;

    let header_bytes = URL_SAFE_NO_PAD
        .decode(encoded_header)
        .map_err(|_| AuthError::InvalidToken)?;
    let header: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(AuthError::Json)?;
    if header.get("alg").and_then(|value| value.as_str()) != Some("HS256") {
        return Err(AuthError::InvalidToken);
    }

    let claims_bytes = URL_SAFE_NO_PAD
        .decode(encoded_claims)
        .map_err(|_| AuthError::InvalidToken)?;
    serde_json::from_slice(&claims_bytes).map_err(AuthError::Json)
}

fn sign(secret: &str, message: &[u8]) -> Result<Vec<u8>, AuthError> {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AuthError::InvalidSecret)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_signature(secret: &str, message: &[u8], signature: &[u8]) -> Result<(), AuthError> {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AuthError::InvalidSecret)?;
    mac.update(message);
    mac.verify_slice(signature)
        .map_err(|_| AuthError::InvalidToken)
}

fn unauthorized(code: &'static str, message: &'static str) -> Response {
    api_error(StatusCode::UNAUTHORIZED, code, message)
}

#[derive(Debug)]
pub enum AuthError {
    InvalidToken,
    ExpiredToken,
    InvalidSecret,
    Json(serde_json::Error),
}

impl AuthError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidToken | Self::Json(_) => "invalid_token",
            Self::ExpiredToken => "expired_token",
            Self::InvalidSecret => "invalid_secret",
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::InvalidToken | Self::Json(_) => "Authentication token is invalid.",
            Self::ExpiredToken => "Authentication token has expired.",
            Self::InvalidSecret => "Authentication is unavailable.",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToken => write!(f, "invalid token"),
            Self::ExpiredToken => write!(f, "expired token"),
            Self::InvalidSecret => write!(f, "invalid signing secret"),
            Self::Json(error) => write!(f, "jwt json error: {error}"),
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests {
    use std::{env, sync::MutexGuard};

    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::{
        app::{
            router::{health, metrics, ready},
            state::{SessionLaunchResult, SessionLauncher},
        },
        core::config::Config,
    };

    use super::*;

    fn env_lock() -> MutexGuard<'static, ()> {
        crate::core::config::test_env_lock()
    }

    fn clear_env() {
        for key in [
            "APP_ENV",
            "APP_HOST",
            "APP_PORT",
            "APP_BOOKS_ROOT",
            "APP_DATA_DIR",
            "FRONTEND_DIST_DIR",
            "FRONTEND_BASE_URL",
            "WEB_AUTH_USERNAME",
            "WEB_AUTH_PASSWORD",
            "JWT_SIGNING_SECRET",
        ] {
            unsafe { env::remove_var(key) };
        }
    }

    async fn test_router() -> Router {
        clear_env();
        unsafe {
            env::set_var("APP_ENV", "test");
            env::set_var("WEB_AUTH_USERNAME", "operator");
            env::set_var("WEB_AUTH_PASSWORD", "secret-password");
            env::set_var("JWT_SIGNING_SECRET", "jwt-test-secret");
        }

        let config = Config::from_env().unwrap();
        let state = AppState {
            config,
            repository: crate::storage::repository::Repository::load(std::path::Path::new(
                "target/test/data",
            ))
            .await
            .unwrap(),
            metrics: crate::app::metrics::Metrics::default(),
            conversation_locks: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            session_launcher: std::sync::Arc::new(NoopLauncher),
        };

        Router::new()
            .route("/api/healthz", get(health))
            .route("/api/readyz", get(ready))
            .route("/api/metrics", get(metrics))
            .nest("/api", routes())
            .with_state(state)
    }

    struct NoopLauncher;

    #[async_trait::async_trait]
    impl SessionLauncher for NoopLauncher {
        async fn launch(
            &self,
            _workspace: &std::path::Path,
            _title: &str,
            _initial_prompt: &str,
        ) -> anyhow::Result<SessionLaunchResult> {
            unreachable!("auth tests should not launch Codex sessions")
        }

        async fn resume(
            &self,
            _workspace: &std::path::Path,
            _session_id: &str,
            _prompt: &str,
        ) -> anyhow::Result<SessionLaunchResult> {
            unreachable!("auth tests should not resume Codex sessions")
        }
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn successful_login_returns_signed_jwt() {
        let _guard = env_lock();
        let router = test_router().await;

        let response = router
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"operator","password":"secret-password"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        let access_token = payload
            .get("access_token")
            .and_then(|value| value.as_str())
            .unwrap();
        let refresh_token = payload
            .get("refresh_token")
            .and_then(|value| value.as_str())
            .unwrap();
        assert_eq!(
            verify_token(
                &Config::from_env().unwrap(),
                access_token,
                TokenType::Access
            )
            .unwrap()
            .sub,
            "operator"
        );
        assert_eq!(
            verify_token(
                &Config::from_env().unwrap(),
                refresh_token,
                TokenType::Refresh
            )
            .unwrap()
            .sub,
            "operator"
        );
        assert!(
            payload
                .get("access_token_expires_at")
                .and_then(|value| value.as_str())
                .is_some()
        );
        assert!(
            payload
                .get("refresh_token_expires_at")
                .and_then(|value| value.as_str())
                .is_some()
        );
    }

    #[tokio::test]
    async fn invalid_credentials_return_unauthorized() {
        let _guard = env_lock();
        let router = test_router().await;

        let response = router
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"username":"operator","password":"wrong"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "invalid_credentials");
    }

    #[tokio::test]
    async fn missing_token_is_rejected() {
        let _guard = env_lock();
        let router = test_router().await;

        let response = router
            .oneshot(
                Request::get("/api/web/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "missing_token");
    }

    #[tokio::test]
    async fn invalid_token_is_rejected() {
        let _guard = env_lock();
        let router = test_router().await;

        let response = router
            .oneshot(
                Request::get("/api/web/session")
                    .header(header::AUTHORIZATION, "Bearer not-a-jwt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "invalid_token");
    }

    #[tokio::test]
    async fn refresh_returns_rotated_token_pair() {
        let _guard = env_lock();
        let router = test_router().await;

        let login_response = router
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"operator","password":"secret-password"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let login_payload = response_json(login_response).await;
        let refresh_token = login_payload["refresh_token"].as_str().unwrap();

        let refresh_response = router
            .oneshot(
                Request::post("/api/auth/refresh")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        r#"{{"refresh_token":"{refresh_token}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(refresh_response.status(), StatusCode::OK);
        let refresh_payload = response_json(refresh_response).await;
        let next_access_token = refresh_payload["access_token"].as_str().unwrap();
        let next_refresh_token = refresh_payload["refresh_token"].as_str().unwrap();
        assert_ne!(
            refresh_payload["access_token"].as_str().unwrap(),
            login_payload["access_token"].as_str().unwrap()
        );
        assert_ne!(next_refresh_token, refresh_token);
        assert_eq!(
            verify_token(
                &Config::from_env().unwrap(),
                next_access_token,
                TokenType::Access
            )
            .unwrap()
            .sub,
            "operator"
        );
        assert_eq!(
            verify_token(
                &Config::from_env().unwrap(),
                next_refresh_token,
                TokenType::Refresh
            )
            .unwrap()
            .sub,
            "operator"
        );
    }

    #[tokio::test]
    async fn access_token_cannot_be_used_for_refresh() {
        let _guard = env_lock();
        let router = test_router().await;

        let login_response = router
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"operator","password":"secret-password"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let login_payload = response_json(login_response).await;
        let access_token = login_payload["access_token"].as_str().unwrap();

        let response = router
            .oneshot(
                Request::post("/api/auth/refresh")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        r#"{{"refresh_token":"{access_token}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "invalid_token");
    }

    #[tokio::test]
    async fn expired_token_is_rejected() {
        let _guard = env_lock();
        let router = test_router().await;
        let config = Config::from_env().unwrap();
        let token = issue_token(
            &config,
            "operator",
            Utc::now().timestamp() - 1,
            TokenType::Access,
        )
        .unwrap();

        let response = router
            .oneshot(
                Request::get("/api/web/session")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "expired_token");
    }

    #[tokio::test]
    async fn login_token_allows_protected_follow_up_request() {
        let _guard = env_lock();
        let router = test_router().await;

        let login_response = router
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"operator","password":"secret-password"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let login_payload = response_json(login_response).await;
        let token = login_payload["access_token"].as_str().unwrap();

        let response = router
            .oneshot(
                Request::get("/api/web/session")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["username"], "operator");
    }
}
