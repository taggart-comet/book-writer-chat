use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

use crate::{
    app::state::{AppState, conversation_lock},
    authoring::{
        executor::{ExecutionOutcome, ExecutionRequest},
        prompt::build_prompt,
    },
    core::models::{
        BookLanguage, CommandKind, JobStatus, NormalizedMessage, Notification, RevisionRenderStatus,
    },
    messaging::handlers::MessageApiResponse,
    reader::links::{READER_TOKEN_TTL_HOURS, issue_token, reader_url},
    storage::{
        media_assets::{SavedImageAttachment, save_image_attachment},
        render_store::render_workspace,
        workspace::{
            diff_workspace, ensure_workspace, read_book_language, snapshot_workspace, workspace_dir,
        },
    },
};

pub async fn authoring_flow(
    state: AppState,
    conversation_id: String,
    message: NormalizedMessage,
    instruction: String,
) -> Result<MessageApiResponse> {
    let Some(book) = state
        .repository
        .find_book_by_conversation(&conversation_id)
        .await
    else {
        return Ok(MessageApiResponse {
            handled: true,
            ignored: false,
            notification: Some(Notification {
                provider: message.provider,
                provider_chat_id: message.provider_chat_id,
                message: "Run init first so this conversation gets its own book workspace."
                    .to_string(),
                reader_url: None,
            }),
        });
    };
    let conversation_lock = conversation_lock(&state, &conversation_id).await;
    let _conversation_guard = conversation_lock.lock().await;

    let workspace = ensure_workspace(&state.config.books_root, &conversation_id, &book)?;
    let language = read_book_language(&workspace);
    let expected_workspace = workspace_dir(&state.config.books_root, &conversation_id);
    if workspace != expected_workspace || PathBuf::from(&book.workspace_path) != expected_workspace
    {
        return Ok(MessageApiResponse {
            handled: true,
            ignored: false,
            notification: Some(Notification {
                provider: message.provider,
                provider_chat_id: message.provider_chat_id,
                message: "This conversation's workspace mapping is invalid. Re-run init for a clean workspace."
                    .to_string(),
                reader_url: None,
            }),
        });
    }
    let before = snapshot_workspace(&workspace)?;
    let saved_images = match save_message_images(&state, &workspace, &message).await {
        Ok(saved_images) => saved_images,
        Err(error) => {
            return Ok(MessageApiResponse {
                handled: true,
                ignored: false,
                notification: Some(Notification {
                    provider: message.provider,
                    provider_chat_id: message.provider_chat_id,
                    message: format!(
                        "The image attachment could not be saved for the book: {error}"
                    ),
                    reader_url: None,
                }),
            });
        }
    };
    let session = state
        .repository
        .open_session(&conversation_id, &book.book_id, message.timestamp)
        .await?;
    let prompt = build_prompt(&workspace, &book, &instruction, &message, &saved_images)?;
    let job = state
        .repository
        .create_job(
            &book.book_id,
            &conversation_id,
            &session.session_id,
            &message.message_id,
            CommandKind::Authoring,
            prompt.clone(),
        )
        .await?;
    state
        .repository
        .update_job_status(
            &job.job_id,
            JobStatus::Accepted,
            Some(localized_text(language).job_accepted.to_string()),
            None,
            None,
        )
        .await?;
    state
        .repository
        .update_job_status(
            &job.job_id,
            JobStatus::Running,
            Some(localized_text(language).job_running.to_string()),
            None,
            None,
        )
        .await?;

    info!(job_id = %job.job_id, book_id = %book.book_id, "starting authoring job");
    let outcome = state
        .executor
        .execute(ExecutionRequest {
            workspace: workspace.clone(),
            prompt,
        })
        .await;
    let after = snapshot_workspace(&workspace)?;
    let changed_files = diff_workspace(&before, &after);

    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            state.metrics.inc_failure();
            state
                .repository
                .update_job_status(
                    &job.job_id,
                    JobStatus::Failed,
                    Some(localized_text(language).job_failed_to_start.to_string()),
                    Some(changed_files),
                    Some(format!("launcher failure: {error:#}")),
                )
                .await?;
            return Ok(MessageApiResponse {
                handled: true,
                ignored: false,
                notification: Some(Notification {
                    provider: message.provider,
                    provider_chat_id: message.provider_chat_id,
                    message: localized_text(language).job_could_not_start.to_string(),
                    reader_url: None,
                }),
            });
        }
    };

    finalize_authoring(
        state,
        book.book_id,
        workspace,
        job.job_id,
        message,
        language,
        outcome,
        changed_files,
    )
    .await
}

