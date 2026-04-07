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
