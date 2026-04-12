use anyhow::Result;

use crate::{
    core::models::{Book, BookLanguage, NormalizedMessage},
    storage::media_assets::SavedImageAttachment,
    storage::workspace::read_manifest,
};

pub fn build_prompt(
    workspace: &std::path::Path,
    book: &Book,
    instruction: &str,
    message: &NormalizedMessage,
    saved_images: &[SavedImageAttachment],
) -> Result<String> {
    let manifest = read_manifest(workspace)?;
    let content_entries = manifest
        .content
        .iter()
        .map(|entry| format!("- {} [{}] -> {}", entry.id, entry.kind, entry.file))
        .collect::<Vec<_>>()
        .join("\n");
    let language = BookLanguage::from_manifest_code(&manifest.language);
    let image_context = image_context(saved_images);
    Ok(format!(
        "Prompt package for Codex CLI authoring job\n\nSystem constraints:\n- Only modify files inside this workspace: {}.\n- Never read from, write to, or reference files outside this workspace.\n- Keep manuscript prose in Markdown.\n- Preserve valid YAML in workspace config files.\n- If you add or remove manuscript files, update `book.yaml` to match.\n- Make the smallest set of file changes needed for the request.\n- Communicate with the author in {} and keep new manuscript prose in {} unless the author explicitly asks for quoted text in another language.\n\nCurrent book metadata summary:\n- Book id: {}\n- Conversation id: {}\n- Title: {}\n- Subtitle: {}\n- Language: {}\n- Render profile: {}\n\nCurrent manuscript structure summary:\n{}\n\nRecent conversation summary:\n- Latest author message id: {}\n- Latest author display name: {}\n- No additional conversation summary is available in this MVP build.\n\nNormalized user instruction:\n{}\n\n{}Output constraints:\n- Apply edits directly in the workspace.\n- Do not emit patch text or prose pretending to have edited files.\n- Leave the workspace in a readable state for rendering.\n",
        workspace.display(),
        language.display_name(),
        language.display_name(),
        book.book_id,
        book.conversation_id,
        manifest.title,
        manifest.subtitle,
        manifest.language,
        manifest.render_profile,
        content_entries,
        message.message_id,
        message.sender_display_name,
        instruction.trim(),
        image_context
    ))
}

fn image_context(saved_images: &[SavedImageAttachment]) -> String {
    if saved_images.is_empty() {
        return "Available image attachments:\n- None\n\n".to_string();
    }
    let images = saved_images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            format!(
                "- Image {}: path `{}`; markdown `{}`; MIME {}; bytes {}; original filename {}; dimensions {}; caption {}",
                index + 1,
                image.workspace_relative_path,
                image.markdown,
                image.mime_type,
                image.file_size,
                image.original_filename.as_deref().unwrap_or("none"),
                image
                    .width
                    .zip(image.height)
                    .map(|(width, height)| format!("{width}x{height}"))
                    .unwrap_or_else(|| "unknown".to_string()),
                image.caption.as_deref().unwrap_or("none")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Available image attachments:\n{images}\nUse these workspace-relative paths when placing images in Markdown. Do not use messenger API URLs or external file URLs for these attachments.\n\n"
    )
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::tempdir;

    use super::*;
    use crate::{
        core::models::{Book, BookStatus, NormalizedMessage, Provider},
        storage::workspace::ensure_workspace,
    };

    #[test]
    fn prompt_includes_required_context() {
        let dir = tempdir().unwrap();
        let book = Book {
            book_id: "book-1".to_string(),
            conversation_id: "telegram:1".to_string(),
            title: "Prompt Test".to_string(),
            status: BookStatus::Active,
            workspace_path: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let workspace = ensure_workspace(dir.path(), "telegram:1", &book).unwrap();
        let message = NormalizedMessage {
            provider: Provider::Telegram,
            provider_chat_id: "telegram:1".to_string(),
            message_id: "1".to_string(),
            timestamp: Utc::now(),
            raw_text: "@bookbot write a better opening chapter".to_string(),
            attachments: Vec::new(),
            mentions_bot: true,
            sender_display_name: "Alice".to_string(),
        };
        let prompt = build_prompt(
            &workspace,
            &book,
            "write a better opening chapter",
            &message,
            &[],
        )
        .unwrap();
        assert!(prompt.contains("Prompt package for Codex CLI authoring job"));
        assert!(prompt.contains("Only modify files inside this workspace"));
        assert!(prompt.contains("Communicate with the author in English"));
        assert!(prompt.contains("Normalized user instruction:\nwrite a better opening chapter"));
        assert!(prompt.contains("Current manuscript structure summary"));
        assert!(prompt.contains("No additional conversation summary is available"));
        assert!(!prompt.contains("@bookbot write a better opening chapter"));
        assert!(!prompt.contains("unrelated secret"));
    }
}