async fn save_message_images(
    state: &AppState,
    workspace: &Path,
    message: &NormalizedMessage,
) -> Result<Vec<SavedImageAttachment>> {
    let mut saved_images = Vec::new();
    for (index, attachment) in message.attachments.iter().enumerate() {
        let media = state
            .media_downloader
            .download(&message.provider, attachment)
            .await
            .with_context(|| {
                format!(
                    "failed to download image attachment {} from {:?}",
                    index + 1,
                    message.provider
                )
            })?;
        saved_images.push(save_image_attachment(
            workspace,
            &message.provider,
            &message.message_id,
            index,
            attachment,
            media,
        )?);
    }
    Ok(saved_images)
}

pub async fn finalize_authoring(
    state: AppState,
    book_id: String,
    workspace: PathBuf,
    job_id: String,
    message: NormalizedMessage,
    language: BookLanguage,
    outcome: ExecutionOutcome,
    changed_files: Vec<String>,
) -> Result<MessageApiResponse> {
    if outcome.timed_out {
        state.metrics.inc_failure();
        state
            .repository
            .update_job_status(
                &job_id,
                JobStatus::TimedOut,
                Some(localized_text(language).job_timed_out.to_string()),
                Some(changed_files),
                Some(outcome.stderr),
            )
            .await?;
        return Ok(MessageApiResponse {
            handled: true,
            ignored: false,
            notification: Some(Notification {
                provider: message.provider,
                provider_chat_id: message.provider_chat_id,
                message: localized_text(language)
                    .job_timed_out_before_finish
                    .to_string(),
                reader_url: None,
            }),
        });
    }
    if outcome.exit_code != Some(0) {
        state.metrics.inc_failure();
        state
            .repository
            .update_job_status(
                &job_id,
                JobStatus::Failed,
                Some(localized_text(language).job_failed.to_string()),
                Some(changed_files),
                Some(outcome.stderr),
            )
            .await?;
        return Ok(MessageApiResponse {
            handled: true,
            ignored: false,
            notification: Some(Notification {
                provider: message.provider,
                provider_chat_id: message.provider_chat_id,
                message: localized_text(language).job_failed_try_again.to_string(),
                reader_url: None,
            }),
        });
    }

    if let Err(error) = render_workspace(&workspace) {
        state.metrics.inc_failure();
        state
            .repository
            .update_job_status(
                &job_id,
                JobStatus::Failed,
                Some(
                    localized_text(language)
                        .render_refresh_failed_job
                        .to_string(),
                ),
                Some(changed_files),
                Some(format!("render refresh failure: {error:#}")),
            )
            .await?;
        return Ok(MessageApiResponse {
            handled: true,
            ignored: false,
            notification: Some(Notification {
                provider: message.provider,
                provider_chat_id: message.provider_chat_id,
                message: localized_text(language)
                    .render_refresh_failed_user
                    .to_string(),
                reader_url: None,
            }),
        });
    }
    persist_revision(
        &state,
        &book_id,
        &job_id,
        &revision_summary(&changed_files, &outcome),
    )
    .await?;
    state.repository.touch_book(&book_id).await?;
    state.metrics.inc_success();
    state
        .repository
        .update_job_status(
            &job_id,
            JobStatus::Succeeded,
            Some(localized_text(language).draft_updated_job.to_string()),
            Some(changed_files),
            None,
        )
        .await?;
    let token = issue_token(
        &state.config.reader_token_secret,
        &book_id,
        READER_TOKEN_TTL_HOURS,
    )?;
    Ok(MessageApiResponse {
        handled: true,
        ignored: false,
        notification: Some(Notification {
            provider: message.provider,
            provider_chat_id: message.provider_chat_id,
            message: localized_text(language).draft_updated_user.to_string(),
            reader_url: Some(reader_url(&state.config.frontend_base_url, &token)),
        }),
    })
}

