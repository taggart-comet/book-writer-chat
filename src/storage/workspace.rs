use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::core::models::{Book, BookLanguage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestContentEntry {
    pub id: String,
    pub kind: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfig {
    pub images_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestRepository {
    pub provider: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookManifest {
    pub book_id: String,
    pub conversation_key: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    pub language: String,
    pub render_profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<ManifestRepository>,
    pub content: Vec<ManifestContentEntry>,
    pub assets: AssetConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    pub theme: String,
    pub typography: BTreeMap<String, String>,
    pub layout: BTreeMap<String, String>,
    pub images: BTreeMap<String, String>,
}

pub fn workspace_dir(root: &Path, conversation_id: &str) -> PathBuf {
    root.join(slugify(conversation_id))
}

pub fn ensure_workspace(root: &Path, conversation_id: &str, book: &Book) -> Result<PathBuf> {
    ensure_workspace_with_language(root, conversation_id, book, BookLanguage::English)
}

pub fn ensure_workspace_with_language(
    root: &Path,
    conversation_id: &str,
    book: &Book,
    language: BookLanguage,
) -> Result<PathBuf> {
    let workspace = workspace_dir(root, conversation_id);
    std::fs::create_dir_all(workspace.join("assets/images"))?;
    std::fs::create_dir_all(workspace.join("content/frontmatter"))?;
    std::fs::create_dir_all(workspace.join("content/chapters"))?;
    std::fs::create_dir_all(workspace.join("content/backmatter"))?;

    let manifest = BookManifest {
        book_id: book.book_id.clone(),
        conversation_key: conversation_id.to_string(),
        title: book.title.clone(),
        subtitle: localized_subtitle(language).to_string(),
        language: language.code().to_string(),
        render_profile: "standard-book".to_string(),
        repository: None,
        content: vec![
            ManifestContentEntry {
                id: "title-page".to_string(),
                kind: "frontmatter".to_string(),
                file: "content/frontmatter/001-title-page.md".to_string(),
            },
            ManifestContentEntry {
                id: "chapter-1".to_string(),
                kind: "chapter".to_string(),
                file: "content/chapters/001-opening.md".to_string(),
            },
        ],
        assets: AssetConfig {
            images_dir: "assets/images".to_string(),
        },
    };
    let style = StyleConfig {
        theme: "classic-readable".to_string(),
        typography: BTreeMap::from([
            ("body".to_string(), "book-serif".to_string()),
            ("headings".to_string(), "editorial-serif".to_string()),
            ("scale".to_string(), "medium".to_string()),
        ]),
        layout: BTreeMap::from([
            ("page_width".to_string(), "readable".to_string()),
            ("chapter_opening".to_string(), "spacious".to_string()),
        ]),
        images: BTreeMap::from([
            ("default_alignment".to_string(), "center".to_string()),
            ("default_width".to_string(), "medium".to_string()),
            ("captions".to_string(), "enabled".to_string()),
        ]),
    };

    write_if_missing(
        &workspace.join("book.yaml"),
        &serde_yaml::to_string(&manifest)?,
    )?;
    write_if_missing(
        &workspace.join("style.yaml"),
        &serde_yaml::to_string(&style)?,
    )?;
    write_if_missing(
        &workspace.join("content/frontmatter/001-title-page.md"),
        &format!("# {}\n\n{}\n", manifest.title, manifest.subtitle),
    )?;
    write_if_missing(
        &workspace.join("content/chapters/001-opening.md"),
        localized_opening(language),
    )?;
    Ok(workspace)
}

pub fn read_manifest(workspace: &Path) -> Result<BookManifest> {
    Ok(serde_yaml::from_slice(&std::fs::read(
        workspace.join("book.yaml"),
    )?)?)
}

pub fn read_book_language(workspace: &Path) -> BookLanguage {
    read_manifest(workspace)
        .map(|manifest| BookLanguage::from_manifest_code(&manifest.language))
        .unwrap_or_default()
}

pub fn read_style(workspace: &Path) -> Result<StyleConfig> {
    Ok(serde_yaml::from_slice(&std::fs::read(
        workspace.join("style.yaml"),
    )?)?)
}

fn localized_subtitle(language: BookLanguage) -> &'static str {
    match language {
        BookLanguage::English => "Draft in progress",
        BookLanguage::Russian => "Черновик в работе",
    }
}

fn localized_opening(language: BookLanguage) -> &'static str {
    match language {
        BookLanguage::English => "# Opening\n\nThis conversation is ready for authoring.\n",
        BookLanguage::Russian => "# Начало\n\nЭта беседа готова для работы над книгой.\n",
    }
}

pub fn snapshot_workspace(workspace: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut snapshot = BTreeMap::new();
    if !workspace.exists() {
        return Ok(snapshot);
    }
    for entry in WalkDir::new(workspace).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let relative = entry
                .path()
                .strip_prefix(workspace)?
                .to_string_lossy()
                .to_string();
            snapshot.insert(relative, std::fs::read(entry.path())?);
        }
    }
    Ok(snapshot)
}

pub fn diff_workspace(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut changed = Vec::new();
    for key in before.keys().chain(after.keys()) {
        let previous = before.get(key);
        let current = after.get(key);
        if previous != current && !changed.contains(key) {
            changed.push(key.clone());
        }
    }
    changed.sort();
    changed
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    if !path.exists() {
        std::fs::write(path, contents)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::core::models::{Book, BookStatus};
    use chrono::Utc;

    #[test]
    fn creates_expected_workspace_tree() {
        let dir = tempdir().unwrap();
        let book = Book {
            book_id: "book-1".to_string(),
            conversation_id: "app:1".to_string(),
            title: "Sample".to_string(),
            status: BookStatus::Active,
            workspace_path: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let workspace = ensure_workspace(dir.path(), "app:1", &book).unwrap();
        assert!(workspace.join("book.yaml").exists());
        assert!(workspace.join("style.yaml").exists());
        assert!(workspace.join("content/frontmatter").exists());
        assert!(workspace.join("content/chapters").exists());
        assert!(workspace.join("content/backmatter").exists());
        assert!(workspace.join("assets/images").exists());
        assert!(
            workspace
                .join("content/frontmatter/001-title-page.md")
                .exists()
        );
        assert!(workspace.join("content/chapters/001-opening.md").exists());

        let manifest = read_manifest(&workspace).unwrap();
        assert_eq!(manifest.book_id, "book-1");
        assert_eq!(manifest.conversation_key, "app:1");
        assert_eq!(manifest.language, "en");
        assert_eq!(manifest.assets.images_dir, "assets/images");
        assert_eq!(manifest.content.len(), 2);
    }

    #[test]
    fn creates_russian_workspace_template() {
        let dir = tempdir().unwrap();
        let book = Book {
            book_id: "book-1".to_string(),
            conversation_id: "app:1".to_string(),
            title: "Sample".to_string(),
            status: BookStatus::Active,
            workspace_path: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let workspace =
            ensure_workspace_with_language(dir.path(), "app:1", &book, BookLanguage::Russian)
                .unwrap();

        let manifest = read_manifest(&workspace).unwrap();
        assert_eq!(manifest.language, "ru");
        assert_eq!(manifest.subtitle, "Черновик в работе");
        let opening =
            std::fs::read_to_string(workspace.join("content/chapters/001-opening.md")).unwrap();
        assert!(opening.contains("Эта беседа готова"));
    }
}
