use anyhow::Result;

use crate::{
    core::models::{Book, NormalizedMessage},
    storage::workspace::read_manifest,
};

pub fn build_prompt(
    workspace: &std::path::Path,
    book: &Book,
    instruction: &str,
    message: &NormalizedMessage,
) -> Result<String> {
    let manifest = read_manifest(workspace)?;
    let content_entries = manifest
        .content
        .iter()
        .map(|entry| format!("- {} [{}] -> {}", entry.id, entry.kind, entry.file))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "Prompt package for Codex CLI authoring job\n\nSystem constraints:\n- Only modify files inside this workspace: {}.\n- Never read from, write to, or reference files outside this workspace.\n- Keep manuscript prose in Markdown.\n- Preserve valid YAML in workspace config files.\n- If you add or remove manuscript files, update `book.yaml` to match.\n- Make the smallest set of file changes needed for the request.\n\nCurrent book metadata summary:\n- Book id: {}\n- Conversation id: {}\n- Title: {}\n- Subtitle: {}\n- Language: {}\n- Render profile: {}\n\nCurrent manuscript structure summary:\n{}\n\nRecent conversation summary:\n- Latest author message id: {}\n- Latest author display name: {}\n- No additional conversation summary is available in this MVP build.\n\nNormalized user instruction:\n{}\n\nOutput constraints:\n- Apply edits directly in the workspace.\n- Do not emit patch text or prose pretending to have edited files.\n- Leave the workspace in a readable state for rendering.\n",
        workspace.display(),
        book.book_id,
        book.conversation_id,
        manifest.title,
        manifest.subtitle,
        manifest.language,
        manifest.render_profile,
        content_entries,
        message.message_id,
        message.sender_display_name,
        instruction.trim()
    ))
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
        )
        .unwrap();
        assert!(prompt.contains("Prompt package for Codex CLI authoring job"));
        assert!(prompt.contains("Only modify files inside this workspace"));
        assert!(prompt.contains("Normalized user instruction:\nwrite a better opening chapter"));
        assert!(prompt.contains("Current manuscript structure summary"));
        assert!(prompt.contains("No additional conversation summary is available"));
        assert!(!prompt.contains("@bookbot write a better opening chapter"));
        assert!(!prompt.contains("unrelated secret"));
    }
}
