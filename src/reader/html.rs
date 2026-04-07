use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};

use crate::{
    app::state::AppState, reader::handlers::resolve_book_for_token,
    storage::render_store::read_render_snapshot,
};

pub async fn reader_shell(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    match resolve_book_for_token(&state, &token).await {
        Ok(book) => {
            let revision = state
                .repository
                .latest_revision_for_book(&book.book_id)
                .await;
            let snapshot = state
                .repository
                .latest_render_snapshot_for_book(&book.book_id)
                .await;
            let html = match snapshot
                .as_ref()
                .and_then(|snapshot| read_render_snapshot(&snapshot.storage_location).ok())
            {
                Some(rendered) => render_reader_shell_html(
                    &token,
                    &rendered.title,
                    &rendered.subtitle,
                    revision
                        .as_ref()
                        .map(|revision| revision.revision_id.as_str()),
                    &rendered.full_html,
                    None,
                ),
                None => render_reader_shell_html(
                    &token,
                    &book.title,
                    "Draft in progress",
                    revision
                        .as_ref()
                        .map(|revision| revision.revision_id.as_str()),
                    "<section class=\"reader-empty\"><h2>The draft shell is ready</h2><p>No rendered manuscript content is available yet.</p></section>",
                    None,
                ),
            };
            (StatusCode::OK, Html(html)).into_response()
        }
        Err(error) => (
            StatusCode::UNAUTHORIZED,
            Html(render_reader_shell_html(
                &token,
                "Reader unavailable",
                "Signed link required",
                None,
                "",
                Some(&error.to_string()),
            )),
        )
            .into_response(),
    }
}

pub fn render_reader_shell_html(
    token: &str,
    title: &str,
    subtitle: &str,
    revision_id: Option<&str>,
    content_html: &str,
    error_message: Option<&str>,
) -> String {
    let escaped_title = escape_html(title);
    let escaped_subtitle = escape_html(subtitle);
    let escaped_token = escape_html(token);
    let revision_markup = revision_id
        .map(|revision_id| {
            format!(
                "<p class=\"reader-meta\">Revision {}</p>",
                escape_html(revision_id)
            )
        })
        .unwrap_or_default();
    let body_markup = error_message
        .map(|message| {
            format!(
                "<section class=\"reader-error\"><h2>Reader unavailable</h2><p>{}</p></section>",
                escape_html(message)
            )
        })
        .unwrap_or_else(|| content_html.to_string());

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{escaped_title} | Reader</title><style>
            body {{ margin: 0; font-family: Baskerville, Georgia, serif; color: #221814; background: linear-gradient(180deg, #efe4d2 0%, #dbc7ae 100%); }}
            .reader-shell {{ max-width: 72rem; margin: 0 auto; padding: 2rem 1.25rem 4rem; }}
            .reader-header {{ margin-bottom: 2rem; padding: 1.5rem; border: 1px solid rgba(67, 43, 25, 0.12); border-radius: 1.25rem; background: rgba(255, 249, 241, 0.92); box-shadow: 0 1.2rem 3rem rgba(56, 35, 19, 0.1); }}
            .reader-header p {{ margin: 0.35rem 0 0; color: #654f42; }}
            .reader-meta {{ text-transform: uppercase; letter-spacing: 0.12em; font-size: 0.75rem; color: #8a6039; }}
            .reader-book {{ padding: 2rem; border: 1px solid rgba(67, 43, 25, 0.12); border-radius: 1.5rem; background: rgba(255, 252, 247, 0.94); box-shadow: 0 1.2rem 3rem rgba(56, 35, 19, 0.08); }}
            .reader-book section {{ margin-bottom: 2rem; }}
            .reader-book h1, .reader-book h2, .reader-book h3 {{ line-height: 1.05; }}
            .reader-book p {{ line-height: 1.75; }}
            .reader-empty, .reader-error {{ padding: 1.5rem; border-radius: 1rem; background: rgba(239, 228, 210, 0.7); }}
        </style></head><body><main class=\"reader-shell\"><header class=\"reader-header\" data-token=\"{escaped_token}\"><p class=\"reader-meta\">Book writer chat reader</p><h1>{escaped_title}</h1><p>{escaped_subtitle}</p>{revision_markup}</header><article class=\"reader-book\">{body_markup}</article></main></body></html>"
    )
}

pub fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
