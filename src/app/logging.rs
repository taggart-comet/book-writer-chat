use std::time::Instant;

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

pub async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();
    let response = next.run(request).await;
    let status = response.status();
    let elapsed_ms = start.elapsed().as_millis() as u64;

    if status.is_server_error() {
        tracing::error!(
            method = %method,
            path = %uri.path(),
            query = uri.query().unwrap_or(""),
            status = status.as_u16(),
            elapsed_ms,
            "request failed"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            method = %method,
            path = %uri.path(),
            query = uri.query().unwrap_or(""),
            status = status.as_u16(),
            elapsed_ms,
            "request completed with client error"
        );
    } else {
        tracing::info!(
            method = %method,
            path = %uri.path(),
            query = uri.query().unwrap_or(""),
            status = status.as_u16(),
            elapsed_ms,
            "request completed"
        );
    }

    response
}
