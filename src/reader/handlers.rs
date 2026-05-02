use anyhow::{Result, anyhow};
use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::{
    app::{errors::api_error, state::AppState},
    core::models::{
        Book, BookStatus, ReaderContentResponse, ReaderJobResponse, ReaderRevisionResponse,
        ReaderSummary, Revision, RevisionRenderStatus,
    },
    reader::content::{ChapterCursor, ContentQuery, encode_cursor, requested_chapter_index},
    storage::{
        media_assets::{content_type_for_asset_path, ensure_workspace_asset_path},
        render_store::{RenderedBook, render_workspace},
        web_books::find_book_workspace,
        workspace::read_book_language,
    },
};

pub async fn resolve_book_for_reader(state: &AppState, route_book_id: &str) -> Result<Book> {
    if let Some(book) = state.repository.get_book(route_book_id).await {
        return Ok(book);
    }

    let Some(workspace) = find_book_workspace(&state.config.books_root, route_book_id)? else {
        return Err(anyhow!("book not found"));
    };

    if let Some(book) = state
        .repository
        .find_book_by_workspace_path(&workspace.workspace_path)
        .await
    {
        return Ok(book);
    }

    Ok(Book {
        book_id: workspace.book_id,
        conversation_id: String::new(),
        title: workspace.title,
        status: BookStatus::Active,
        workspace_path: workspace.workspace_path.display().to_string(),
        created_at: workspace.created_at,
        updated_at: workspace.updated_at,
    })
}

fn reader_access_error(error: anyhow::Error) -> axum::response::Response {
    api_error(StatusCode::NOT_FOUND, "book_not_found", error.to_string())
}

