use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        auth::AuthenticatedOperator,
        errors::{api_error, internal_api_error},
        state::{AppState, conversation_lock},
    },
    core::models::{MessageAttachment, MessageAttachmentKind, Provider},
    storage::media_assets::{DownloadedMedia, save_image_attachment},
    storage::web_books::{
        ConversationRegistryError, ConversationRegistryRecord, TranscriptReadError,
        append_conversation_record, attach_conversation_session, find_book_workspace,
        list_conversation_records, mark_conversation_message_activity,
        read_conversation_transcript_snapshot, update_conversation_status,
        update_conversation_title,
    },
};

const DEFAULT_CONVERSATION_TITLE: &str = "New conversation";
const CONVERSATION_STATUS_PENDING: &str = "pending";
const CONVERSATION_STATUS_IN_PROGRESS: &str = "in_progress";
const CONVERSATION_STATUS_READY: &str = "ready";
const CONVERSATION_STATUS_FAILED: &str = "failed";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/books/:book_id/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/books/:book_id/conversations/:conversation_id/messages",
            get(get_conversation_messages).post(submit_conversation_message),
        )
}

#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebConversationResponse {
    pub conversation_id: String,
    pub book_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_active_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmitConversationMessageResponse {
    pub conversation_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMessagesResponse {
    pub status: String,
    pub last_comment: Option<String>,
    pub messages: Vec<crate::storage::web_books::NormalizedTranscriptMessage>,
}

pub async fn list_conversations(
    _auth: AuthenticatedOperator,
    Path(book_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let book = match find_book_workspace(&state.config.books_root, &book_id) {
        Ok(Some(book)) => book,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "book_not_found", "Book not found."),
        Err(error) => {
            return internal_api_error(
                "list_conversations.find_book_workspace",
                &error,
                "conversation_list_failed",
                "Failed to list conversations.",
            );
        }
    };

    match list_conversation_records(&book.workspace_path) {
        Ok(conversations) => Json(
            conversations
                .into_iter()
                .map(WebConversationResponse::from)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => internal_api_error(
            "list_conversations.list_conversation_records",
            &error,
            "conversation_list_failed",
            "Failed to list conversations.",
        ),
    }
}

pub async fn create_conversation(
    _auth: AuthenticatedOperator,
    Path(book_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<CreateConversationRequest>,
) -> Response {
    let book = match find_book_workspace(&state.config.books_root, &book_id) {
        Ok(Some(book)) => book,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "book_not_found", "Book not found."),
        Err(error) => {
            return internal_api_error(
                "create_conversation.find_book_workspace",
                &error,
                "conversation_create_failed",
                "Failed to create conversation.",
            );
        }
    };

    let title = payload
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CONVERSATION_TITLE)
        .to_string();

    let created_at = chrono::Utc::now();
    let record = ConversationRegistryRecord {
        conversation_id: generate_conversation_id(),
        book_id: book.book_id.clone(),
        title,
        session_id: None,
        session_log_path: String::new(),
        created_at,
        updated_at: created_at,
        last_active_at: created_at,
        status: CONVERSATION_STATUS_PENDING.to_string(),
    };

    match append_conversation_record(&book.workspace_path, record.clone()) {
        Ok(_) => (
            StatusCode::CREATED,
            Json(WebConversationResponse::from(record)),
        )
            .into_response(),
        Err(ConversationRegistryError::DuplicateConversation { .. }) => api_error(
            StatusCode::CONFLICT,
            "duplicate_conversation",
            "Conversation already exists.",
        ),
        Err(error) => internal_api_error(
            "create_conversation.append_conversation_record",
            &error,
            "conversation_create_failed",
            "Failed to create conversation.",
        ),
    }
}

pub async fn submit_conversation_message(
    _auth: AuthenticatedOperator,
    Path((book_id, conversation_id)): Path<(String, String)>,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    let book = match find_book_workspace(&state.config.books_root, &book_id) {
        Ok(Some(book)) => book,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "book_not_found", "Book not found."),
        Err(error) => {
            return internal_api_error(
                "submit_conversation_message.find_book_workspace",
                &error,
                "conversation_submit_failed",
                "Failed to submit conversation message.",
            );
        }
    };

    let parsed =
        match parse_conversation_message_request(multipart, &book.workspace_path, &conversation_id)
            .await
        {
            Ok(parsed) => parsed,
            Err(ConversationMessageRequestError::InvalidMessage) => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_message",
                    "Message text must not be empty.",
                );
            }
            Err(ConversationMessageRequestError::InvalidAttachment) => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_attachment",
                    "Uploaded file must be a supported image.",
                );
            }
            Err(ConversationMessageRequestError::Multipart(error)) => {
                tracing::warn!(error = %error, conversation_id, "invalid multipart message payload");
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_message",
                    "Message payload is invalid.",
                );
            }
        };

    let prompt = parsed.prompt.trim().to_string();
    if prompt.is_empty() {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_message",
            "Message text must not be empty.",
        );
    }

    let lock = conversation_lock(&state, &conversation_id).await;
    let (conversation_title, previous_session_id) = {
        let _guard = lock.lock().await;
        let record = match list_conversation_records(&book.workspace_path) {
            Ok(records) => match records
                .into_iter()
                .find(|record| record.conversation_id == conversation_id)
            {
                Some(record) => record,
                None => {
                    return api_error(
                        StatusCode::NOT_FOUND,
                        "conversation_not_found",
                        "Conversation not found.",
                    );
                }
            },
            Err(error) => {
                return internal_api_error(
                    "submit_conversation_message.list_conversation_records",
                    &error,
                    "conversation_list_failed",
                    "Failed to load conversation metadata.",
                );
            }
        };

        if record.status == CONVERSATION_STATUS_IN_PROGRESS {
            return api_error(
                StatusCode::CONFLICT,
                "conversation_busy",
                "Conversation is already processing another request.",
            );
        }

        let conversation_title = if record.session_id.is_none() {
            conversation_title_from_request(&prompt)
        } else {
            record.title.clone()
        };

        if conversation_title != record.title {
            match update_conversation_title(
                &book.workspace_path,
                &conversation_id,
                &conversation_title,
            ) {
                Ok(_) => {}
                Err(ConversationRegistryError::ConversationNotFound { .. }) => {
                    return api_error(
                        StatusCode::NOT_FOUND,
                        "conversation_not_found",
                        "Conversation not found.",
                    );
                }
                Err(error) => {
                    return internal_api_error(
                        "submit_conversation_message.update_conversation_title",
                        &error,
                        "conversation_update_failed",
                        "Failed to update conversation title.",
                    );
                }
            }
        }

        let activity_at = chrono::Utc::now();
        match update_conversation_status(
            &book.workspace_path,
            &conversation_id,
            CONVERSATION_STATUS_IN_PROGRESS,
            Some(activity_at),
        ) {
            Ok(_) => (conversation_title, record.session_id.clone()),
            Err(ConversationRegistryError::ConversationNotFound { .. }) => {
                return api_error(
                    StatusCode::NOT_FOUND,
                    "conversation_not_found",
                    "Conversation not found.",
                );
            }
            Err(error) => {
                return internal_api_error(
                    "submit_conversation_message.update_conversation_status",
                    &error,
                    "conversation_update_failed",
                    "Failed to update conversation status.",
                );
            }
        }
    };

    let background_state = state.clone();
    let workspace_path = book.workspace_path.clone();
    let background_conversation_id = conversation_id.clone();

    tokio::spawn(async move {
        process_conversation_message(
            background_state,
            workspace_path,
            background_conversation_id,
            conversation_title,
            previous_session_id,
            prompt,
        )
        .await;
    });

    (
        StatusCode::OK,
        Json(SubmitConversationMessageResponse {
            conversation_id,
            status: CONVERSATION_STATUS_IN_PROGRESS.to_string(),
        }),
    )
        .into_response()
}