pub fn revision_summary(changed_files: &[String], outcome: &ExecutionOutcome) -> String {
    let summary = if changed_files.is_empty() {
        "Workspace updated with no detected file delta".to_string()
    } else {
        format!("Updated files: {}", changed_files.join(", "))
    };
    if outcome.stdout.trim().is_empty() {
        summary
    } else {
        format!("{summary}. Executor stdout: {}", outcome.stdout.trim())
    }
}

pub async fn seed_initial_render(
    state: &AppState,
    book_id: &str,
    conversation_id: &str,
    workspace: &Path,
    message_id: &str,
) -> Result<()> {
    let session = state
        .repository
        .open_session(conversation_id, book_id, chrono::Utc::now())
        .await?;
    let job = state
        .repository
        .create_job(
            book_id,
            conversation_id,
            &session.session_id,
            message_id,
            CommandKind::Init,
            "Initial workspace render".to_string(),
        )
        .await?;
    state
        .repository
        .update_job_status(
            &job.job_id,
            JobStatus::Succeeded,
            Some(
                localized_text(read_book_language(workspace))
                    .initial_draft_created
                    .to_string(),
            ),
            Some(Vec::new()),
            None,
        )
        .await?;
    render_workspace(workspace)?;
    persist_revision(state, book_id, &job.job_id, "Initial workspace render").await
}

pub async fn persist_revision(
    state: &AppState,
    book_id: &str,
    job_id: &str,
    summary: &str,
) -> Result<()> {
    state
        .repository
        .create_revision(
            book_id,
            job_id,
            summary.to_string(),
            RevisionRenderStatus::Ready,
        )
        .await?;
    Ok(())
}

struct LocalizedAuthoringText {
    job_accepted: &'static str,
    job_running: &'static str,
    job_failed_to_start: &'static str,
    job_could_not_start: &'static str,
    job_timed_out: &'static str,
    job_timed_out_before_finish: &'static str,
    job_failed: &'static str,
    job_failed_try_again: &'static str,
    render_refresh_failed_job: &'static str,
    render_refresh_failed_user: &'static str,
    draft_updated_job: &'static str,
    draft_updated_user: &'static str,
    initial_draft_created: &'static str,
}

fn localized_text(language: BookLanguage) -> LocalizedAuthoringText {
    match language {
        BookLanguage::English => LocalizedAuthoringText {
            job_accepted: "Job accepted",
            job_running: "Job running",
            job_failed_to_start: "The writing job failed before it could start.",
            job_could_not_start: "The writing job could not be started. Please try again.",
            job_timed_out: "The writing job timed out.",
            job_timed_out_before_finish: "The writing job timed out before finishing.",
            job_failed: "The writing job failed.",
            job_failed_try_again: "The writing job failed. Please try again.",
            render_refresh_failed_job: "The draft changed, but render refresh failed.",
            render_refresh_failed_user: "The draft changed, but the reader view could not be refreshed.",
            draft_updated_job: "The draft was updated successfully.",
            draft_updated_user: "Draft updated successfully.",
            initial_draft_created: "Initial draft created.",
        },
        BookLanguage::Russian => LocalizedAuthoringText {
            job_accepted: "Задача принята",
            job_running: "Задача выполняется",
            job_failed_to_start: "Задачу написания не удалось запустить.",
            job_could_not_start: "Задачу написания не удалось запустить. Попробуйте еще раз.",
            job_timed_out: "Задача написания не завершилась вовремя.",
            job_timed_out_before_finish: "Задача написания не успела завершиться.",
            job_failed: "Задача написания завершилась с ошибкой.",
            job_failed_try_again: "Задача написания завершилась с ошибкой. Попробуйте еще раз.",
            render_refresh_failed_job: "Черновик изменился, но обновить отображение не удалось.",
            render_refresh_failed_user: "Черновик изменился, но читательский вид не удалось обновить.",
            draft_updated_job: "Черновик успешно обновлен.",
            draft_updated_user: "Черновик успешно обновлен.",
            initial_draft_created: "Первый черновик создан.",
        },
    }
}
