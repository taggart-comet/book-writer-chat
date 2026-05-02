use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    core::models::BookLanguage,
    storage::workspace::{AssetConfig, BookManifest, ManifestContentEntry, StyleConfig},
};

const CONVERSATION_REGISTRY_VERSION: u32 = 1;
const WEB_MESSENGER_BOOTSTRAP_PROMPT: &str = "Initialize a new web messenger conversation for this book workspace. Reply with exactly: Session ready.";
const WEB_MESSENGER_BOOTSTRAP_REPLY: &str = "Session ready.";
const ENVIRONMENT_CONTEXT_START: &str = "<environment_context>";
const ENVIRONMENT_CONTEXT_END: &str = "</environment_context>";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationRegistry {
    pub version: u32,
    pub book_id: String,
    pub conversations: Vec<ConversationRegistryRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationRegistryRecord {
    pub conversation_id: String,
    pub book_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub session_log_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedTranscriptMessage {
    pub message_id: String,
    pub role: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(skip)]
    source: TranscriptMessageSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationTranscriptSnapshot {
    pub messages: Vec<NormalizedTranscriptMessage>,
    pub session_title: Option<String>,
    pub last_comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookWorkspace {
    pub book_id: String,
    pub slug: String,
    pub title: String,
    pub subtitle: String,
    pub language: BookLanguage,
    pub workspace_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum TranscriptMessageSource {
    EventMessage,
    ResponseItem,
    #[default]
    Unknown,
}

#[derive(Debug, Error)]
pub enum BookWorkspaceError {
    #[error("book title must not be empty")]
    InvalidTitle,
    #[error("book title does not produce a safe slug")]
    InvalidSlug,
    #[error("book workspace already exists for slug `{slug}`")]
    DuplicateSlug { slug: String },
    #[error("book workspace path escaped books root")]
    PathEscape,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ConversationRegistryError {
    #[error("conversation `{conversation_id}` already exists")]
    DuplicateConversation { conversation_id: String },
    #[error("conversation `{conversation_id}` not found")]
    ConversationNotFound { conversation_id: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum TranscriptReadError {
    #[error("conversation `{conversation_id}` not found")]
    ConversationNotFound { conversation_id: String },
    #[error("session log path is invalid")]
    InvalidSessionLogPath,
    #[error("session log file is missing")]
    SessionLogMissing,
    #[error("session log is malformed at line {line}")]
    MalformedLogLine { line: usize },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn list_book_workspaces(root: &Path) -> Result<Vec<BookWorkspace>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut books = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let workspace_path = entry.path();
        let slug = entry.file_name().to_string_lossy().to_string();
        let manifest = crate::storage::workspace::read_manifest(&workspace_path)
            .with_context(|| format!("read manifest for book `{slug}`"))?;
        let _registry = read_conversation_registry(&workspace_path)
            .with_context(|| format!("read conversation registry for book `{slug}`"))?;
        let metadata = fs::metadata(&workspace_path)?;
        let created_at = metadata
            .created()
            .or_else(|_| metadata.modified())
            .map(system_time_to_utc)
            .unwrap_or_else(|_| Utc::now());
        let updated_at = metadata
            .modified()
            .map(system_time_to_utc)
            .unwrap_or(created_at);
        books.push(BookWorkspace {
            book_id: manifest.book_id.clone(),
            slug,
            title: manifest.title.clone(),
            subtitle: manifest.subtitle.clone(),
            language: BookLanguage::from_manifest_code(&manifest.language),
            workspace_path,
            created_at,
            updated_at,
        });
    }

    books.sort_by(|left, right| left.slug.cmp(&right.slug));
    Ok(books)
}

pub fn find_book_workspace(root: &Path, book_id: &str) -> Result<Option<BookWorkspace>> {
    Ok(list_book_workspaces(root)?
        .into_iter()
        .find(|workspace| workspace.book_id == book_id || workspace.slug == book_id))
}

pub fn provision_book_workspace(
    root: &Path,
    title: &str,
    language: BookLanguage,
) -> Result<BookWorkspace, BookWorkspaceError> {
    let normalized_title = normalize_title(title).ok_or(BookWorkspaceError::InvalidTitle)?;
    let slug = slugify_book_title(&normalized_title).ok_or(BookWorkspaceError::InvalidSlug)?;
    let workspace_path = validated_workspace_path(root, &slug)?;
    if workspace_path.exists() {
        return Err(BookWorkspaceError::DuplicateSlug { slug });
    }

    fs::create_dir_all(workspace_path.join("assets/images"))?;
    fs::create_dir_all(workspace_path.join("content/frontmatter"))?;
    fs::create_dir_all(workspace_path.join("content/chapters"))?;
    fs::create_dir_all(workspace_path.join("content/backmatter"))?;

    let manifest = BookManifest {
        book_id: slug.clone(),
        conversation_key: slug.clone(),
        title: normalized_title.to_string(),
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
        typography: [
            ("body".to_string(), "book-serif".to_string()),
            ("headings".to_string(), "editorial-serif".to_string()),
            ("scale".to_string(), "medium".to_string()),
        ]
        .into_iter()
        .collect(),
        layout: [
            ("page_width".to_string(), "readable".to_string()),
            ("chapter_opening".to_string(), "spacious".to_string()),
        ]
        .into_iter()
        .collect(),
        images: [
            ("default_alignment".to_string(), "center".to_string()),
            ("default_width".to_string(), "medium".to_string()),
            ("captions".to_string(), "enabled".to_string()),
        ]
        .into_iter()
        .collect(),
    };

    fs::write(
        workspace_path.join("book.yaml"),
        serde_yaml::to_string(&manifest)?,
    )?;
    fs::write(
        workspace_path.join("style.yaml"),
        serde_yaml::to_string(&style)?,
    )?;
    fs::write(
        workspace_path.join("content/frontmatter/001-title-page.md"),
        format!("# {}\n\n{}\n", manifest.title, manifest.subtitle),
    )?;
    fs::write(
        workspace_path.join("content/chapters/001-opening.md"),
        localized_opening(language),
    )?;
    initialize_conversation_registry(&workspace_path, &slug).map_err(BookWorkspaceError::Other)?;

    let metadata = fs::metadata(&workspace_path)?;
    let created_at = metadata
        .created()
        .or_else(|_| metadata.modified())
        .map(system_time_to_utc)
        .unwrap_or_else(|_| Utc::now());
    let updated_at = metadata
        .modified()
        .map(system_time_to_utc)
        .unwrap_or(created_at);

    Ok(BookWorkspace {
        book_id: slug.clone(),
        slug,
        title: manifest.title,
        subtitle: manifest.subtitle,
        language,
        workspace_path,
        created_at,
        updated_at,
    })
}

pub fn read_conversation_registry(workspace_path: &Path) -> Result<ConversationRegistry> {
    let path = registry_path(workspace_path);
    Ok(serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
    )?)
}

pub fn write_conversation_registry(
    workspace_path: &Path,
    registry: &ConversationRegistry,
) -> Result<()> {
    let path = registry_path(workspace_path);
    fs::write(&path, serde_json::to_vec_pretty(registry)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn append_conversation_record(
    workspace_path: &Path,
    record: ConversationRegistryRecord,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    let mut registry = read_conversation_registry(workspace_path)?;
    if registry
        .conversations
        .iter()
        .any(|existing| existing.conversation_id == record.conversation_id)
    {
        return Err(ConversationRegistryError::DuplicateConversation {
            conversation_id: record.conversation_id,
        });
    }

    registry.conversations.push(record.clone());
    registry
        .conversations
        .sort_by(|left, right| right.last_active_at.cmp(&left.last_active_at));
    write_conversation_registry(workspace_path, &registry)?;
    Ok(record)
}

pub fn mark_conversation_prompt_activity(
    workspace_path: &Path,
    conversation_id: &str,
    activity_at: DateTime<Utc>,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    update_conversation_activity(workspace_path, conversation_id, activity_at)
}

pub fn mark_conversation_message_activity(
    workspace_path: &Path,
    conversation_id: &str,
    activity_at: DateTime<Utc>,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    update_conversation_activity(workspace_path, conversation_id, activity_at)
}

pub fn update_conversation_status(
    workspace_path: &Path,
    conversation_id: &str,
    status: &str,
    activity_at: Option<DateTime<Utc>>,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    mutate_conversation_record(workspace_path, conversation_id, |record| {
        record.status = status.to_string();
        if let Some(activity_at) = activity_at {
            if activity_at > record.updated_at {
                record.updated_at = activity_at;
            }
            if activity_at > record.last_active_at {
                record.last_active_at = activity_at;
            }
        }
    })
}

pub fn attach_conversation_session(
    workspace_path: &Path,
    conversation_id: &str,
    session_id: &str,
    session_log_path: &str,
    status: &str,
    activity_at: Option<DateTime<Utc>>,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    mutate_conversation_record(workspace_path, conversation_id, |record| {
        record.session_id = Some(session_id.to_string());
        record.session_log_path = session_log_path.to_string();
        record.status = status.to_string();
        if let Some(activity_at) = activity_at {
            if activity_at > record.updated_at {
                record.updated_at = activity_at;
            }
            if activity_at > record.last_active_at {
                record.last_active_at = activity_at;
            }
        }
    })
}

pub fn update_conversation_title(
    workspace_path: &Path,
    conversation_id: &str,
    title: &str,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    mutate_conversation_record(workspace_path, conversation_id, |record| {
        if record.title != title {
            record.title = title.to_string();
            let now = Utc::now();
            if now > record.updated_at {
                record.updated_at = now;
            }
        }
    })
}

pub fn list_conversation_records(
    workspace_path: &Path,
) -> Result<Vec<ConversationRegistryRecord>, ConversationRegistryError> {
    let mut conversations = read_conversation_registry(workspace_path)?.conversations;
    conversations.sort_by(|left, right| right.last_active_at.cmp(&left.last_active_at));
    Ok(conversations)
}

pub fn read_normalized_transcript(
    workspace_path: &Path,
    conversation_id: &str,
) -> Result<Vec<NormalizedTranscriptMessage>, TranscriptReadError> {
    Ok(read_conversation_transcript_snapshot(workspace_path, conversation_id)?.messages)
}

pub fn read_conversation_transcript_snapshot(
    workspace_path: &Path,
    conversation_id: &str,
) -> Result<ConversationTranscriptSnapshot, TranscriptReadError> {
    let registry = read_conversation_registry(workspace_path)?;
    let record = registry
        .conversations
        .iter()
        .find(|record| record.conversation_id == conversation_id)
        .cloned()
        .ok_or_else(|| TranscriptReadError::ConversationNotFound {
            conversation_id: conversation_id.to_string(),
        })?;

    if record.session_log_path.trim().is_empty() {
        return Ok(ConversationTranscriptSnapshot {
            messages: Vec::new(),
            session_title: None,
            last_comment: None,
        });
    }

    let session_log_path = validate_session_log_path(&record.session_log_path)?;
    let content = fs::read_to_string(&session_log_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            TranscriptReadError::SessionLogMissing
        } else {
            TranscriptReadError::Io(error)
        }
    })?;

    let mut messages = Vec::new();
    let mut last_comment: Option<NormalizedTranscriptMessage> = None;
    let lines = content.lines().collect::<Vec<_>>();
    for (line_index, line) in lines.iter().enumerate() {
        let line_number = line_index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => {
                let is_last_line = line_index + 1 == lines.len();
                if is_last_line && !content.ends_with('\n') {
                    break;
                }
                return Err(TranscriptReadError::MalformedLogLine { line: line_number });
            }
        };

        if let Some(message) = normalize_transcript_message(&value, messages.len() + 1) {
            if is_commentary_transcript_message(&message) {
                if should_replace_last_comment(last_comment.as_ref(), &message) {
                    last_comment = Some(message);
                }
            } else {
                messages.push(message);
            }
        }
    }

    let messages = filter_internal_transcript_messages(deduplicate_transcript_messages(messages));
    let session_title = infer_title_from_messages(&messages);

    Ok(ConversationTranscriptSnapshot {
        messages,
        session_title,
        last_comment: last_comment.map(|message| message.text),
    })
}

pub fn validate_session_log_path(session_log_path: &str) -> Result<PathBuf, TranscriptReadError> {
    let home = env::var("HOME").map_err(|_| TranscriptReadError::InvalidSessionLogPath)?;
    let allowed_root = PathBuf::from(home).join(".codex/sessions");
    let path = PathBuf::from(session_log_path);

    if !path.is_absolute() || path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
        return Err(TranscriptReadError::InvalidSessionLogPath);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(TranscriptReadError::InvalidSessionLogPath);
    }

    let normalized = normalize_absolute_path(&path);
    let normalized_root = normalize_absolute_path(&allowed_root);
    if !normalized.starts_with(&normalized_root) {
        return Err(TranscriptReadError::InvalidSessionLogPath);
    }

    Ok(normalized)
}

pub fn registry_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join("conversations.json")
}

pub fn initialize_conversation_registry(
    workspace_path: &Path,
    book_id: &str,
) -> Result<ConversationRegistry> {
    let registry = ConversationRegistry {
        version: CONVERSATION_REGISTRY_VERSION,
        book_id: book_id.to_string(),
        conversations: Vec::new(),
    };
    write_conversation_registry(workspace_path, &registry)?;
    Ok(registry)
}

pub fn slugify_book_title(title: &str) -> Option<String> {
    let mut slug = String::new();
    let mut pending_separator = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch.to_ascii_lowercase());
            pending_separator = false;
            continue;
        }

        let transliterated = transliterate_char(ch);
        if transliterated.is_empty() {
            pending_separator = !slug.is_empty();
            continue;
        }

        for lower in transliterated.chars().flat_map(char::to_lowercase) {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(lower);
            pending_separator = false;
        }
    }

    if slug.is_empty() { None } else { Some(slug) }
}