pub async fn get_conversation_messages(
    _auth: AuthenticatedOperator,
    Path((book_id, conversation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    let book = match find_book_workspace(&state.config.books_root, &book_id) {
        Ok(Some(book)) => book,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "book_not_found", "Book not found."),
        Err(error) => {
            return internal_api_error(
                "get_conversation_messages.find_book_workspace",
                &error,
                "session_log_read_failed",
                "Failed to load conversation transcript.",
            );
        }
    };

    let conversation_status = match list_conversation_records(&book.workspace_path) {
        Ok(records) => match records
            .into_iter()
            .find(|record| record.conversation_id == conversation_id)
        {
            Some(record) => record.status,
            None => {
                return api_error(
                    StatusCode::NOT_FOUND,
                    "conversation_not_found",
                    "Conversation not found.",
                );
            }
        },
        Err(error) => {
            return internal_api_error(
                "get_conversation_messages.list_conversation_records",
                &error,
                "conversation_list_failed",
                "Failed to load conversation metadata.",
            );
        }
    };

    match read_conversation_transcript_snapshot(&book.workspace_path, &conversation_id) {
        Ok(snapshot) => {
            if let Some(activity_at) = snapshot
                .messages
                .iter()
                .filter_map(|message| message.timestamp)
                .max()
            {
                let _ = mark_conversation_message_activity(
                    &book.workspace_path,
                    &conversation_id,
                    activity_at,
                );
            }
            Json(ConversationMessagesResponse {
                status: conversation_status,
                last_comment: snapshot.last_comment,
                messages: snapshot.messages,
            })
            .into_response()
        }
        Err(TranscriptReadError::ConversationNotFound { .. }) => api_error(
            StatusCode::NOT_FOUND,
            "conversation_not_found",
            "Conversation not found.",
        ),
        Err(TranscriptReadError::InvalidSessionLogPath) => {
            tracing::error!(conversation_id, "conversation transcript has invalid session log path");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_log_path_invalid",
                "Conversation transcript is unavailable.",
            )
        }
        Err(TranscriptReadError::SessionLogMissing) => {
            tracing::warn!(conversation_id, "conversation transcript session log is missing");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_log_missing",
                "Conversation transcript is not available yet.",
            )
        }
        Err(TranscriptReadError::MalformedLogLine { line }) => {
            tracing::error!(conversation_id, line, "conversation transcript contains malformed log line");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_log_read_failed",
                "Failed to load conversation transcript.",
            )
        }
        Err(TranscriptReadError::Io(error)) => internal_api_error(
            "get_conversation_messages.read_conversation_transcript_snapshot",
            &error,
            "session_log_read_failed",
            "Failed to load conversation transcript.",
        ),
        Err(TranscriptReadError::Json(error)) => internal_api_error(
            "get_conversation_messages.read_conversation_transcript_snapshot",
            &error,
            "session_log_read_failed",
            "Failed to load conversation transcript.",
        ),
        Err(TranscriptReadError::Other(error)) => internal_api_error(
            "get_conversation_messages.read_conversation_transcript_snapshot",
            &error,
            "session_log_read_failed",
            "Failed to load conversation transcript.",
        ),
    }
}

