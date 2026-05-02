use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::{auth::AuthenticatedOperator, errors::api_error, state::AppState},
    core::models::BookLanguage,
    storage::web_books::{
        BookWorkspace, BookWorkspaceError, list_book_workspaces, provision_book_workspace,
    },
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/books", get(list_books).post(create_book))
}

#[derive(Debug, Deserialize)]
pub struct CreateBookRequest {
    title: String,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebBookResponse {
    pub book_id: String,
    pub slug: String,
    pub title: String,
    pub subtitle: String,
    pub language: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list_books(_auth: AuthenticatedOperator, State(state): State<AppState>) -> Response {
    match list_book_workspaces(&state.config.books_root) {
        Ok(books) => Json(
            books
                .into_iter()
                .map(WebBookResponse::from)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "book_list_failed",
            "Failed to list books.",
        ),
    }
}

pub async fn create_book(
    _auth: AuthenticatedOperator,
    State(state): State<AppState>,
    Json(payload): Json<CreateBookRequest>,
) -> Response {
    let language = match payload.language.as_deref() {
        None => BookLanguage::Russian,
        Some(value) => match BookLanguage::parse(value) {
            Some(language) => language,
            None => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_language",
                    "Language must be en or ru.",
                );
            }
        },
    };

    match provision_book_workspace(&state.config.books_root, &payload.title, language) {
        Ok(book) => (StatusCode::CREATED, Json(WebBookResponse::from(book))).into_response(),
        Err(BookWorkspaceError::InvalidTitle | BookWorkspaceError::InvalidSlug) => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_title",
            "Book title must contain letters or numbers.",
        ),
        Err(BookWorkspaceError::DuplicateSlug { slug }) => api_error(
            StatusCode::CONFLICT,
            "duplicate_book_slug",
            format!("A book workspace already exists for slug `{slug}`."),
        ),
        Err(BookWorkspaceError::PathEscape) => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_book_path",
            "Book title resolved to an invalid workspace path.",
        ),
        Err(BookWorkspaceError::Io(_))
        | Err(BookWorkspaceError::Yaml(_))
        | Err(BookWorkspaceError::Other(_)) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "book_create_failed",
            "Failed to create book.",
        ),
    }
}