fn normalize_title(title: &str) -> Option<&str> {
    let normalized = title.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn update_conversation_activity(
    workspace_path: &Path,
    conversation_id: &str,
    activity_at: DateTime<Utc>,
) -> Result<ConversationRegistryRecord, ConversationRegistryError> {
    mutate_conversation_record(workspace_path, conversation_id, |record| {
        if activity_at > record.updated_at {
            record.updated_at = activity_at;
        }
        if activity_at > record.last_active_at {
            record.last_active_at = activity_at;
        }
    })
}

fn mutate_conversation_record<F>(
    workspace_path: &Path,
    conversation_id: &str,
    mutator: F,
) -> Result<ConversationRegistryRecord, ConversationRegistryError>
where
    F: FnOnce(&mut ConversationRegistryRecord),
{
    let mut registry = read_conversation_registry(workspace_path)?;
    let record = registry
        .conversations
        .iter_mut()
        .find(|record| record.conversation_id == conversation_id)
        .ok_or_else(|| ConversationRegistryError::ConversationNotFound {
            conversation_id: conversation_id.to_string(),
        })?;

    mutator(record);

    let updated = record.clone();
    registry
        .conversations
        .sort_by(|left, right| right.last_active_at.cmp(&left.last_active_at));
    write_conversation_registry(workspace_path, &registry)?;
    Ok(updated)
}

fn normalize_transcript_message(
    value: &Value,
    message_number: usize,
) -> Option<NormalizedTranscriptMessage> {
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339_utc);
    let record_type = value.get("type").and_then(Value::as_str)?;
    match record_type {
        "event_msg" | "event message" => {
            normalize_event_message(value.get("payload")?, timestamp, message_number)
        }
        "response_item" | "response item" => {
            normalize_response_item(value.get("payload")?, timestamp, message_number)
        }
        _ => None,
    }
}