impl From<ConversationRegistryRecord> for WebConversationResponse {
    fn from(value: ConversationRegistryRecord) -> Self {
        Self {
            conversation_id: value.conversation_id,
            book_id: value.book_id,
            title: value.title,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            last_active_at: value.last_active_at.to_rfc3339(),
            status: value.status,
        }
    }
}

fn generate_conversation_id() -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!("conversation-{micros}")
}

fn conversation_title_from_request(prompt: &str) -> String {
    let words = prompt
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty() {
        return DEFAULT_CONVERSATION_TITLE.to_string();
    }

    let max_words = 6;
    if words.len() <= max_words {
        words.join(" ")
    } else {
        format!("{}...", words[..max_words].join(" "))
    }
}

async fn process_conversation_message(
    state: AppState,
    workspace_path: std::path::PathBuf,
    conversation_id: String,
    conversation_title: String,
    session_id: Option<String>,
    prompt: String,
) {
    let lock = conversation_lock(&state, &conversation_id).await;
    let _guard = lock.lock().await;

    let result = if let Some(session_id) = session_id {
        tracing::info!(conversation_id, session_id, workspace = %workspace_path.display(), "resuming conversation session");
        state
            .session_launcher
            .resume(&workspace_path, &session_id, &prompt)
            .await
    } else {
        tracing::info!(conversation_id, workspace = %workspace_path.display(), title = conversation_title, "launching conversation session");
        state
            .session_launcher
            .launch(&workspace_path, &conversation_title, &prompt)
            .await
    };

    let now = chrono::Utc::now();
    match result {
        Ok(run) => {
            tracing::info!(
                conversation_id,
                session_id = run.session_id,
                session_log_path = %run.session_log_path.display(),
                "conversation session completed"
            );
            if let Err(error) = attach_conversation_session(
                &workspace_path,
                &conversation_id,
                &run.session_id,
                &run.session_log_path.to_string_lossy(),
                CONVERSATION_STATUS_READY,
                Some(now),
            ) {
                tracing::error!(conversation_id, error = %error, "failed to attach conversation session metadata");
            }
        }
        Err(error) => {
            tracing::error!(conversation_id, workspace = %workspace_path.display(), error = %error, "conversation session failed");
            if let Err(status_error) = update_conversation_status(
                &workspace_path,
                &conversation_id,
                CONVERSATION_STATUS_FAILED,
                Some(now),
            ) {
                tracing::error!(
                    conversation_id,
                    error = %status_error,
                    "failed to mark conversation session as failed"
                );
            }
        }
    }
}

