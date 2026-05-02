use anyhow::Result;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{app::errors::api_error, storage::render_store::RenderedBook};

#[derive(Debug, Deserialize)]
pub struct ContentQuery {
    pub cursor: Option<String>,
    pub chapter_id: Option<String>,
    pub revision_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChapterCursor {
    pub revision_id: String,
    pub chapter_index: usize,
}

pub fn requested_chapter_index(
    rendered: &RenderedBook,
    query: &ContentQuery,
    current_revision_id: &str,
) -> std::result::Result<usize, axum::response::Response> {
    if let Some(cursor) = &query.cursor {
        let cursor = decode_cursor(cursor).map_err(|error| {
            api_error(StatusCode::BAD_REQUEST, "invalid_cursor", error.to_string())
        })?;
        if cursor.revision_id != current_revision_id {
            return Err(api_error(
                StatusCode::CONFLICT,
                "stale_revision",
                "The requested cursor belongs to an older revision.",
            ));
        }
        return Ok(cursor.chapter_index);
    }

    if let Some(chapter_id) = &query.chapter_id {
        return rendered
            .chapters
            .iter()
            .position(|chapter| chapter.id == *chapter_id)
            .ok_or_else(|| {
                api_error(
                    StatusCode::NOT_FOUND,
                    "chapter_not_found",
                    "Requested chapter was not found.",
                )
            });
    }

    Ok(0)
}

pub fn encode_cursor(cursor: &ChapterCursor) -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).expect("cursor serialization should succeed"))
}

pub fn decode_cursor(cursor: &str) -> Result<ChapterCursor> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    Ok(serde_json::from_slice(&URL_SAFE_NO_PAD.decode(cursor)?)?)
}