fn infer_title_from_messages(messages: &[NormalizedTranscriptMessage]) -> Option<String> {
    let first_user_message = messages.iter().find(|message| message.role == "user")?;
    normalize_inferred_title(&first_user_message.text)
}

fn normalize_inferred_title(value: &str) -> Option<String> {
    let words = value
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty() {
        return None;
    }

    let max_words = 6;
    if words.len() <= max_words {
        return Some(words.join(" "));
    }

    Some(format!("{}...", words[..max_words].join(" ")))
}

fn filter_internal_transcript_messages(
    messages: Vec<NormalizedTranscriptMessage>,
) -> Vec<NormalizedTranscriptMessage> {
    messages
        .into_iter()
        .filter(|message| !is_internal_transcript_message(message))
        .enumerate()
        .map(|(index, mut message)| {
            message.message_id = format!("msg-{:06}", index + 1);
            message
        })
        .collect()
}

fn deduplicate_transcript_messages(
    messages: Vec<NormalizedTranscriptMessage>,
) -> Vec<NormalizedTranscriptMessage> {
    let mut deduplicated: Vec<NormalizedTranscriptMessage> = Vec::with_capacity(messages.len());
    for message in messages {
        let is_duplicate = deduplicated
            .last()
            .map(|previous| is_duplicate_transcript_message(previous, &message))
            .unwrap_or(false);
        if !is_duplicate {
            deduplicated.push(message);
        }
    }
    deduplicated
}

