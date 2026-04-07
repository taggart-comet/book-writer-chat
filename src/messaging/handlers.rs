use anyhow::Result;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;

use crate::{
    app::state::AppState,
    authoring::flow::{authoring_flow, seed_initial_render},
    core::models::{NormalizedMessage, Notification, Provider},
    messaging::{
        commands::{ParsedCommand, parse_command},
        providers::{normalize_max, normalize_telegram},
    },
    reader::links::issue_token,
    storage::workspace::{ensure_workspace, workspace_dir},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageApiResponse {
    pub handled: bool,
    pub ignored: bool,
    pub notification: Option<Notification>,
}

pub async fn telegram_webhook(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match normalize_telegram(payload, &state.config.telegram_bot_username) {
        Ok(message) => match message_flow(state, message).await {
            Ok(response) => (StatusCode::OK, Json(response)).into_response(),
            Err(error) => {
                error!(?error, "telegram webhook failed");
                (
                    StatusCode::BAD_REQUEST,
                    Json(MessageApiResponse {
                        handled: false,
                        ignored: false,
                        notification: Some(Notification {
                            provider: Provider::Telegram,
                            provider_chat_id: "unknown".to_string(),
                            message: "Invalid Telegram payload".to_string(),
                            reader_url: None,
                        }),
                    }),
                )
                    .into_response()
            }
        },
        Err(error) => {
            error!(?error, "telegram webhook normalization failed");
            (
                StatusCode::BAD_REQUEST,
                Json(MessageApiResponse {
                    handled: false,
                    ignored: false,
                    notification: Some(Notification {
                        provider: Provider::Telegram,
                        provider_chat_id: "unknown".to_string(),
                        message: "Invalid Telegram payload".to_string(),
                        reader_url: None,
                    }),
                }),
            )
                .into_response()
        }
    }
}

pub async fn max_webhook(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match normalize_max(payload, &state.config.max_bot_handle) {
        Ok(message) => match message_flow(state, message).await {
            Ok(response) => (StatusCode::OK, Json(response)).into_response(),
            Err(error) => {
                error!(?error, "max webhook failed");
                (StatusCode::BAD_REQUEST, error.to_string()).into_response()
            }
        },
        Err(error) => {
            error!(?error, "max webhook failed");
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
    }
}

pub async fn message_flow(
    state: AppState,
    message: NormalizedMessage,
) -> Result<MessageApiResponse> {
    state.metrics.inc_inbound();
    if message.raw_text.len() > 4_000 {
        return Ok(MessageApiResponse {
            handled: true,
            ignored: false,
            notification: Some(Notification {
                provider: message.provider,
                provider_chat_id: message.provider_chat_id,
                message: "Message too large for processing.".to_string(),
                reader_url: None,
            }),
        });
    }
    if !message.mentions_bot {
        return Ok(MessageApiResponse {
            handled: false,
            ignored: true,
            notification: None,
        });
    }

    let conversation = state
        .repository
        .resolve_or_create_conversation(
            message.provider.clone(),
            message.provider_chat_id.clone(),
            format!("{} conversation", message.provider_chat_id),
        )
        .await?;
    let bot_name = match message.provider {
        Provider::Telegram => &state.config.telegram_bot_username,
        Provider::Max => &state.config.max_bot_handle,
    };
    let Some(parsed) = parse_command(&message.raw_text, message.mentions_bot, bot_name) else {
        return Ok(MessageApiResponse {
            handled: false,
            ignored: true,
            notification: None,
        });
    };

    match parsed {
        ParsedCommand::Init => init_flow(state, conversation.conversation_id, message).await,
        ParsedCommand::Status => status_flow(state, conversation.conversation_id, message).await,
        ParsedCommand::Authoring(instruction) => {
            authoring_flow(state, conversation.conversation_id, message, instruction).await
        }
    }
}

pub async fn init_flow(
    state: AppState,
    conversation_id: String,
    message: NormalizedMessage,
) -> Result<MessageApiResponse> {
    let existing = state
        .repository
        .find_book_by_conversation(&conversation_id)
        .await;
    let book = if let Some(book) = existing {
        book
    } else {
        let workspace = workspace_dir(&state.config.books_root, &conversation_id);
        let workspace_path = workspace.display().to_string();
        let book = state
            .repository
            .create_book(
                &conversation_id,
                "Untitled Conversation Book".to_string(),
                workspace_path,
            )
            .await?;
        let workspace = ensure_workspace(&state.config.books_root, &conversation_id, &book)?;
        seed_initial_render(
            &state,
            &book.book_id,
            &conversation_id,
            &workspace,
            &message.message_id,
        )
        .await?;
        book
    };
    let token = issue_token(&state.config.reader_token_secret, &book.book_id, 24 * 30)?;
    let reply = Notification {
        provider: message.provider,
        provider_chat_id: message.provider_chat_id,
        message: "Book workspace is ready for this conversation.".to_string(),
        reader_url: Some(format!(
            "{}/reader/{}",
            state.config.frontend_base_url, token
        )),
    };
    Ok(MessageApiResponse {
        handled: true,
        ignored: false,
        notification: Some(reply),
    })
}

pub async fn status_flow(
    state: AppState,
    conversation_id: String,
    message: NormalizedMessage,
) -> Result<MessageApiResponse> {
    let response = if let Some(book) = state
        .repository
        .find_book_by_conversation(&conversation_id)
        .await
    {
        let revision = state
            .repository
            .latest_revision_for_book(&book.book_id)
            .await;
        let job = state.repository.latest_job_for_book(&book.book_id).await;
        Notification {
            provider: message.provider,
            provider_chat_id: message.provider_chat_id,
            message: format!(
                "Book status: {:?}. Latest revision: {}. Latest job: {}.",
                book.status,
                revision
                    .as_ref()
                    .map(|revision| revision.revision_id.as_str())
                    .unwrap_or("none"),
                job.as_ref()
                    .map(|job| format!("{:?}", job.status))
                    .unwrap_or_else(|| "none".to_string())
            ),
            reader_url: Some(format!(
                "{}/reader/{}",
                state.config.frontend_base_url,
                issue_token(&state.config.reader_token_secret, &book.book_id, 24 * 30)?
            )),
        }
    } else {
        Notification {
            provider: message.provider,
            provider_chat_id: message.provider_chat_id,
            message: "No book exists for this conversation yet. Run init first.".to_string(),
            reader_url: None,
        }
    };
    Ok(MessageApiResponse {
        handled: true,
        ignored: false,
        notification: Some(response),
    })
}
