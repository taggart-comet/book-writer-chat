use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::storage::workspace::{read_manifest, read_style};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedChapter {
    pub id: String,
    pub kind: String,
    pub title: String,
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

pub fn render_snapshot_path(data_dir: &Path, revision_id: &str) -> PathBuf {
    data_dir
        .join("render-snapshots")
        .join(format!("{revision_id}.json"))
}

pub fn write_render_snapshot(
    data_dir: &Path,
    revision_id: &str,
    rendered: &RenderedBook,
) -> Result<String> {
    let path = render_snapshot_path(data_dir, revision_id);
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("render snapshot path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(&path, serde_json::to_vec_pretty(rendered)?)?;
    Ok(path.display().to_string())
}

pub fn read_render_snapshot(storage_location: &str) -> Result<RenderedBook> {
    Ok(serde_json::from_slice(&std::fs::read(storage_location)?)?)
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
        let html = markdown_to_html(&markdown);
        let title = first_heading(&markdown).unwrap_or_else(|| entry.id.clone());
        let chapter = RenderedChapter {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            title: title.clone(),
            html: html.clone(),
        };
        let _ = write!(
            full_html,
            "<section data-kind=\"{}\" data-id=\"{}\"><h2>{}</h2>{}</section>",
            chapter.kind, chapter.id, chapter.title, chapter.html
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

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
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
            conversation_id: "telegram:1".to_string(),
            title: "Deterministic".to_string(),
            status: BookStatus::Active,
            workspace_path: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let workspace = ensure_workspace(dir.path(), "telegram:1", &book).unwrap();
        let one = render_workspace(&workspace).unwrap();
        let two = render_workspace(&workspace).unwrap();
        assert_eq!(one.full_html, two.full_html);
        assert_eq!(one.content_hash, two.content_hash);
    }
}