struct ParsedConversationMessageRequest {
    prompt: String,
}

#[derive(Debug)]
enum ConversationMessageRequestError {
    InvalidMessage,
    InvalidAttachment,
    Multipart(axum::extract::multipart::MultipartError),
}

impl From<axum::extract::multipart::MultipartError> for ConversationMessageRequestError {
    fn from(error: axum::extract::multipart::MultipartError) -> Self {
        Self::Multipart(error)
    }
}

async fn parse_conversation_message_request(
    mut multipart: Multipart,
    workspace_path: &std::path::Path,
    conversation_id: &str,
) -> Result<ParsedConversationMessageRequest, ConversationMessageRequestError> {
    let mut text = None;
    let mut image_path = None;
    let mut image_index = 0usize;

    while let Some(field) = multipart.next_field().await? {
        let Some(name) = field.name().map(str::to_string) else {
            continue;
        };

        match name.as_str() {
            "text" => {
                text = Some(
                    field
                        .text()
                        .await
                        .map_err(ConversationMessageRequestError::Multipart)?,
                );
            }
            "image" => {
                if image_path.is_some() {
                    continue;
                }

                let content_type = field.content_type().map(str::to_string);
                let file_name = field.file_name().map(str::to_string);
                let bytes = field
                    .bytes()
                    .await
                    .map_err(ConversationMessageRequestError::Multipart)?
                    .to_vec();

                if bytes.is_empty() {
                    continue;
                }

                let attachment = MessageAttachment {
                    kind: MessageAttachmentKind::Image,
                    provider_file_id: format!("{conversation_id}-{image_index}"),
                    provider_unique_id: None,
                    original_filename: file_name.clone(),
                    mime_type: content_type.clone(),
                    width: None,
                    height: None,
                    file_size: Some(bytes.len() as u64),
                    caption: None,
                };

                let saved = save_image_attachment(
                    workspace_path,
                    &Provider::App,
                    conversation_id,
                    image_index,
                    &attachment,
                    DownloadedMedia {
                        bytes,
                        mime_type: content_type,
                        provider_file_path: file_name,
                    },
                )
                .map_err(|_| ConversationMessageRequestError::InvalidAttachment)?;
                image_path = Some(workspace_path.join(saved.workspace_relative_path));
                image_index += 1;
            }
            _ => {}
        }
    }

    let text = text.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(ConversationMessageRequestError::InvalidMessage);
    }

    let prompt = match image_path {
        Some(path) => format!("{text}\n{}", path.display()),
        None => text,
    };

    Ok(ParsedConversationMessageRequest { prompt })
}

#[cfg(test)]
mod tests {
    use std::{env, path::Path, sync::Arc, sync::MutexGuard};

