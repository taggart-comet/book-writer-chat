use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use book_writer_chat::{
    core::{
        config::Config,
        models::{CommandKind, Provider, RevisionRenderStatus},
    },
    storage::{
        render_store::render_workspace,
        repository::Repository,
        workspace::{BookManifest, workspace_dir},
    },
};

const MOCK_CONVERSATION_ID: &str = "app:mock-demo";
const MOCK_TITLE: &str = "The Quiet Lighthouse";

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    config.ensure_directories()?;

    let repository = Repository::load(&config.data_dir).await?;
    let conversation = repository
        .resolve_or_create_conversation(
            Provider::App,
            MOCK_CONVERSATION_ID.to_string(),
            "Mock demo conversation".to_string(),
        )
        .await?;
    let workspace = workspace_dir(&config.books_root, &conversation.conversation_id);
    let book = repository
        .create_book(
            &conversation.conversation_id,
            MOCK_TITLE.to_string(),
            workspace.display().to_string(),
        )
        .await?;

    copy_template(&template_root(), &workspace)?;
    rewrite_manifest(
        &workspace.join("book.yaml"),
        &book.book_id,
        &conversation.conversation_id,
    )?;

    let session = repository
        .open_session(
            &conversation.conversation_id,
            &book.book_id,
            chrono::Utc::now(),
        )
        .await?;
    let job = repository
        .create_job(
            &book.book_id,
            &conversation.conversation_id,
            &session.session_id,
            "mock-seed",
            CommandKind::Init,
            "Seed mock book".to_string(),
        )
        .await?;
    repository
        .update_job_status(
            &job.job_id,
            book_writer_chat::core::models::JobStatus::Succeeded,
            Some("Mock book seeded for local UI checks.".to_string()),
            Some(Vec::new()),
            None,
        )
        .await?;

    render_workspace(&workspace)?;
    repository
        .create_revision(
            &book.book_id,
            &job.job_id,
            "Mock book seeded".to_string(),
            RevisionRenderStatus::Ready,
        )
        .await?;

    println!("Mock book seeded.");
    println!("Workspace: {}", workspace.display());
    let reader_base_url =
        std::env::var("FRONTEND_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:5173".to_string());
    println!(
        "Reader URL: {}/reader/{}",
        reader_base_url.trim_end_matches('/'),
        book.book_id
    );

    Ok(())
}

fn template_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mock-book")
}

fn copy_template(source_root: &Path, target_root: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(source_root) {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(source_root)
            .context("template path escaped source root")?;
        let target = target_root.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn rewrite_manifest(path: &Path, book_id: &str, conversation_key: &str) -> Result<()> {
    let mut manifest: BookManifest = serde_yaml::from_slice(
        &fs::read(path).with_context(|| format!("read {}", path.display()))?,
    )?;
    manifest.book_id = book_id.to_string();
    manifest.conversation_key = conversation_key.to_string();
    fs::write(path, serde_yaml::to_string(&manifest)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
