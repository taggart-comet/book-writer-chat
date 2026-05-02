use std::{fmt::Write as _, path::Path};

use anyhow::{Result, anyhow};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::storage::{
    media_assets::ensure_workspace_asset_path,
    workspace::{read_manifest, read_style},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedChapter {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub source_file: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedBook {
    pub title: String,
    pub subtitle: String,
    pub theme: String,
    pub chapters: Vec<RenderedChapter>,
    pub full_html: String,
    pub content_hash: String,
}

pub fn render_workspace(workspace: &Path) -> Result<RenderedBook> {
    let manifest = read_manifest(workspace)?;
    let style = read_style(workspace)?;
    if manifest.content.is_empty() {
        return Err(anyhow!("book manifest contains no content entries"));
    }

    let mut chapters = Vec::new();
    let mut full_html = String::new();
    for entry in &manifest.content {
        let markdown = std::fs::read_to_string(workspace.join(&entry.file))?;
        let html = markdown_to_html(workspace, &markdown, &entry.file)?;
        let title = first_heading(&markdown).unwrap_or_else(|| entry.id.clone());
        let chapter = RenderedChapter {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            title: title.clone(),
            source_file: entry.file.clone(),
            html: html.clone(),
        };
        let _ = write!(
            full_html,
            "<section data-kind=\"{}\" data-id=\"{}\">{}</section>",
            chapter.kind, chapter.id, chapter.html
        );
        chapters.push(chapter);
    }
    let content_hash = format!("{:x}", sha2::Sha256::digest(full_html.as_bytes()));
    Ok(RenderedBook {
        title: manifest.title,
        subtitle: manifest.subtitle,
        theme: style.theme,
        chapters,
        full_html,
        content_hash,
    })
}

fn markdown_to_html(workspace: &Path, markdown: &str, source_file: &str) -> Result<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);
    let line_index = SourceLineIndex::new(markdown);
    let mut events = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if let Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                } = tag
                {
                    let normalized_dest_url =
                        normalize_image_reference(workspace, source_file, dest_url.as_ref())?;
                    events.push(Event::Start(
                        Tag::Image {
                            link_type,
                            dest_url: CowStr::from(normalized_dest_url),
                            title,
                            id,
                        }
                        .into_static(),
                    ));
                    continue;
                }
                events.push(Event::Start(tag.into_static()));
            }
            Event::Text(text) => {
                let start = line_index.position(range.start);
                let end = line_index.position(range.end);
                events.extend([
                    Event::Html(CowStr::from(format!(
                        "<span data-source-file=\"{}\" data-source-start-line=\"{}\" data-source-start-char=\"{}\" data-source-end-line=\"{}\" data-source-end-char=\"{}\">",
                        escape_html_attr(source_file),
                        start.line,
                        start.character,
                        end.line,
                        end.character
                    ))),
                    Event::Text(text.into_static()),
                    Event::Html(CowStr::from("</span>")),
                ]);
            }
            _ => events.push(event.into_static()),
        }
    }
    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    Ok(html_output)
}

fn normalize_image_reference(
    workspace: &Path,
    source_file: &str,
    dest_url: &str,
) -> Result<String> {
    let normalized = if dest_url.starts_with("assets/images/") {
        dest_url.to_string()
    } else if let Some(trimmed) = dest_url.strip_prefix("/assets/images/") {
        format!("assets/images/{trimmed}")
    } else if let Ok(path) = resolve_workspace_relative_image_path(workspace, source_file, dest_url)
    {
        path
    } else if let Ok(path) = std::path::Path::new(dest_url).strip_prefix(workspace) {
        path.to_string_lossy().replace('\\', "/")
    } else {
        return Ok(dest_url.to_string());
    };

    ensure_workspace_asset_path(&normalized)?;
    let image_path = workspace.join(&normalized);
    if !image_path.exists() {
        return Err(anyhow!("image reference `{dest_url}` does not exist"));
    }
    Ok(normalized)
}

fn resolve_workspace_relative_image_path(
    workspace: &Path,
    source_file: &str,
    dest_url: &str,
) -> Result<String> {
    let canonical_workspace = std::fs::canonicalize(workspace)?;
    let source_dir = workspace.join(source_file);
    let joined = source_dir.parent().unwrap_or(workspace).join(dest_url);
    let canonical_image_path = std::fs::canonicalize(joined)?;
    Ok(canonical_image_path
        .strip_prefix(canonical_workspace)?
        .to_string_lossy()
        .replace('\\', "/"))
}

#[derive(Debug, Clone, Copy)]
struct SourcePosition {
    line: usize,
    character: usize,
}

struct SourceLineIndex<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceLineIndex<'a> {
    fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self {
            source,
            line_starts,
        }
    }

    fn position(&self, byte_offset: usize) -> SourcePosition {
        let line_index = self
            .line_starts
            .partition_point(|line_start| *line_start <= byte_offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line_index];
        let character = self.source[line_start..byte_offset].chars().count() + 1;
        SourcePosition {
            line: line_index + 1,
            character,
        }
    }
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn first_heading(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(ToOwned::to_owned))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::core::models::{Book, BookStatus};
    use crate::storage::workspace::ensure_workspace;
    use chrono::Utc;

    #[test]
    fn render_is_deterministic() {
        let dir = tempdir().unwrap();
        let book = Book {
            book_id: "book-1".to_string(),
            conversation_id: "app:1".to_string(),
            title: "Deterministic".to_string(),
            status: BookStatus::Active,
            workspace_path: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let workspace = ensure_workspace(dir.path(), "app:1", &book).unwrap();
        let one = render_workspace(&workspace).unwrap();
        let two = render_workspace(&workspace).unwrap();
        assert_eq!(one.full_html, two.full_html);
        assert_eq!(one.content_hash, two.content_hash);
        assert_eq!(
            one.chapters[0].source_file,
            "content/frontmatter/001-title-page.md"
        );
        assert!(one.full_html.contains("data-source-file=\"content/"));
        assert!(!one.full_html.contains("<h2>Opening</h2>"));
    }

    #[test]
    fn normalizes_workspace_image_references_before_rendering() {
        let dir = tempdir().unwrap();
        let book = Book {
            book_id: "book-1".to_string(),
            conversation_id: "app:1".to_string(),
            title: "Images".to_string(),
            status: BookStatus::Active,
            workspace_path: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let workspace = ensure_workspace(dir.path(), "app:1", &book).unwrap();
        let image_path = workspace.join("assets/images/example.png");
        std::fs::write(&image_path, [137, 80, 78, 71]).unwrap();

        let absolute_html = markdown_to_html(
            &workspace,
            &format!("![alt]({})", image_path.display()),
            "content/chapters/001-opening.md",
        )
        .unwrap();
        assert!(absolute_html.contains("src=\"assets/images/example.png\""));

        let root_relative_html = markdown_to_html(
            &workspace,
            "![alt](/assets/images/example.png)",
            "content/chapters/001-opening.md",
        )
        .unwrap();
        assert!(root_relative_html.contains("src=\"assets/images/example.png\""));

        let chapter_relative_html = markdown_to_html(
            &workspace,
            "![alt](../../assets/images/example.png)",
            "content/chapters/001-opening.md",
        )
        .unwrap();
        assert!(chapter_relative_html.contains("src=\"assets/images/example.png\""));
    }
}
