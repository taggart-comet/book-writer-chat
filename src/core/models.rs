use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    App,
    Telegram,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Telegram => "telegram",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BookStatus {
    Active,
    Archived,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Idle,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Received,
    Accepted,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Init,
    Status,
    Authoring,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RevisionRenderStatus {
    Pending,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookLanguage {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ru")]
    Russian,
}

impl Default for BookLanguage {
    fn default() -> Self {
        Self::English
    }
}

impl BookLanguage {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "en" | "eng" | "english" => Some(Self::English),
            "ru" | "rus" | "russian" | "русский" | "русскии" | "рус" => {
                Some(Self::Russian)
            }
            _ => None,
        }
    }

    pub fn from_manifest_code(value: &str) -> Self {
        Self::parse(value).unwrap_or_default()
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Russian",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryBindingStatus {
    Unlinked,
    Linked,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub conversation_id: String,
    pub provider: Provider,
    pub provider_chat_id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub status: ConversationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub book_id: String,
    pub conversation_id: String,
    pub title: String,
    pub status: BookStatus,
    pub workspace_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthoringSession {
    pub session_id: String,
    pub conversation_id: String,
    pub book_id: String,
    pub status: SessionStatus,
    pub last_message_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingJob {
    pub job_id: String,
    pub book_id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub source_message_id: String,
    pub status: JobStatus,
    pub command_kind: CommandKind,
    pub prompt_snapshot: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub user_facing_message: Option<String>,
    pub changed_files: Vec<String>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub revision_id: String,
    pub book_id: String,
    pub job_id: String,
    pub summary: String,
    pub created_at: DateTime<Utc>,
    pub render_status: RevisionRenderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryBinding {
    pub repository_binding_id: String,
    pub book_id: String,
    pub provider: String,
    pub repository_url: String,
    pub repository_name: String,
    pub status: RepositoryBindingStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageAttachmentKind {
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageAttachment {
    pub kind: MessageAttachmentKind,
    pub provider_file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_unique_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMessage {
    pub provider: Provider,
    pub provider_chat_id: String,
    pub message_id: String,
    pub timestamp: DateTime<Utc>,
    pub raw_text: String,
    pub attachments: Vec<MessageAttachment>,
    pub mentions_bot: bool,
    pub sender_display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub provider: Provider,
    pub provider_chat_id: String,
    pub message: String,
    pub reader_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderSummary {
    pub book_id: String,
    pub title: String,
    pub subtitle: String,
    pub language: BookLanguage,
    pub status: BookStatus,
    pub last_revision_id: Option<String>,
    pub last_updated_at: DateTime<Utc>,
    pub render_status: RevisionRenderStatus,
    pub chapter_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderContentChapter {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderContentResponse {
    pub revision_id: String,
    pub content_hash: String,
    pub chapter_index: usize,
    pub chapter_id: String,
    pub title: String,
    pub source_file: String,
    pub html: String,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderRevisionResponse {
    pub revision_id: String,
    pub created_at: DateTime<Utc>,
    pub source_job_id: String,
    pub summary: String,
    pub render_status: RevisionRenderStatus,
    pub content_hash: Option<String>,
    pub render_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderJobResponse {
    pub job_id: String,
    pub status: JobStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub user_facing_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderErrorResponse {
    pub code: String,
    pub message: String,
}