impl From<BookWorkspace> for WebBookResponse {
    fn from(value: BookWorkspace) -> Self {
        Self {
            book_id: value.book_id,
            slug: value.slug,
            title: value.title,
            subtitle: value.subtitle,
            language: value.language.code().to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, sync::MutexGuard};

    use axum::{
        body::Body,
        http::{Request, header},
    };
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::{
        app::{
            auth,
            metrics::Metrics,
            state::{SessionLaunchResult, SessionLauncher},
        },
        core::config::{Config, test_env_lock},
        storage::web_books::{ConversationRegistry, read_conversation_registry},
    };

    use super::*;

    fn env_lock() -> MutexGuard<'static, ()> {
        test_env_lock()
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

    fn configure_env(temp_dir: &TempDir) {
        clear_env();
        unsafe {
            env::set_var("APP_ENV", "test");
            env::set_var("APP_BOOKS_ROOT", temp_dir.path().join("books-data"));
            env::set_var("APP_DATA_DIR", temp_dir.path().join("data"));
            env::set_var("WEB_AUTH_USERNAME", "operator");
            env::set_var("WEB_AUTH_PASSWORD", "secret-password");
            env::set_var("JWT_SIGNING_SECRET", "jwt-test-secret");
        }
    }

    async fn test_router(temp_dir: &TempDir) -> Router {
        configure_env(temp_dir);
        let config = Config::from_env().unwrap();
        config.ensure_directories().unwrap();
        let repository = crate::storage::repository::Repository::load(&config.data_dir)
            .await
            .unwrap();
        let state = AppState {
            config,
            repository,
            metrics: Metrics::default(),
            conversation_locks: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            session_launcher: std::sync::Arc::new(NoopLauncher),
        };

        Router::new()
            .nest("/api", auth::routes())
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
            unreachable!("book tests should not launch Codex sessions")
        }

        async fn resume(
            &self,
            _workspace: &std::path::Path,
            _session_id: &str,
            _prompt: &str,
        ) -> anyhow::Result<SessionLaunchResult> {
            unreachable!("book tests should not resume Codex sessions")
        }
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn login_token(router: Router) -> String {
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
        let payload = response_json(response).await;
        payload["access_token"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn books_endpoints_require_authentication() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(&temp_dir).await;

        let get_response = router
            .clone()
            .oneshot(Request::get("/api/books").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(get_response.status(), StatusCode::UNAUTHORIZED);

        let post_response = router
            .oneshot(
                Request::post("/api/books")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"Quiet Lighthouse"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(post_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_book_bootstraps_workspace_and_empty_registry() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(&temp_dir).await;
        let token = login_token(router.clone()).await;

        let response = router
            .clone()
            .oneshot(
                Request::post("/api/books")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"Quiet Lighthouse"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = response_json(response).await;
        assert_eq!(payload["book_id"], "quiet-lighthouse");
        assert_eq!(payload["slug"], "quiet-lighthouse");
        assert_eq!(payload["language"], "ru");

        let workspace = temp_dir.path().join("books-data/quiet-lighthouse");
        assert!(workspace.starts_with(temp_dir.path().join("books-data")));
        assert!(workspace.join("book.yaml").exists());
        assert!(workspace.join("style.yaml").exists());
        assert!(workspace.join("assets/images").exists());
        assert!(
            workspace
                .join("content/frontmatter/001-title-page.md")
                .exists()
        );
        assert!(workspace.join("content/chapters/001-opening.md").exists());

        let registry = read_conversation_registry(&workspace).unwrap();
        assert_eq!(
            registry,
            ConversationRegistry {
                version: 1,
                book_id: "quiet-lighthouse".to_string(),
                conversations: Vec::new(),
            }
        );
    }

    #[tokio::test]
    async fn create_book_rejects_duplicate_slug() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(&temp_dir).await;
        let token = login_token(router.clone()).await;

        for title in ["Quiet Lighthouse", "Quiet---Lighthouse"] {
            let response = router
                .clone()
                .oneshot(
                    Request::post("/api/books")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(format!(r#"{{"title":"{title}"}}"#)))
                        .unwrap(),
                )
                .await
                .unwrap();

            if title == "Quiet Lighthouse" {
                assert_eq!(response.status(), StatusCode::CREATED);
            } else {
                assert_eq!(response.status(), StatusCode::CONFLICT);
                let payload = response_json(response).await;
                assert_eq!(payload["code"], "duplicate_book_slug");
            }
        }
    }

    #[tokio::test]
    async fn create_book_keeps_workspace_inside_books_root() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(&temp_dir).await;
        let token = login_token(router.clone()).await;

        let response = router
            .clone()
            .oneshot(
                Request::post("/api/books")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"../../../Outside Root"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = response_json(response).await;
        assert_eq!(payload["slug"], "outside-root");

        let workspace = temp_dir.path().join("books-data/outside-root");
        assert!(workspace.exists());
        assert!(workspace.starts_with(temp_dir.path().join("books-data")));
        assert!(!temp_dir.path().join("outside-root").exists());
    }

    #[tokio::test]
    async fn get_books_lists_newly_created_workspaces() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(&temp_dir).await;
        let token = login_token(router.clone()).await;

        let create_response = router
            .clone()
            .oneshot(
                Request::post("/api/books")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"title":"Quiet Lighthouse","language":"ru"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let list_response = router
            .oneshot(
                Request::get("/api/books")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let payload = response_json(list_response).await;
        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert_eq!(payload[0]["book_id"], "quiet-lighthouse");
        assert_eq!(payload[0]["language"], "ru");
        assert_eq!(payload[0]["title"], "Quiet Lighthouse");
    }

    #[tokio::test]
    async fn create_book_rejects_empty_title() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(&temp_dir).await;
        let token = login_token(router.clone()).await;

        let response = router
            .oneshot(
                Request::post("/api/books")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"title\":\"   \"}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "invalid_title");
    }
}
