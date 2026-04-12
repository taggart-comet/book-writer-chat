use anyhow::{Result, anyhow};
use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{
    app::{errors::api_error, state::AppState},
    core::models::{
        Book, BookStatus, ReaderContentResponse, ReaderJobResponse, ReaderRevisionResponse,
        ReaderSummary, Revision, RevisionRenderStatus,
    },
    reader::{
        content::{ChapterCursor, ContentQuery, encode_cursor, requested_chapter_index},
        links::{ReaderTokenError, verify_token},
    },
    storage::{
        media_assets::{content_type_for_asset_path, ensure_workspace_asset_path},
        render_store::{RenderedBook, render_workspace},
        workspace::read_book_language,
    },
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

fn reader_access_error(error: anyhow::Error) -> axum::response::Response {
    let status = if error.downcast_ref::<ReaderTokenError>().is_some() {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::UNAUTHORIZED
    };
    api_error(status, "access_denied", error.to_string())
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
            let rendered = render_workspace(std::path::Path::new(&book.workspace_path));
            let chapter_count = rendered
                .as_ref()
                .map(|rendered| rendered.chapters.len())
                .unwrap_or(0);
            let render_status = if rendered.is_ok() {
                revision
                    .as_ref()
                    .map(|revision| revision.render_status.clone())
                    .unwrap_or(RevisionRenderStatus::Ready)
            } else {
                RevisionRenderStatus::Failed
            };
            let summary = ReaderSummary {
                book_id: book.book_id,
                title: book.title,
                subtitle: "Draft in progress".to_string(),
                language: read_book_language(std::path::Path::new(&book.workspace_path)),
                status: BookStatus::Active,
                last_revision_id: revision.map(|revision| revision.revision_id),
                last_updated_at: book.updated_at,
                render_status,
                chapter_count,
            };
            (StatusCode::OK, Json(summary)).into_response()
        }
        Err(error) => reader_access_error(error),
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
                Ok((revision, rendered)) => {
                    let index =
                        match requested_chapter_index(&rendered, &query, &revision.revision_id) {
                            Ok(index) => index,
                            Err(response) => return response,
                        };
                    if let Some(chapter) = rendered.chapters.get(index) {
                        let payload = ReaderContentResponse {
                            revision_id: revision.revision_id.clone(),
                            content_hash: rendered.content_hash.clone(),
                            chapter_index: index,
                            chapter_id: chapter.id.clone(),
                            title: chapter.title.clone(),
                            source_file: chapter.source_file.clone(),
                            html: rewrite_reader_asset_urls(&chapter.html, &query.token),
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
        Err(error) => reader_access_error(error),
    }
}

pub async fn reader_asset(
    State(state): State<AppState>,
    AxumPath(asset_path): AxumPath<String>,
    Query(query): Query<TokenQuery>,
) -> Response {
    let asset_path = asset_path.trim_start_matches('/');
    match resolve_book_for_token(&state, &query.token).await {
        Ok(book) => match load_reader_asset(&book, asset_path) {
            Ok((content_type, bytes)) => {
                ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
            }
            Err(error) => api_error(StatusCode::NOT_FOUND, "asset_not_found", error.to_string()),
        },
        Err(error) => reader_access_error(error),
    }
}

fn load_reader_asset(book: &Book, asset_path: &str) -> Result<(&'static str, Vec<u8>)> {
    ensure_workspace_asset_path(asset_path)?;
    let content_type = content_type_for_asset_path(asset_path)
        .ok_or_else(|| anyhow!("unsupported reader asset type"))?;
    let path = std::path::Path::new(&book.workspace_path).join(asset_path);
    Ok((content_type, std::fs::read(path)?))
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
                let rendered = render_workspace(std::path::Path::new(&book.workspace_path));
                let payload = ReaderRevisionResponse {
                    revision_id: revision.revision_id,
                    created_at: revision.created_at,
                    source_job_id: revision.job_id,
                    summary: revision.summary.clone(),
                    render_status: if rendered.is_ok() {
                        revision.render_status.clone()
                    } else {
                        RevisionRenderStatus::Failed
                    },
                    content_hash: rendered
                        .as_ref()
                        .ok()
                        .map(|rendered| rendered.content_hash.clone()),
                    render_error: rendered.err().map(|error| error.to_string()).or_else(|| {
                        (revision.render_status == RevisionRenderStatus::Failed)
                            .then_some(revision.summary)
                    }),
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
        Err(error) => reader_access_error(error),
    }
}

fn rewrite_reader_asset_urls(html: &str, token: &str) -> String {
    let html = rewrite_reader_asset_urls_for_quote(html, token, '"');
    rewrite_reader_asset_urls_for_quote(&html, token, '\'')
}

fn rewrite_reader_asset_urls_for_quote(html: &str, token: &str, quote: char) -> String {
    let marker = format!("src={quote}assets/images/");
    let replacement = format!("src={quote}/api/reader/assets/assets/images/");
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(index) = rest.find(&marker) {
        let (before, after_before) = rest.split_at(index);
        output.push_str(before);
        output.push_str(&replacement);
        let after_marker = &after_before[marker.len()..];
        if let Some(end_index) = after_marker.find(quote) {
            let (asset_tail, after_asset) = after_marker.split_at(end_index);
            output.push_str(asset_tail);
            output.push_str("?token=");
            output.push_str(&escape_html_attr(token));
            output.push(quote);
            rest = &after_asset[quote.len_utf8()..];
        } else {
            output.push_str(after_marker);
            rest = "";
        }
    }
    output.push_str(rest);
    output
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
        Err(error) => reader_access_error(error),
    }
}

pub async fn load_latest_rendered_book(
    state: &AppState,
    book_id: &str,
    expected_revision_id: Option<&str>,
) -> std::result::Result<(Revision, RenderedBook), axum::response::Response> {
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

    let book = match state.repository.get_book(book_id).await {
        Some(book) => book,
        None => {
            return Err(api_error(
                StatusCode::NOT_FOUND,
                "book_not_found",
                "Requested book was not found.",
            ));
        }
    };

    match render_workspace(std::path::Path::new(&book.workspace_path)) {
        Ok(rendered) => Ok((revision, rendered)),
        Err(error) => Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "render_failed",
            error.to_string(),
        )),
    }
}
