use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use axum::{
    Router,
    http::Uri,
    response::Html,
    routing::{get, post},
};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

use crate::{
    app::{metrics::Metrics, state::AppState},
    authoring::executor::RealExecutor,
    core::config::Config,
    messaging::handlers::{max_webhook, telegram_webhook},
    reader::{
        handlers::{reader_content, reader_job, reader_revision, reader_summary},
        html::reader_shell,
    },
    storage::repository::Repository,
};

pub async fn build_router(
    config: Config,
    executor: Option<crate::authoring::executor::DynExecutor>,
) -> Result<Router> {
    config.ensure_directories()?;
    let repository = Repository::load(&config.data_dir).await?;
    let executor = executor.unwrap_or_else(|| Arc::new(RealExecutor::new(config.clone())));
    let state = AppState {
        config: config.clone(),
        repository,
        executor,
        metrics: Metrics::default(),
        conversation_locks: Arc::new(Mutex::new(HashMap::new())),
    };

    let mut router = Router::new()
        .route("/api/healthz", get(health))
        .route("/api/readyz", get(ready))
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/metrics", get(metrics))
        .route("/api/messages/telegram", post(telegram_webhook))
        .route("/api/messages/max", post(max_webhook))
        .route("/api/reader/summary", get(reader_summary))
        .route("/api/reader/content", get(reader_content))
        .route("/api/reader/revision", get(reader_revision))
        .route("/api/reader/job", get(reader_job))
        .route("/reader/:token", get(reader_shell))
        .with_state(state);

    if config.frontend_dist_dir.exists() {
        router = router.fallback_service(ServeDir::new(config.frontend_dist_dir));
    } else {
        router = router.fallback(fallback_reader);
    }
    Ok(router)
}

pub async fn health() -> &'static str {
    "ok"
}

pub async fn ready(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::http::StatusCode {
    match state.config.ensure_directories() {
        Ok(_) => axum::http::StatusCode::OK,
        Err(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
    }
}

pub async fn metrics(axum::extract::State(state): axum::extract::State<AppState>) -> String {
    state.metrics.render()
}

pub async fn fallback_reader(uri: Uri) -> Html<String> {
    let html = format!(
        "<!doctype html><html><body><main><h1>Book Writer Chat</h1><p>No built frontend was found.</p><p>Requested route: {}</p></main></body></html>",
        uri.path()
    );
    Html(html)
}
