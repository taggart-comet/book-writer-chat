use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::core::models::ReaderErrorResponse;

pub fn api_error(
    status: StatusCode,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(ReaderErrorResponse {
            code: code.into(),
            message: message.into(),
        }),
    )
        .into_response()
}

pub fn internal_api_error(
    operation: &'static str,
    error: &dyn std::fmt::Display,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Response {
    tracing::error!(operation, error = %error, "request handling failed");
    api_error(StatusCode::INTERNAL_SERVER_ERROR, code, message)
}