fn is_duplicate_transcript_message(
    previous: &NormalizedTranscriptMessage,
    current: &NormalizedTranscriptMessage,
) -> bool {
    if previous.role != current.role || previous.text != current.text {
        return false;
    }

    if previous.timestamp == current.timestamp {
        return true;
    }

    matches!(
        (previous.source, current.source),
        (
            TranscriptMessageSource::EventMessage,
            TranscriptMessageSource::ResponseItem
        ) | (
            TranscriptMessageSource::ResponseItem,
            TranscriptMessageSource::EventMessage
        )
    )
}

fn is_internal_transcript_message(message: &NormalizedTranscriptMessage) -> bool {
    match message.role.as_str() {
        "user" => is_internal_transcript_text(&message.text),
        "assistant" => message.text == WEB_MESSENGER_BOOTSTRAP_REPLY,
        _ => false,
    }
}

fn is_commentary_transcript_message(message: &NormalizedTranscriptMessage) -> bool {
    message.role == "commentary"
}

fn should_replace_last_comment(
    previous: Option<&NormalizedTranscriptMessage>,
    current: &NormalizedTranscriptMessage,
) -> bool {
    match previous.and_then(|message| message.timestamp) {
        None => true,
        Some(previous_timestamp) => current
            .timestamp
            .map(|current_timestamp| current_timestamp >= previous_timestamp)
            .unwrap_or(true),
    }
}

fn is_internal_transcript_text(text: &str) -> bool {
    text.starts_with("# AGENTS.md instructions for ")
        || text == WEB_MESSENGER_BOOTSTRAP_PROMPT
        || is_environment_context_message(text)
}