pub async fn reader_summary(
    AxumPath(book_id): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match resolve_book_for_reader(&state, &book_id).await {
        Ok(book) => {
            let rendered = render_workspace(std::path::Path::new(&book.workspace_path));
            let revision = latest_reader_revision(&state, &book, rendered.as_ref().ok()).await;
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
    AxumPath(book_id): AxumPath<String>,
    State(state): State<AppState>,
    Query(query): Query<ContentQuery>,
) -> impl IntoResponse {
    match resolve_book_for_reader(&state, &book_id).await {
        Ok(book) => {
            match load_latest_rendered_book(&state, &book, query.revision_id.as_deref()).await {
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
                            html: rewrite_reader_asset_urls(&chapter.html, &book.book_id),
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
    AxumPath((book_id, asset_path)): AxumPath<(String, String)>,
) -> Response {
    let asset_path = asset_path.trim_start_matches('/');
    match resolve_book_for_reader(&state, &book_id).await {
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
    AxumPath(book_id): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match resolve_book_for_reader(&state, &book_id).await {
        Ok(book) => {
            let rendered = render_workspace(std::path::Path::new(&book.workspace_path));
            if let Some(revision) =
                latest_reader_revision(&state, &book, rendered.as_ref().ok()).await
            {
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

fn rewrite_reader_asset_urls(html: &str, book_id: &str) -> String {
    let html = rewrite_reader_asset_urls_for_quote(html, book_id, '"', false);
    let html = rewrite_reader_asset_urls_for_quote(&html, book_id, '"', true);
    let html = rewrite_reader_asset_urls_for_quote(&html, book_id, '\'', false);
    rewrite_reader_asset_urls_for_quote(&html, book_id, '\'', true)
}

fn rewrite_reader_asset_urls_for_quote(
    html: &str,
    book_id: &str,
    quote: char,
    leading_slash: bool,
) -> String {
    let marker = if leading_slash {
        format!("src={quote}/assets/images/")
    } else {
        format!("src={quote}assets/images/")
    };
    let replacement = format!(
        "src={quote}/api/reader/{}/assets/assets/images/",
        escape_html_attr(book_id)
    );
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
    AxumPath(book_id): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match resolve_book_for_reader(&state, &book_id).await {
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
    book: &Book,
    expected_revision_id: Option<&str>,
) -> std::result::Result<(Revision, RenderedBook), axum::response::Response> {
    let rendered = match render_workspace(std::path::Path::new(&book.workspace_path)) {
        Ok(rendered) => rendered,
        Err(error) => {
            return Err(api_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "render_failed",
                error.to_string(),
            ));
        }
    };

    let revision = match latest_reader_revision(state, book, Some(&rendered)).await {
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

    Ok((revision, rendered))
}

async fn latest_reader_revision(
    state: &AppState,
    book: &Book,
    rendered: Option<&RenderedBook>,
) -> Option<Revision> {
    if let Some(revision) = state
        .repository
        .latest_revision_for_book(&book.book_id)
        .await
    {
        return Some(revision);
    }

    rendered.map(|rendered| synthetic_workspace_revision(book, rendered))
}

fn synthetic_workspace_revision(book: &Book, rendered: &RenderedBook) -> Revision {
    Revision {
        revision_id: format!("workspace-{}", rendered.content_hash),
        book_id: book.book_id.clone(),
        job_id: "workspace".to_string(),
        summary: "Rendered directly from the local workspace.".to_string(),
        created_at: book.updated_at,
        render_status: RevisionRenderStatus::Ready,
    }
}

#[cfg(test)]
mod tests {
    use std::{env, sync::MutexGuard};

    use axum::{Router, body::Body, http::Request, routing::get};
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::{
        app::{
            metrics::Metrics,
            state::{AppState, SessionLaunchResult, SessionLauncher},
        },
        core::{
            config::{Config, test_env_lock},
            models::{BookLanguage, CommandKind, JobStatus, Provider, RevisionRenderStatus},
        },
        storage::{
            render_store::render_workspace, repository::Repository,
            web_books::provision_book_workspace,
        },
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

    struct NoopLauncher;

    #[async_trait::async_trait]
    impl SessionLauncher for NoopLauncher {
        async fn launch(
            &self,
            _workspace: &std::path::Path,
            _title: &str,
            _initial_prompt: &str,
        ) -> anyhow::Result<SessionLaunchResult> {
            unreachable!("reader tests should not launch Codex sessions")
        }

        async fn resume(
            &self,
            _workspace: &std::path::Path,
            _session_id: &str,
            _prompt: &str,
        ) -> anyhow::Result<SessionLaunchResult> {
            unreachable!("reader tests should not resume Codex sessions")
        }
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn test_state(temp_dir: &TempDir) -> AppState {
        configure_env(temp_dir);
        let config = Config::from_env().unwrap();
        config.ensure_directories().unwrap();
        let repository = Repository::load(&config.data_dir).await.unwrap();
        AppState {
            config,
            repository,
            metrics: Metrics::default(),
            conversation_locks: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            session_launcher: std::sync::Arc::new(NoopLauncher),
        }
    }

    #[tokio::test]
    async fn reader_summary_resolves_slug_to_repository_book() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let state = test_state(&temp_dir).await;

        let conversation = state
            .repository
            .resolve_or_create_conversation(
                Provider::Telegram,
                "chat-1".to_string(),
                "Reader preview".to_string(),
            )
            .await
            .unwrap();

        let workspace = provision_book_workspace(
            &state.config.books_root,
            "Moya Testovaya Kniga",
            BookLanguage::Russian,
        )
        .unwrap();

        let book = state
            .repository
            .create_book(
                &conversation.conversation_id,
                "Moya Testovaya Kniga".to_string(),
                workspace.workspace_path.display().to_string(),
            )
            .await
            .unwrap();

        let session = state
            .repository
            .open_session(
                &conversation.conversation_id,
                &book.book_id,
                chrono::Utc::now(),
            )
            .await
            .unwrap();

        let job = state
            .repository
            .create_job(
                &book.book_id,
                &conversation.conversation_id,
                &session.session_id,
                "message-1",
                CommandKind::Authoring,
                "Write a chapter".to_string(),
            )
            .await
            .unwrap();

        state
            .repository
            .update_job_status(
                &job.job_id,
                JobStatus::Succeeded,
                Some("Done".to_string()),
                Some(Vec::new()),
                None,
            )
            .await
            .unwrap();

        render_workspace(&workspace.workspace_path).unwrap();
        let revision = state
            .repository
            .create_revision(
                &book.book_id,
                &job.job_id,
                "Rendered".to_string(),
                RevisionRenderStatus::Ready,
            )
            .await
            .unwrap();

        let router = Router::new()
            .route("/api/reader/:book_id/summary", get(reader_summary))
            .with_state(state.clone());

        let response = router
            .oneshot(
                Request::get(format!("/api/reader/{}/summary", workspace.slug))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["book_id"], book.book_id);
        assert_eq!(payload["title"], "Moya Testovaya Kniga");
        assert_eq!(payload["last_revision_id"], revision.revision_id);
    }

    #[tokio::test]
    async fn reader_resolver_supports_workspace_only_books() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let state = test_state(&temp_dir).await;

        let workspace = provision_book_workspace(
            &state.config.books_root,
            "Quiet Lighthouse",
            BookLanguage::English,
        )
        .unwrap();

        let book = resolve_book_for_reader(&state, &workspace.slug)
            .await
            .unwrap();

        assert_eq!(book.book_id, workspace.book_id);
        assert_eq!(book.title, workspace.title);
        assert_eq!(
            std::path::Path::new(&book.workspace_path),
            workspace.workspace_path.as_path()
        );
    }

    #[tokio::test]
    async fn reader_summary_synthesizes_revision_for_workspace_only_book() {
        let _guard = env_lock();
        let temp_dir = TempDir::new().unwrap();
        let state = test_state(&temp_dir).await;

        let workspace = provision_book_workspace(
            &state.config.books_root,
            "Quiet Lighthouse",
            BookLanguage::English,
        )
        .unwrap();

        let router = Router::new()
            .route("/api/reader/:book_id/summary", get(reader_summary))
            .route("/api/reader/:book_id/revision", get(reader_revision))
            .route("/api/reader/:book_id/content", get(reader_content))
            .with_state(state.clone());

        let summary_response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/reader/{}/summary", workspace.slug))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(summary_response.status(), StatusCode::OK);
        let summary_payload = response_json(summary_response).await;
        let revision_id = summary_payload["last_revision_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(revision_id.starts_with("workspace-"));
        assert_eq!(summary_payload["chapter_count"], 2);

        let revision_response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/reader/{}/revision", workspace.slug))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(revision_response.status(), StatusCode::OK);
        let revision_payload = response_json(revision_response).await;
        assert_eq!(revision_payload["revision_id"], revision_id);
        assert_eq!(revision_payload["source_job_id"], "workspace");

        let content_response = router
            .oneshot(
                Request::get(format!(
                    "/api/reader/{}/content?revision_id={}",
                    workspace.slug, revision_id
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(content_response.status(), StatusCode::OK);
        let content_payload = response_json(content_response).await;
        assert_eq!(content_payload["revision_id"], revision_id);
    }
}
