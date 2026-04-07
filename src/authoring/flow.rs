use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::info;

use crate::{
    app::state::{AppState, conversation_lock},
    authoring::{
        executor::{ExecutionOutcome, ExecutionRequest},
        prompt::build_prompt,
    },
    core::models::{CommandKind, JobStatus, NormalizedMessage, Notification, RevisionRenderStatus},
    messaging::handlers::MessageApiResponse,
    reader::links::issue_token,
    storage::{
        render_store::{RenderedBook, render_workspace, write_render_snapshot},
        workspace::{diff_workspace, ensure_workspace, snapshot_workspace, workspace_dir},
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
    let session = state
        .repository
        .open_session(&conversation_id, &book.book_id, message.timestamp)
        .await?;
    let prompt = build_prompt(&workspace, &book, &instruction, &message)?;
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
            Some("Job accepted".to_string()),
            None,
            None,
        )
        .await?;
    state
        .repository
        .update_job_status(
            &job.job_id,
            JobStatus::Running,
            Some("Job running".to_string()),
            None,
            None,
        )
        .await?;

    let before = snapshot_workspace(&workspace)?;
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
                    Some("The writing job failed before it could start.".to_string()),
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
                    message: "The writing job could not be started. Please try again.".to_string(),
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
        outcome,
        changed_files,
    )
    .await
}

pub async fn finalize_authoring(
    state: AppState,
    book_id: String,
    workspace: PathBuf,
    job_id: String,
    message: NormalizedMessage,
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
                Some("The writing job timed out.".to_string()),
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
                message: "The writing job timed out before finishing.".to_string(),
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
                Some("The writing job failed.".to_string()),
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
                message: "The writing job failed. Please try again.".to_string(),
                reader_url: None,
            }),
        });
    }

    let rendered = match render_workspace(&workspace) {
        Ok(rendered) => rendered,
        Err(error) => {
            state.metrics.inc_failure();
            state
                .repository
                .update_job_status(
                    &job_id,
                    JobStatus::Failed,
                    Some("The draft changed, but render refresh failed.".to_string()),
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
                    message: "The draft changed, but the reader view could not be refreshed."
                        .to_string(),
                    reader_url: None,
                }),
            });
        }
    };
    persist_render_snapshot(
        &state,
        &book_id,
        &job_id,
        &revision_summary(&changed_files, &outcome),
        rendered,
    )
    .await?;
    state.repository.touch_book(&book_id).await?;
    state.metrics.inc_success();
    state
        .repository
        .update_job_status(
            &job_id,
            JobStatus::Succeeded,
            Some("The draft was updated successfully.".to_string()),
            Some(changed_files),
            None,
        )
        .await?;
    let token = issue_token(&state.config.reader_token_secret, &book_id, 24 * 30)?;
    Ok(MessageApiResponse {
        handled: true,
        ignored: false,
        notification: Some(Notification {
            provider: message.provider,
            provider_chat_id: message.provider_chat_id,
            message: "Draft updated successfully.".to_string(),
            reader_url: Some(format!(
                "{}/reader/{}",
                state.config.frontend_base_url, token
            )),
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
            Some("Initial draft created.".to_string()),
            Some(Vec::new()),
            None,
        )
        .await?;
    let rendered = render_workspace(workspace)?;
    persist_render_snapshot(
        state,
        book_id,
        &job.job_id,
        "Initial workspace render",
        rendered,
    )
    .await
}

pub async fn persist_render_snapshot(
    state: &AppState,
    book_id: &str,
    job_id: &str,
    summary: &str,
    rendered: RenderedBook,
) -> Result<()> {
    let revision = state
        .repository
        .create_revision(
            book_id,
            job_id,
            summary.to_string(),
            RevisionRenderStatus::Ready,
        )
        .await?;
    let storage_location =
        write_render_snapshot(&state.config.data_dir, &revision.revision_id, &rendered)?;
    state
        .repository
        .create_render_snapshot(
            &revision.revision_id,
            storage_location,
            rendered.content_hash,
        )
        .await?;
    Ok(())
}