fn is_environment_context_message(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with(ENVIRONMENT_CONTEXT_START) && trimmed.ends_with(ENVIRONMENT_CONTEXT_END)
}

fn normalize_event_message(
    payload: &Value,
    timestamp: Option<DateTime<Utc>>,
    message_number: usize,
) -> Option<NormalizedTranscriptMessage> {
    let payload_type = payload.get("type").and_then(Value::as_str)?;
    let (role, text) = match payload_type {
        "user_message" => ("user", payload.get("message").and_then(Value::as_str)?),
        "agent_message" => ("assistant", payload.get("message").and_then(Value::as_str)?),
        _ => return None,
    };
    if role == "user" && is_internal_transcript_text(text) {
        return None;
    }
    let normalized_role = if role == "assistant" && is_commentary_phase(payload) {
        "commentary"
    } else {
        role
    };

    Some(NormalizedTranscriptMessage {
        message_id: format!("msg-{message_number:06}"),
        role: normalized_role.to_string(),
        text: text.to_string(),
        timestamp,
        source: TranscriptMessageSource::EventMessage,
    })
}

fn normalize_response_item(
    payload: &Value,
    timestamp: Option<DateTime<Utc>>,
    message_number: usize,
) -> Option<NormalizedTranscriptMessage> {
    if payload.get("type").and_then(Value::as_str)? != "message" {
        return None;
    }
    let role = payload.get("role").and_then(Value::as_str)?;
    if role != "user" && role != "assistant" {
        return None;
    }

    let text = extract_response_item_text(payload)?;
    let normalized_role = if role == "assistant" && is_commentary_phase(payload) {
        "commentary"
    } else {
        role
    };
    Some(NormalizedTranscriptMessage {
        message_id: format!("msg-{message_number:06}"),
        role: normalized_role.to_string(),
        text,
        timestamp,
        source: TranscriptMessageSource::ResponseItem,
    })
}

fn is_commentary_phase(payload: &Value) -> bool {
    payload
        .get("phase")
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|value| value.eq_ignore_ascii_case("commentary"))
        .unwrap_or(false)
}

fn extract_response_item_text(payload: &Value) -> Option<String> {
    if let Some(message) = payload.get("message").and_then(Value::as_str) {
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Some(content) = payload.get("content") {
        match content {
            Value::String(value) => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            Value::Array(items) => {
                let mut chunks = Vec::new();
                for item in items {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() && !is_internal_transcript_text(trimmed) {
                            chunks.push(trimmed.to_string());
                        }
                    }
                }
                if !chunks.is_empty() {
                    return Some(chunks.join("\n"));
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                normalized.pop();
            }
        }
    }
    normalized
}

fn validated_workspace_path(root: &Path, slug: &str) -> Result<PathBuf, BookWorkspaceError> {
    let path = root.join(slug);
    let relative = path
        .strip_prefix(root)
        .map_err(|_| BookWorkspaceError::PathEscape)?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(segment) if !segment.is_empty()))
    {
        return Err(BookWorkspaceError::PathEscape);
    }
    Ok(path)
}

fn transliterate_char(ch: char) -> &'static str {
    match ch {
        'а' | 'А' => "a",
        'б' | 'Б' => "b",
        'в' | 'В' => "v",
        'г' | 'Г' => "g",
        'д' | 'Д' => "d",
        'е' | 'Е' => "e",
        'ё' | 'Ё' => "e",
        'ж' | 'Ж' => "zh",
        'з' | 'З' => "z",
        'и' | 'И' => "i",
        'й' | 'Й' => "i",
        'к' | 'К' => "k",
        'л' | 'Л' => "l",
        'м' | 'М' => "m",
        'н' | 'Н' => "n",
        'о' | 'О' => "o",
        'п' | 'П' => "p",
        'р' | 'Р' => "r",
        'с' | 'С' => "s",
        'т' | 'Т' => "t",
        'у' | 'У' => "u",
        'ф' | 'Ф' => "f",
        'х' | 'Х' => "kh",
        'ц' | 'Ц' => "ts",
        'ч' | 'Ч' => "ch",
        'ш' | 'Ш' => "sh",
        'щ' | 'Щ' => "shch",
        'ъ' | 'Ъ' => "",
        'ы' | 'Ы' => "y",
        'ь' | 'Ь' => "",
        'э' | 'Э' => "e",
        'ю' | 'Ю' => "iu",
        'я' | 'Я' => "ia",
        _ => "",
    }
}

fn localized_subtitle(language: BookLanguage) -> &'static str {
    match language {
        BookLanguage::English => "Draft in progress",
        BookLanguage::Russian => "Черновик в работе",
    }
}

