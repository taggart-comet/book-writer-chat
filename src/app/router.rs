use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use axum::{
    Router,
    http::{HeaderValue, Method, Uri},
    response::Html,
    routing::get,
};
use tokio::sync::Mutex;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

use crate::{
    app::{
        auth,
        metrics::Metrics,
        state::{AppState, RealSessionLauncher},
        web_books, web_conversations,
    },
    core::config::Config,
    reader::handlers::{reader_asset, reader_content, reader_job, reader_revision, reader_summary},
    storage::repository::Repository,
};

pub async fn build_router(config: Config) -> Result<Router> {
    config.ensure_directories()?;
    let repository = Repository::load(&config.data_dir).await?;
    let state = AppState {
        config: config.clone(),
        repository,
        metrics: Metrics::default(),
        conversation_locks: Arc::new(Mutex::new(HashMap::new())),
        session_launcher: Arc::new(RealSessionLauncher::new(config.clone())),
    };

    let cors = HeaderValue::from_str(&config.frontend_base_url)
        .map(|origin| {
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any)
        })
        .unwrap_or_else(|_| CorsLayer::new());

    let mut router = Router::new()
        .route("/api/healthz", get(health))
        .route("/api/readyz", get(ready))
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/metrics", get(metrics))
        .nest("/api", auth::routes())
        .nest("/api", web_books::routes())
        .nest("/api", web_conversations::routes())
        .route("/api/reader/:book_id/summary", get(reader_summary))
        .route("/api/reader/:book_id/content", get(reader_content))
        .route("/api/reader/:book_id/assets/*asset_path", get(reader_asset))
        .route("/api/reader/:book_id/revision", get(reader_revision))
        .route("/api/reader/:book_id/job", get(reader_job))
        .layer(cors)
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