    use anyhow::Result;
    use axum::{
        body::Body,
        http::{Request, header},
    };
    use chrono::{Duration, TimeZone, Utc};
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
        storage::{
            repository::Repository,
            web_books::{initialize_conversation_registry, provision_book_workspace},
        },
    };

    use super::*;

    struct FakeSessionLauncher {
        handler: Arc<dyn Fn(&Path, &str, &str) -> Result<SessionLaunchResult> + Send + Sync>,
    }

    impl FakeSessionLauncher {
        fn new<F>(handler: F) -> Self
        where
            F: Fn(&Path, &str, &str) -> Result<SessionLaunchResult> + Send + Sync + 'static,
        {
            Self {
                handler: Arc::new(handler),
            }
        }
    }

    #[async_trait::async_trait]
    impl SessionLauncher for FakeSessionLauncher {
        async fn launch(
            &self,
            workspace: &Path,
            title: &str,
            initial_prompt: &str,
        ) -> Result<SessionLaunchResult> {
            (self.handler)(workspace, title, initial_prompt)
        }

        async fn resume(
            &self,
            workspace: &Path,
            session_id: &str,
            prompt: &str,
        ) -> Result<SessionLaunchResult> {
            (self.handler)(workspace, session_id, prompt)
        }
    }

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
            "HOME",
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
            env::set_var("HOME", temp_dir.path());
        }
    }

    async fn test_router(temp_dir: &TempDir, launcher: Arc<dyn SessionLauncher>) -> Router {
        configure_env(temp_dir);
        let config = Config::from_env().unwrap();
        config.ensure_directories().unwrap();
        let repository = Repository::load(&config.data_dir).await.unwrap();
        let state = AppState {
            config,
            repository,
            metrics: Metrics::default(),
            conversation_locks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            session_launcher: launcher,
        };

        Router::new()
            .nest("/api", auth::routes())
            .nest("/api", crate::app::web_books::routes())
            .nest("/api", routes())
            .with_state(state)
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn multipart_body(boundary: &str, text: &str, image: Option<(&str, &str, &[u8])>) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"text\"\r\n\r\n{text}\r\n"
            )
            .as_bytes(),
        );

        if let Some((file_name, mime_type, bytes)) = image {
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{file_name}\"\r\nContent-Type: {mime_type}\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }

        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
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

    async fn create_book(router: Router, token: &str) {
        let response = router
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
    }

    #[tokio::test]
    async fn conversation_endpoints_require_authentication() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;

        let list_response = router
            .clone()
            .oneshot(
                Request::get("/api/books/quiet-lighthouse/conversations")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::UNAUTHORIZED);

        let create_response = router
            .oneshot(
                Request::post("/api/books/quiet-lighthouse/conversations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"Outline"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_and_list_conversations_persist_registry_and_last_active_at() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;
        let token = login_token(router.clone()).await;
        create_book(router.clone(), &token).await;

        let create_response = router
            .clone()
            .oneshot(
                Request::post("/api/books/quiet-lighthouse/conversations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"Opening scene"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_payload = response_json(create_response).await;
        assert_eq!(create_payload["title"], "Opening scene");
        assert_eq!(create_payload["status"], "pending");

        let list_response = router
            .oneshot(
                Request::get("/api/books/quiet-lighthouse/conversations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_payload = response_json(list_response).await;
        assert_eq!(list_payload.as_array().unwrap().len(), 1);
        assert_eq!(list_payload[0]["title"], "Opening scene");
        assert_eq!(list_payload[0]["status"], "pending");

        let registry = crate::storage::web_books::read_conversation_registry(
            &temp_dir.path().join("books-data/quiet-lighthouse"),
        )
        .unwrap();
        assert_eq!(registry.conversations.len(), 1);
        assert_eq!(registry.conversations[0].status, "pending");
        assert_eq!(registry.conversations[0].session_log_path, "");
    }

    #[tokio::test]
    async fn submit_message_returns_immediately_and_completes_in_background() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(move |workspace, title_or_session, prompt| {
                let session_id = if title_or_session.starts_with("session-") {
                    title_or_session.to_string()
                } else {
                    "session-async".to_string()
                };
                let sessions_root = home.join(".codex/sessions/2026/05/02");
                std::fs::create_dir_all(&sessions_root)?;
                let log_path = sessions_root.join(format!("{session_id}.jsonl"));
                std::fs::write(
                    &log_path,
                    format!(
                        concat!(
                            "{{\"timestamp\":\"{timestamp}\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"timestamp\":\"{timestamp}\",\"cwd\":\"{cwd}\"}}}}\n",
                            "{{\"timestamp\":\"{timestamp}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"{prompt}\"}}}}\n",
                            "{{\"timestamp\":\"{timestamp}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Done.\"}}]}}}}\n"
                        ),
                        timestamp = Utc::now().to_rfc3339(),
                        session_id = session_id,
                        cwd = workspace.display(),
                        prompt = prompt,
                    ),
                )?;
                Ok(SessionLaunchResult {
                    session_id,
                    session_log_path: log_path,
                    launched_at: Utc::now(),
                })
            })),
        )
        .await;
        let token = login_token(router.clone()).await;
        create_book(router.clone(), &token).await;

        let create_response = router
            .clone()
            .oneshot(
                Request::post("/api/books/quiet-lighthouse/conversations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"Async"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let conversation_id = response_json(create_response).await["conversation_id"]
            .as_str()
            .unwrap()
            .to_string();

        let send_response = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/books/quiet-lighthouse/conversations/{conversation_id}/messages"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(
                    header::CONTENT_TYPE,
                    "multipart/form-data; boundary=message-boundary",
                )
                .body(Body::from(multipart_body(
                    "message-boundary",
                    "Write the next section",
                    None,
                )))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(send_response.status(), StatusCode::OK);
        let send_payload = response_json(send_response).await;
        assert_eq!(send_payload["status"], "in_progress");

        let workspace = temp_dir.path().join("books-data/quiet-lighthouse");
        let background_result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let registry =
                    crate::storage::web_books::read_conversation_registry(&workspace).unwrap();
                let record = registry
                    .conversations
                    .iter()
                    .find(|record| record.conversation_id == conversation_id)
                    .unwrap()
                    .clone();
                if record.status == "ready" {
                    break record;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            background_result.session_id.as_deref(),
            Some("session-async")
        );
        assert_eq!(background_result.title, "Write the next section");
        assert!(!background_result.session_log_path.is_empty());

        let transcript_response = router
            .oneshot(
                Request::get(format!(
                    "/api/books/quiet-lighthouse/conversations/{conversation_id}/messages"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(transcript_response.status(), StatusCode::OK);
        let transcript: ConversationMessagesResponse =
            serde_json::from_value(response_json(transcript_response).await).unwrap();
        assert_eq!(transcript.status, "ready");
        assert_eq!(transcript.messages.len(), 2);
        assert_eq!(transcript.messages[0].text, "Write the next section");
        assert_eq!(transcript.messages[1].text, "Done.");
    }

    #[tokio::test]
    async fn submit_message_saves_uploaded_image_and_appends_absolute_path_to_prompt() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("books-data/quiet-lighthouse");
        let session_log_path = temp_dir
            .path()
            .join(".codex/sessions/2026/05/02/session-upload.jsonl");
        let launched_prompt = Arc::new(std::sync::Mutex::new(String::new()));
        let captured_prompt = launched_prompt.clone();
        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(move |_, _, prompt| {
                *captured_prompt.lock().unwrap() = prompt.to_string();
                Ok(SessionLaunchResult {
                    session_id: "session-upload".to_string(),
                    session_log_path: session_log_path.clone(),
                    launched_at: Utc::now(),
                })
            })),
        )
        .await;
        let token = login_token(router.clone()).await;
        create_book(router.clone(), &token).await;

        let create_response = router
            .clone()
            .oneshot(
                Request::post("/api/books/quiet-lighthouse/conversations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"title":"Upload"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let conversation_id = response_json(create_response).await["conversation_id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/books/quiet-lighthouse/conversations/{conversation_id}/messages"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(
                    header::CONTENT_TYPE,
                    "multipart/form-data; boundary=image-boundary",
                )
                .body(Body::from(multipart_body(
                    "image-boundary",
                    "Use this photo in the next section",
                    Some(("cover.png", "image/png", &[137, 80, 78, 71])),
                )))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let saved_path = workspace.join(format!("assets/images/app-{}-1.png", conversation_id));
        let expected_prompt = format!(
            "Use this photo in the next section\n{}",
            saved_path.display()
        );
        let expected_prompt_for_wait = expected_prompt.clone();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if launched_prompt.lock().unwrap().as_str() == expected_prompt_for_wait {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        assert!(saved_path.exists());
        assert_eq!(launched_prompt.lock().unwrap().as_str(), expected_prompt);
    }

    #[tokio::test]
    async fn list_conversations_orders_by_last_active_at_descending() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        configure_env(&temp_dir);
        let workspace = provision_book_workspace(
            &temp_dir.path().join("books-data"),
            "Quiet Lighthouse",
            crate::core::models::BookLanguage::English,
        )
        .unwrap();
        initialize_conversation_registry(&workspace.workspace_path, &workspace.book_id).unwrap();

        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-early".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Earlier".to_string(),
                session_id: Some("session-early".to_string()),
                session_log_path: "/tmp/early.jsonl".to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-late".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Later".to_string(),
                session_id: Some("session-late".to_string()),
                session_log_path: "/tmp/late.jsonl".to_string(),
                created_at: created_at + Duration::minutes(1),
                updated_at: created_at + Duration::minutes(7),
                last_active_at: created_at + Duration::minutes(7),
                status: "active".to_string(),
            },
        )
        .unwrap();

        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;
        let token = login_token(router.clone()).await;

        let response = router
            .oneshot(
                Request::get("/api/books/quiet-lighthouse/conversations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        let items = payload.as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["conversation_id"], "conversation-late");
        assert_eq!(items[1]["conversation_id"], "conversation-early");
    }

    #[tokio::test]
    async fn transcript_endpoint_normalizes_messages_and_updates_last_active_at() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        configure_env(&temp_dir);
        let workspace = provision_book_workspace(
            &temp_dir.path().join("books-data"),
            "Quiet Lighthouse",
            crate::core::models::BookLanguage::English,
        )
        .unwrap();
        let sessions_root = temp_dir.path().join(".codex/sessions/2026/05/01");
        std::fs::create_dir_all(&sessions_root).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        let latest_at = created_at + Duration::minutes(5);
        let log_path = sessions_root.join("rollout-2026-05-01T12-00-00-session-42.jsonl");
        std::fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"session-42\",\"timestamp\":\"{created}\",\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"# AGENTS.md instructions for /tmp/workspace\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Initialize a new web messenger conversation for this book workspace. Reply with exactly: Session ready.\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Session ready.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"Initialize a new web messenger conversation for this book workspace. Reply with exactly: Session ready.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Session ready.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{latest}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Hello\"}}}}\n",
                    "{{\"timestamp\":\"{latest}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                latest = latest_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        initialize_conversation_registry(&workspace.workspace_path, &workspace.book_id).unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-42".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Outline".to_string(),
                session_id: Some("session-42".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;
        let token = login_token(router.clone()).await;

        let response = router
            .oneshot(
                Request::get("/api/books/quiet-lighthouse/conversations/conversation-42/messages")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let payload: ConversationMessagesResponse =
            serde_json::from_value(response_json(response).await).unwrap();
        assert_eq!(payload.status, "active");
        assert_eq!(payload.messages.len(), 1);
        assert_eq!(payload.messages[0].role, "user");
        assert_eq!(payload.messages[0].text, "Hello");
        assert_eq!(payload.messages[0].message_id, "msg-000001");
        assert_eq!(payload.last_comment, None);

        let registry =
            crate::storage::web_books::read_conversation_registry(&workspace.workspace_path)
                .unwrap();
        assert_eq!(registry.conversations[0].title, "Outline");
        assert_eq!(registry.conversations[0].last_active_at, latest_at);
        assert_eq!(registry.conversations[0].updated_at, latest_at);
    }

    #[tokio::test]
    async fn transcript_endpoint_returns_empty_messages_for_pending_conversations() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        configure_env(&temp_dir);
        let workspace = provision_book_workspace(
            &temp_dir.path().join("books-data"),
            "Quiet Lighthouse",
            crate::core::models::BookLanguage::English,
        )
        .unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();

        initialize_conversation_registry(&workspace.workspace_path, &workspace.book_id).unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-pending".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Pending".to_string(),
                session_id: None,
                session_log_path: String::new(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "pending".to_string(),
            },
        )
        .unwrap();

        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;
        let token = login_token(router.clone()).await;

        let response = router
            .oneshot(
                Request::get(
                    "/api/books/quiet-lighthouse/conversations/conversation-pending/messages",
                )
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let payload: ConversationMessagesResponse =
            serde_json::from_value(response_json(response).await).unwrap();
        assert_eq!(payload.status, "pending");
        assert!(payload.messages.is_empty());
        assert_eq!(payload.last_comment, None);
    }

    #[tokio::test]
    async fn transcript_endpoint_returns_last_comment_separately_from_messages() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        configure_env(&temp_dir);
        let workspace = provision_book_workspace(
            &temp_dir.path().join("books-data"),
            "Quiet Lighthouse",
            crate::core::models::BookLanguage::English,
        )
        .unwrap();
        let sessions_dir = temp_dir.path().join(".codex/sessions/2026/05/02");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 11, 0, 0).unwrap();
        let commentary_at = created_at + chrono::Duration::seconds(1);
        let final_at = created_at + chrono::Duration::seconds(2);
        let log_path = sessions_dir.join("conversation-commentary.jsonl");
        std::fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Rewrite the opening\"}}}}\n",
                    "{{\"timestamp\":\"{commentary}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"commentary\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Reviewing the chapter arc.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{final_at}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Opening rewritten.\"}}]}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                commentary = commentary_at.to_rfc3339(),
                final_at = final_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        initialize_conversation_registry(&workspace.workspace_path, &workspace.book_id).unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-commentary".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Commentary".to_string(),
                session_id: Some("session-commentary".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "in_progress".to_string(),
            },
        )
        .unwrap();

        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;
        let token = login_token(router.clone()).await;

        let response = router
            .oneshot(
                Request::get(
                    "/api/books/quiet-lighthouse/conversations/conversation-commentary/messages",
                )
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let payload: ConversationMessagesResponse =
            serde_json::from_value(response_json(response).await).unwrap();
        assert_eq!(payload.status, "in_progress");
        assert_eq!(payload.last_comment.as_deref(), Some("Reviewing the chapter arc."));
        assert_eq!(payload.messages.len(), 2);
        assert_eq!(payload.messages[0].text, "Rewrite the opening");
        assert_eq!(payload.messages[1].text, "Opening rewritten.");
    }

    #[test]
    fn conversation_title_from_request_truncates_to_six_words_with_ellipsis() {
        assert_eq!(
            conversation_title_from_request(
                "в главе про лето добавь дополнительный пункт про пожары на Кипре"
            ),
            "в главе про лето добавь дополнительный..."
        );
        assert_eq!(
            conversation_title_from_request("short request"),
            "short request"
        );
    }

    #[tokio::test]
    async fn transcript_endpoint_rejects_invalid_paths_and_handles_missing_or_malformed_logs() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        configure_env(&temp_dir);
        let workspace = provision_book_workspace(
            &temp_dir.path().join("books-data"),
            "Quiet Lighthouse",
            crate::core::models::BookLanguage::English,
        )
        .unwrap();
        let outside_log = temp_dir.path().join("outside.jsonl");
        std::fs::write(&outside_log, "{}\n").unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();

        let router = test_router(
            &temp_dir,
            Arc::new(FakeSessionLauncher::new(|_, _, _| unreachable!())),
        )
        .await;
        let token = login_token(router.clone()).await;

        for (conversation_id, session_log_path, expected_code) in [
            (
                "bad-path",
                outside_log.to_string_lossy().to_string(),
                "session_log_path_invalid".to_string(),
            ),
            (
                "missing-log",
                temp_dir
                    .path()
                    .join(".codex/sessions/2026/05/01/missing.jsonl")
                    .to_string_lossy()
                    .to_string(),
                "session_log_missing".to_string(),
            ),
        ] {
            initialize_conversation_registry(&workspace.workspace_path, &workspace.book_id)
                .unwrap();
            append_conversation_record(
                &workspace.workspace_path,
                ConversationRegistryRecord {
                    conversation_id: conversation_id.to_string(),
                    book_id: workspace.book_id.clone(),
                    title: conversation_id.to_string(),
                    session_id: Some(format!("session-{conversation_id}")),
                    session_log_path,
                    created_at,
                    updated_at: created_at,
                    last_active_at: created_at,
                    status: "active".to_string(),
                },
            )
            .unwrap();

            let response = router
                .clone()
                .oneshot(
                    Request::get(format!(
                        "/api/books/quiet-lighthouse/conversations/{conversation_id}/messages"
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let payload = response_json(response).await;
            assert_eq!(payload["code"], expected_code);
        }

        let malformed_dir = temp_dir.path().join(".codex/sessions/2026/05/01");
        std::fs::create_dir_all(&malformed_dir).unwrap();
        let malformed_log = malformed_dir.join("malformed.jsonl");
        std::fs::write(
            &malformed_log,
            format!(
                "{{\"timestamp\":\"{}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{}\"}}}}\nnot-json\n",
                created_at.to_rfc3339(),
                workspace.workspace_path.display()
            ),
        )
        .unwrap();

        initialize_conversation_registry(&workspace.workspace_path, &workspace.book_id).unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "malformed".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Malformed".to_string(),
                session_id: Some("session-malformed".to_string()),
                session_log_path: malformed_log.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let response = router
            .oneshot(
                Request::get("/api/books/quiet-lighthouse/conversations/malformed/messages")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "session_log_read_failed");
    }
}
