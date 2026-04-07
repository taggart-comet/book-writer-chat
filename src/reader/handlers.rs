use anyhow::{Result, anyhow};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{
    app::{errors::api_error, state::AppState},
    core::models::{
        Book, BookStatus, ReaderContentResponse, ReaderJobResponse, ReaderRevisionResponse,
        ReaderSummary, RenderSnapshot, Revision, RevisionRenderStatus,
    },
    reader::{
        content::{ChapterCursor, ContentQuery, encode_cursor, requested_chapter_index},
        links::verify_token,
    },
    storage::render_store::{RenderedBook, read_render_snapshot},
};

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

pub async fn resolve_book_for_token(state: &AppState, token: &str) -> Result<Book> {
    let claims = verify_token(&state.config.reader_token_secret, token)?;
    state
        .repository
        .get_book(&claims.book_id)
        .await
        .ok_or_else(|| anyhow!("book not found"))
}

pub async fn reader_summary(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    match resolve_book_for_token(&state, &query.token).await {
        Ok(book) => {
            let revision = state
                .repository
                .latest_revision_for_book(&book.book_id)
                .await;
            let snapshot = state
                .repository
                .latest_render_snapshot_for_book(&book.book_id)
                .await;
            let chapter_count = snapshot
                .as_ref()
                .and_then(|snapshot| read_render_snapshot(&snapshot.storage_location).ok())
                .map(|rendered| rendered.chapters.len())
                .unwrap_or(0);
            let render_status = revision
                .as_ref()
                .map(|revision| revision.render_status.clone())
                .unwrap_or(RevisionRenderStatus::Ready);
            let summary = ReaderSummary {
                book_id: book.book_id,
                title: book.title,
                subtitle: "Draft in progress".to_string(),
                status: BookStatus::Active,
                last_revision_id: revision.map(|revision| revision.revision_id),
                last_updated_at: book.updated_at,
                render_status,
                chapter_count,
            };
            (StatusCode::OK, Json(summary)).into_response()
        }
        Err(error) => api_error(StatusCode::UNAUTHORIZED, "access_denied", error.to_string()),
    }
}

pub async fn reader_content(
    State(state): State<AppState>,
    Query(query): Query<ContentQuery>,
) -> impl IntoResponse {
    match resolve_book_for_token(&state, &query.token).await {
        Ok(book) => {
            match load_latest_rendered_book(&state, &book.book_id, query.revision_id.as_deref())
                .await
            {
                Ok((revision, snapshot, rendered)) => {
                    let index =
                        match requested_chapter_index(&rendered, &query, &revision.revision_id) {
                            Ok(index) => index,
                            Err(response) => return response,
                        };
                    if let Some(chapter) = rendered.chapters.get(index) {
                        let payload = ReaderContentResponse {
                            revision_id: revision.revision_id.clone(),
                            content_hash: snapshot.content_hash,
                            chapter_index: index,
                            chapter_id: chapter.id.clone(),
                            title: chapter.title.clone(),
                            html: chapter.html.clone(),
                            has_more: index + 1 < rendered.chapters.len(),
                            next_cursor: (index + 1 < rendered.chapters.len()).then(|| {
                                encode_cursor(&ChapterCursor {
                                    revision_id: revision.revision_id.clone(),
                                    chapter_index: index + 1,
                                })
                            }),
                        };
                        (StatusCode::OK, Json(payload)).into_response()
                    } else {
                        api_error(
                            StatusCode::NOT_FOUND,
                            "chapter_not_found",
                            "Requested chapter was not found.",
                        )
                    }
                }
                Err(response) => response,
            }
        }
        Err(error) => api_error(StatusCode::UNAUTHORIZED, "access_denied", error.to_string()),
    }
}

pub async fn reader_revision(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    match resolve_book_for_token(&state, &query.token).await {
        Ok(book) => {
            if let Some(revision) = state
                .repository
                .latest_revision_for_book(&book.book_id)
                .await
            {
                let snapshot = state
                    .repository
                    .latest_render_snapshot_for_revision(&revision.revision_id)
                    .await;
                let payload = ReaderRevisionResponse {
                    revision_id: revision.revision_id,
                    created_at: revision.created_at,
                    source_job_id: revision.job_id,
                    summary: revision.summary.clone(),
                    render_status: revision.render_status.clone(),
                    content_hash: snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.content_hash.clone()),
                    render_error: (revision.render_status == RevisionRenderStatus::Failed)
                        .then_some(revision.summary),
                };
                (StatusCode::OK, Json(payload)).into_response()
            } else {
                api_error(
                    StatusCode::NOT_FOUND,
                    "revision_not_found",
                    "No revision is available for this book.",
                )
            }
        }
        Err(error) => api_error(StatusCode::UNAUTHORIZED, "access_denied", error.to_string()),
    }
}

pub async fn reader_job(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    match resolve_book_for_token(&state, &query.token).await {
        Ok(book) => {
            if let Some(job) = state.repository.latest_job_for_book(&book.book_id).await {
                let payload = ReaderJobResponse {
                    job_id: job.job_id,
                    status: job.status,
                    started_at: job.started_at,
                    finished_at: job.finished_at,
                    user_facing_message: job.user_facing_message,
                };
                (StatusCode::OK, Json(payload)).into_response()
            } else {
                api_error(
                    StatusCode::NOT_FOUND,
                    "job_not_found",
                    "No job is available for this book.",
                )
            }
        }
        Err(error) => api_error(StatusCode::UNAUTHORIZED, "access_denied", error.to_string()),
    }
}

pub async fn load_latest_rendered_book(
    state: &AppState,
    book_id: &str,
    expected_revision_id: Option<&str>,
) -> std::result::Result<(Revision, RenderSnapshot, RenderedBook), axum::response::Response> {
    let revision = match state.repository.latest_revision_for_book(book_id).await {
        Some(revision) => revision,
        None => {
            return Err(api_error(
                StatusCode::NOT_FOUND,
                "revision_not_found",
                "No revision is available for this book.",
            ));
        }
    };

    if let Some(expected_revision_id) = expected_revision_id {
        if revision.revision_id != expected_revision_id {
            return Err(api_error(
                StatusCode::CONFLICT,
                "stale_revision",
                "The requested revision is no longer current.",
            ));
        }
    }

    if revision.render_status == RevisionRenderStatus::Failed {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "render_failed",
            revision.summary.clone(),
        ));
    }

    let snapshot = match state
        .repository
        .latest_render_snapshot_for_revision(&revision.revision_id)
        .await
    {
        Some(snapshot) => snapshot,
        None => {
            return Err(api_error(
                StatusCode::NOT_FOUND,
                "render_snapshot_missing",
                "No render snapshot is available for the latest revision.",
            ));
        }
    };

    match read_render_snapshot(&snapshot.storage_location) {
        Ok(rendered) => Ok((revision, snapshot, rendered)),
        Err(error) => Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "render_snapshot_invalid",
            error.to_string(),
        )),
    }
}
