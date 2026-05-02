use std::path::{Component, Path};

use anyhow::{Result, anyhow};

use crate::core::models::{MessageAttachment, Provider};

const IMAGE_ATTACHMENT_BYTES_LIMIT: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DownloadedMedia {
    pub bytes: Vec<u8>,
    pub mime_type: Option<String>,
    pub provider_file_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SavedImageAttachment {
    pub workspace_relative_path: String,
    pub markdown: String,
    pub mime_type: String,
    pub original_filename: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub file_size: usize,
    pub caption: Option<String>,
}

pub fn save_image_attachment(
    workspace: &Path,
    provider: &Provider,
    message_id: &str,
    index: usize,
    attachment: &MessageAttachment,
    media: DownloadedMedia,
) -> Result<SavedImageAttachment> {
    if media.bytes.len() > IMAGE_ATTACHMENT_BYTES_LIMIT {
        return Err(anyhow!("image attachment exceeds the 20 MB limit"));
    }
    let mime_type = media
        .mime_type
        .clone()
        .or_else(|| attachment.mime_type.clone())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let extension = image_extension(
        Some(mime_type.as_str()),
        media.provider_file_path.as_deref(),
        attachment.original_filename.as_deref(),
    )?;
    let safe_message_id = sanitize_component(message_id);
    let filename = format!(
        "{}-{}-{}.{}",
        provider.as_str(),
        safe_message_id,
        index + 1,
        extension
    );
    let relative_path = format!("assets/images/{filename}");
    ensure_workspace_asset_path(&relative_path)?;
    let output_path = workspace.join(&relative_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, &media.bytes)?;
    let alt_text = attachment
        .caption
        .as_deref()
        .map(strip_bot_address)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Book image");
    Ok(SavedImageAttachment {
        workspace_relative_path: relative_path.clone(),
        markdown: format!("![{}]({})", escape_markdown_alt(alt_text), relative_path),
        mime_type,
        original_filename: attachment.original_filename.clone(),
        width: attachment.width,
        height: attachment.height,
        file_size: media.bytes.len(),
        caption: attachment.caption.clone(),
    })
}

pub fn ensure_workspace_asset_path(relative_path: &str) -> Result<()> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(anyhow!("asset path must be relative"));
    }
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(value)) if value == "assets")
        || !matches!(components.next(), Some(Component::Normal(value)) if value == "images")
    {
        return Err(anyhow!("asset path must stay under assets/images"));
    }
    for component in components {
        match component {
            Component::Normal(_) => {}
            _ => return Err(anyhow!("asset path contains an unsafe component")),
        }
    }
    Ok(())
}

pub fn content_type_for_asset_path(relative_path: &str) -> Option<&'static str> {
    match Path::new(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("png") => Some("image/png"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

fn image_extension(
    mime_type: Option<&str>,
    provider_file_path: Option<&str>,
    original_filename: Option<&str>,
) -> Result<&'static str> {
    if let Some(extension) = match mime_type.map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("image/jpeg" | "image/jpg") => Some("jpg"),
        Some("image/png") => Some("png"),
        Some("image/gif") => Some("gif"),
        Some("image/webp") => Some("webp"),
        Some(_) => return Err(anyhow!("unsupported image MIME type for attachment")),
        None => None,
    } {
        return Ok(extension);
    }
    if let Some(extension) = provider_file_path
        .and_then(extension_from_path)
        .or_else(|| original_filename.and_then(extension_from_path))
        .and_then(normalize_image_extension)
    {
        return Ok(extension);
    }
    Err(anyhow!("unsupported image MIME type for attachment"))
}

fn extension_from_path(path: &str) -> Option<&str> {
    Path::new(path)
        .file_name()
        .and_then(|filename| Path::new(filename).extension())
        .and_then(|extension| extension.to_str())
}

fn normalize_image_extension(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("jpg"),
        "png" => Some("png"),
        "gif" => Some("gif"),
        "webp" => Some("webp"),
        _ => None,
    }
}

fn sanitize_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "message".to_string()
    } else {
        sanitized
    }
}

fn strip_bot_address(value: &str) -> &str {
    value
        .strip_prefix("@assistant")
        .or_else(|| value.strip_prefix("/assistant"))
        .unwrap_or(value)
}

fn escape_markdown_alt(value: &str) -> String {
    value.replace('[', "\\[").replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::tempdir;

    use super::*;
    use crate::core::models::{MessageAttachmentKind, Provider};

    fn attachment(mime_type: &str) -> MessageAttachment {
        MessageAttachment {
            kind: MessageAttachmentKind::Image,
            provider_file_id: "file-1".to_string(),
            provider_unique_id: Some("unique-1".to_string()),
            original_filename: Some("../unsafe.png".to_string()),
            mime_type: Some(mime_type.to_string()),
            width: Some(640),
            height: Some(480),
            file_size: Some(4),
            caption: Some("@assistant place this diagram".to_string()),
        }
    }

    #[test]
    fn saves_image_under_assets_images_with_safe_name() {
        let dir = tempdir().unwrap();
        let saved = save_image_attachment(
            dir.path(),
            &Provider::App,
            "../message:1",
            0,
            &attachment("image/png"),
            DownloadedMedia {
                bytes: vec![1, 2, 3, 4],
                mime_type: Some("image/png".to_string()),
                provider_file_path: Some("photos/original.png".to_string()),
            },
        )
        .unwrap();

        assert_eq!(
            saved.workspace_relative_path,
            "assets/images/app-message-1-1.png"
        );
        assert!(dir.path().join(&saved.workspace_relative_path).exists());
        assert_eq!(
            std::fs::read(dir.path().join(&saved.workspace_relative_path)).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert!(saved.markdown.contains("assets/images/app-message-1-1.png"));
    }

    #[test]
    fn rejects_unsafe_asset_paths() {
        assert!(ensure_workspace_asset_path("assets/images/photo.png").is_ok());
        assert!(ensure_workspace_asset_path("assets/../secret.png").is_err());
        assert!(ensure_workspace_asset_path("/assets/images/photo.png").is_err());
        assert!(ensure_workspace_asset_path("content/photo.png").is_err());
    }

    #[test]
    fn rejects_unsupported_image_mime() {
        let dir = tempdir().unwrap();
        let result = save_image_attachment(
            dir.path(),
            &Provider::App,
            &Utc::now().timestamp().to_string(),
            0,
            &attachment("image/svg+xml"),
            DownloadedMedia {
                bytes: vec![1, 2, 3, 4],
                mime_type: Some("image/svg+xml".to_string()),
                provider_file_path: None,
            },
        );

        assert!(result.is_err());
    }
}