fn localized_opening(language: BookLanguage) -> &'static str {
    match language {
        BookLanguage::English => "# Opening\n\nThis book workspace is ready for authoring.\n",
        BookLanguage::Russian => "# Начало\n\nЭто книжное рабочее пространство готово к работе.\n",
    }
}

fn system_time_to_utc(time: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(time)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::sync::MutexGuard;

    use chrono::{Duration, TimeZone};
    use tempfile::tempdir;

    use super::*;

    fn env_lock() -> MutexGuard<'static, ()> {
        crate::core::config::test_env_lock()
    }

    #[test]
    fn slugifies_titles_into_safe_directory_names() {
        assert_eq!(
            slugify_book_title(" ../Quiet Lighthouse!!! ").as_deref(),
            Some("quiet-lighthouse")
        );
        assert_eq!(
            slugify_book_title("Тихий маяк").as_deref(),
            Some("tikhii-maiak")
        );
        assert_eq!(slugify_book_title("..."), None);
    }

    #[test]
    fn provisioning_creates_workspace_and_empty_registry() {
        let dir = tempdir().unwrap();

        let book = provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
            .unwrap();

        assert_eq!(book.slug, "quiet-lighthouse");
        assert!(book.workspace_path.starts_with(dir.path()));
        assert!(book.workspace_path.join("book.yaml").exists());
        assert!(book.workspace_path.join("style.yaml").exists());
        assert!(book.workspace_path.join("assets/images").exists());
        assert!(
            book.workspace_path
                .join("content/frontmatter/001-title-page.md")
                .exists()
        );
        assert!(
            book.workspace_path
                .join("content/chapters/001-opening.md")
                .exists()
        );

        let registry = read_conversation_registry(&book.workspace_path).unwrap();
        assert_eq!(
            registry,
            ConversationRegistry {
                version: 1,
                book_id: "quiet-lighthouse".to_string(),
                conversations: Vec::new(),
            }
        );
    }

    #[test]
    fn provisioning_rejects_duplicate_slug() {
        let dir = tempdir().unwrap();

        provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English).unwrap();
        let error =
            provision_book_workspace(dir.path(), "Quiet---Lighthouse", BookLanguage::English)
                .unwrap_err();

        assert!(matches!(
            error,
            BookWorkspaceError::DuplicateSlug { slug } if slug == "quiet-lighthouse"
        ));
    }

    #[test]
    fn validates_session_log_paths_and_parses_transcripts() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/01");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        let latest_at = created_at + Duration::minutes(3);
        let log_path = sessions_dir.join("rollout-2026-05-01T12-00-00-session-1.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"session-1\",\"timestamp\":\"{created}\",\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"thread_name_updated\",\"thread_id\":\"session-1\",\"thread_name\":\"Codex named thread\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"# AGENTS.md instructions for /tmp/workspace\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Initialize a new web messenger conversation for this book workspace. Reply with exactly: Session ready.\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Session ready.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"Initialize a new web messenger conversation for this book workspace. Reply with exactly: Session ready.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Session ready.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{latest}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Hello\"}}}}\n",
                    "{{\"timestamp\":\"{latest}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                latest = latest_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-1".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Opening".to_string(),
                session_id: Some("session-1".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let snapshot =
            read_conversation_transcript_snapshot(&workspace.workspace_path, "conversation-1")
                .unwrap();
        assert_eq!(
            snapshot.session_title.as_deref(),
            Some("Codex named thread")
        );
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].role, "user");
        assert_eq!(snapshot.messages[0].text, "Hello");
        assert_eq!(snapshot.messages[0].message_id, "msg-000001");

        let outside_path = dir.path().join("outside.jsonl");
        assert!(matches!(
            validate_session_log_path(outside_path.to_string_lossy().as_ref()),
            Err(TranscriptReadError::InvalidSessionLogPath)
        ));
    }

    #[test]
    fn transcript_snapshot_falls_back_to_first_user_message_for_exec_sessions_without_titles() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/02");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 11, 13, 19).unwrap();
        let log_path = sessions_dir.join("rollout-2026-05-02T14-13-16-session-exec.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"session-exec\",\"timestamp\":\"{created}\",\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"в главе про лето добавь дополнительный пункт про пожары на Кипре\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"Working.\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-exec".to_string(),
                book_id: workspace.book_id.clone(),
                title: "New conversation".to_string(),
                session_id: Some("session-exec".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "ready".to_string(),
            },
        )
        .unwrap();

        let snapshot =
            read_conversation_transcript_snapshot(&workspace.workspace_path, "conversation-exec")
                .unwrap();
        assert_eq!(
            snapshot.session_title.as_deref(),
            Some("в главе про лето добавь дополнительный...")
        );
        assert_eq!(snapshot.messages[0].role, "user");
        assert_eq!(
            snapshot.messages[0].text,
            "в главе про лето добавь дополнительный пункт про пожары на Кипре"
        );
    }

    #[test]
    fn transcript_reader_rejects_malformed_logs_but_ignores_trailing_partial_line() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/01");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        let bad_log_path = sessions_dir.join("malformed.jsonl");
        fs::write(
            &bad_log_path,
            format!(
                "{{\"timestamp\":\"{}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{}\"}}}}\nnot-json\n",
                created_at.to_rfc3339(),
                workspace.workspace_path.display()
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "malformed".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Malformed".to_string(),
                session_id: Some("session-malformed".to_string()),
                session_log_path: bad_log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();
        assert!(matches!(
            read_normalized_transcript(&workspace.workspace_path, "malformed"),
            Err(TranscriptReadError::MalformedLogLine { line: 2 })
        ));

        let partial_log_path = sessions_dir.join("partial.jsonl");
        fs::write(
            &partial_log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Hello\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\""
                ),
                created = created_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "partial".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Partial".to_string(),
                session_id: Some("session-partial".to_string()),
                session_log_path: partial_log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();
        let partial_messages =
            read_normalized_transcript(&workspace.workspace_path, "partial").unwrap();
        assert_eq!(partial_messages.len(), 1);
        assert_eq!(partial_messages[0].text, "Hello");
    }

    #[test]
    fn transcript_reader_deduplicates_adjacent_identical_messages() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/01");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        let duplicate_at = created_at + Duration::minutes(1);
        let log_path = sessions_dir.join("duplicates.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{duplicate}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"Repeated reply\"}}}}\n",
                    "{{\"timestamp\":\"{duplicate}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Repeated reply\"}}]}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Next prompt\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                duplicate = duplicate_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "duplicates".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Duplicates".to_string(),
                session_id: Some("session-duplicates".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let messages = read_normalized_transcript(&workspace.workspace_path, "duplicates").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[0].text, "Repeated reply");
        assert_eq!(messages[0].message_id, "msg-000001");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].text, "Next prompt");
        assert_eq!(messages[1].message_id, "msg-000002");
    }

    #[test]
    fn transcript_reader_deduplicates_adjacent_event_and_response_messages_with_different_timestamps()
     {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/02");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 12, 0, 0).unwrap();
        let event_at = created_at + Duration::seconds(1);
        let response_at = created_at + Duration::seconds(2);
        let log_path = sessions_dir.join("duplicates-mismatched-timestamps.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{event}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Repeat me\"}}}}\n",
                    "{{\"timestamp\":\"{response}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"Repeat me\"}}]}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"Handled\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                event = event_at.to_rfc3339(),
                response = response_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "duplicates-mismatched".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Duplicates mismatched".to_string(),
                session_id: Some("session-duplicates-mismatched".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let messages =
            read_normalized_transcript(&workspace.workspace_path, "duplicates-mismatched").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text, "Repeat me");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].text, "Handled");
    }

    #[test]
    fn transcript_reader_filters_environment_context_bootstrap_messages() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/02");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 14, 32, 0).unwrap();
        let latest_at = created_at + Duration::minutes(1);
        let log_path = sessions_dir.join("environment-context.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"<environment_context>\\n<cwd>{cwd}</cwd>\\n<shell>zsh</shell>\\n<current_date>2026-05-02</current_date>\\n<timezone>Asia/Nicosia</timezone>\\n</environment_context>\"}}}}\n",
                    "{{\"timestamp\":\"{latest}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Actual user prompt\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                latest = latest_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "environment-context".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Environment context".to_string(),
                session_id: Some("session-environment-context".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let messages =
            read_normalized_transcript(&workspace.workspace_path, "environment-context").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text, "Actual user prompt");
    }

    #[test]
    fn transcript_reader_filters_internal_messages_even_when_not_leading() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/02");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 14, 32, 0).unwrap();
        let next_at = created_at + Duration::minutes(1);
        let latest_at = created_at + Duration::minutes(2);
        let log_path = sessions_dir.join("midstream-environment-context.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"First real prompt\"}}}}\n",
                    "{{\"timestamp\":\"{next}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"<environment_context>\\n<cwd>{cwd}</cwd>\\n<shell>zsh</shell>\\n<current_date>2026-05-02</current_date>\\n<timezone>Asia/Nicosia</timezone>\\n</environment_context>\"}}]}}}}\n",
                    "{{\"timestamp\":\"{latest}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Second real prompt\"}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                next = next_at.to_rfc3339(),
                latest = latest_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "midstream-environment-context".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Midstream environment context".to_string(),
                session_id: Some("session-midstream-environment-context".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let messages =
            read_normalized_transcript(&workspace.workspace_path, "midstream-environment-context")
                .unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "First real prompt");
        assert_eq!(messages[1].text, "Second real prompt");
        assert_eq!(messages[0].message_id, "msg-000001");
        assert_eq!(messages[1].message_id, "msg-000002");
    }

    #[test]
    fn transcript_snapshot_excludes_commentary_messages_and_keeps_last_comment() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/02");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 15, 0, 0).unwrap();
        let commentary_at = created_at + Duration::seconds(1);
        let final_at = created_at + Duration::seconds(2);
        let log_path = sessions_dir.join("commentary-phase.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Revise chapter one\"}}}}\n",
                    "{{\"timestamp\":\"{commentary}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"commentary\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Updating outline now.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{final_at}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Chapter one revised.\"}}]}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                commentary = commentary_at.to_rfc3339(),
                final_at = final_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "commentary-phase".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Commentary phase".to_string(),
                session_id: Some("session-commentary-phase".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let snapshot =
            read_conversation_transcript_snapshot(&workspace.workspace_path, "commentary-phase")
                .unwrap();
        assert_eq!(snapshot.last_comment.as_deref(), Some("Updating outline now."));
        assert_eq!(snapshot.messages.len(), 2);
        assert_eq!(snapshot.messages[0].text, "Revise chapter one");
        assert_eq!(snapshot.messages[1].text, "Chapter one revised.");
    }

    #[test]
    fn transcript_snapshot_filters_commentary_from_event_and_response_messages() {
        let _guard = env_lock();
        let dir = tempdir().unwrap();
        unsafe {
            env::set_var("HOME", dir.path());
        }
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let sessions_dir = dir.path().join(".codex/sessions/2026/05/02");
        fs::create_dir_all(&sessions_dir).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 2, 15, 0, 0).unwrap();
        let commentary_event_at = created_at + Duration::seconds(1);
        let commentary_response_at = created_at + Duration::seconds(2);
        let final_at = created_at + Duration::seconds(3);
        let log_path = sessions_dir.join("commentary-event-and-response.jsonl");
        fs::write(
            &log_path,
            format!(
                concat!(
                    "{{\"timestamp\":\"{created}\",\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n",
                    "{{\"timestamp\":\"{created}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"Revise chapter one\"}}}}\n",
                    "{{\"timestamp\":\"{commentary_event}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"Updating outline now.\",\"phase\":\"commentary\"}}}}\n",
                    "{{\"timestamp\":\"{commentary_response}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"commentary\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Updating outline now.\"}}]}}}}\n",
                    "{{\"timestamp\":\"{final_at}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"Chapter one revised.\"}}]}}}}\n"
                ),
                created = created_at.to_rfc3339(),
                commentary_event = commentary_event_at.to_rfc3339(),
                commentary_response = commentary_response_at.to_rfc3339(),
                final_at = final_at.to_rfc3339(),
                cwd = workspace.workspace_path.display(),
            ),
        )
        .unwrap();

        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "commentary-event-and-response".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Commentary event and response".to_string(),
                session_id: Some("session-commentary-event-and-response".to_string()),
                session_log_path: log_path.to_string_lossy().to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let snapshot = read_conversation_transcript_snapshot(
            &workspace.workspace_path,
            "commentary-event-and-response",
        )
        .unwrap();
        assert_eq!(snapshot.last_comment.as_deref(), Some("Updating outline now."));
        assert_eq!(snapshot.messages.len(), 2);
        assert_eq!(snapshot.messages[0].text, "Revise chapter one");
        assert_eq!(snapshot.messages[1].text, "Chapter one revised.");
    }

    #[test]
    fn prompt_and_message_activity_update_last_active_at() {
        let dir = tempdir().unwrap();
        let workspace =
            provision_book_workspace(dir.path(), "Quiet Lighthouse", BookLanguage::English)
                .unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        append_conversation_record(
            &workspace.workspace_path,
            ConversationRegistryRecord {
                conversation_id: "conversation-1".to_string(),
                book_id: workspace.book_id.clone(),
                title: "Opening".to_string(),
                session_id: Some("session-1".to_string()),
                session_log_path: "/tmp/unused.jsonl".to_string(),
                created_at,
                updated_at: created_at,
                last_active_at: created_at,
                status: "active".to_string(),
            },
        )
        .unwrap();

        let prompt_at = created_at + Duration::minutes(1);
        let after_prompt = mark_conversation_prompt_activity(
            &workspace.workspace_path,
            "conversation-1",
            prompt_at,
        )
        .unwrap();
        assert_eq!(after_prompt.last_active_at, prompt_at);
        assert_eq!(after_prompt.updated_at, prompt_at);

        let message_at = created_at + Duration::minutes(2);
        let after_message = mark_conversation_message_activity(
            &workspace.workspace_path,
            "conversation-1",
            message_at,
        )
        .unwrap();
        assert_eq!(after_message.last_active_at, message_at);
        assert_eq!(after_message.updated_at, message_at);
    }
}
