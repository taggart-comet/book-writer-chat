use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::core::models::*;

pub fn normalize_conversation_id(provider: &Provider, provider_chat_id: &str) -> String {
    let prefix = format!("{}:", provider.as_str());
    if provider_chat_id.starts_with(&prefix) {
        provider_chat_id.to_string()
    } else {
        format!("{prefix}{provider_chat_id}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Database {
    pub conversations: HashMap<String, Conversation>,
    pub books: HashMap<String, Book>,
    pub sessions: HashMap<String, AuthoringSession>,
    pub jobs: HashMap<String, WritingJob>,
    pub revisions: HashMap<String, Revision>,
    pub repository_bindings: HashMap<String, RepositoryBinding>,
    pub conversation_to_book: HashMap<String, String>,
    pub next_id: u64,
}

#[derive(Clone)]
pub struct Repository {
    state_path: PathBuf,
    db: Arc<RwLock<Database>>,
}

impl Repository {
    pub async fn load(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let state_path = data_dir.join("state.json");
        let db = if state_path.exists() {
            serde_json::from_slice(&std::fs::read(&state_path)?)?
        } else {
            Database::default()
        };
        Ok(Self {
            state_path,
            db: Arc::new(RwLock::new(db)),
        })
    }

    pub async fn snapshot(&self) -> Database {
        self.db.read().await.clone()
    }

    async fn persist(&self) -> Result<()> {
        let db = self.db.read().await;
        std::fs::write(&self.state_path, serde_json::to_vec_pretty(&*db)?)?;
        Ok(())
    }

    async fn next_id(&self, prefix: &str) -> String {
        let mut db = self.db.write().await;
        db.next_id += 1;
        format!("{prefix}-{}", db.next_id)
    }

    pub async fn resolve_or_create_conversation(
        &self,
        provider: Provider,
        provider_chat_id: String,
        title: String,
    ) -> Result<Conversation> {
        let key = normalize_conversation_id(&provider, &provider_chat_id);
        {
            let db = self.db.read().await;
            if let Some(existing) = db.conversations.get(&key) {
                return Ok(existing.clone());
            }
        }
        let conversation = Conversation {
            conversation_id: key.clone(),
            provider,
            provider_chat_id: key.clone(),
            title,
            created_at: Utc::now(),
            status: ConversationStatus::Active,
        };
        let mut db = self.db.write().await;
        db.conversations.insert(key, conversation.clone());
        drop(db);
        self.persist().await?;
        Ok(conversation)
    }

    pub async fn find_book_by_conversation(&self, conversation_id: &str) -> Option<Book> {
        let db = self.db.read().await;
        db.conversation_to_book
            .get(conversation_id)
            .and_then(|book_id| db.books.get(book_id))
            .cloned()
    }

    pub async fn get_conversation(&self, conversation_id: &str) -> Option<Conversation> {
        self.db
            .read()
            .await
            .conversations
            .get(conversation_id)
            .cloned()
    }

    pub async fn create_book(
        &self,
        conversation_id: &str,
        title: String,
        workspace_path: String,
    ) -> Result<Book> {
        if let Some(existing) = self.find_book_by_conversation(conversation_id).await {
            return Ok(existing);
        }
        let book_id = self.next_id("book").await;
        let book = Book {
            book_id: book_id.clone(),
            conversation_id: conversation_id.to_string(),
            title,
            status: BookStatus::Active,
            workspace_path,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let mut db = self.db.write().await;
        db.conversation_to_book
            .insert(conversation_id.to_string(), book_id.clone());
        db.books.insert(book_id, book.clone());
        drop(db);
        self.persist().await?;
        Ok(book)
    }

    pub async fn touch_book(&self, book_id: &str) -> Result<()> {
        let mut db = self.db.write().await;
        if let Some(book) = db.books.get_mut(book_id) {
            book.updated_at = Utc::now();
        }
        drop(db);
        self.persist().await
    }

    pub async fn open_session(
        &self,
        conversation_id: &str,
        book_id: &str,
        last_message_at: chrono::DateTime<Utc>,
    ) -> Result<AuthoringSession> {
        {
            let db = self.db.read().await;
            if let Some(existing) = db.sessions.values().find(|session| {
                session.book_id == book_id && session.status == SessionStatus::Active
            }) {
                return Ok(existing.clone());
            }
        }
        let session = AuthoringSession {
            session_id: self.next_id("session").await,
            conversation_id: conversation_id.to_string(),
            book_id: book_id.to_string(),
            status: SessionStatus::Active,
            last_message_at,
        };
        let mut db = self.db.write().await;
        db.sessions
            .insert(session.session_id.clone(), session.clone());
        drop(db);
        self.persist().await?;
        Ok(session)
    }

    pub async fn create_job(
        &self,
        book_id: &str,
        conversation_id: &str,
        session_id: &str,
        source_message_id: &str,
        command_kind: CommandKind,
        prompt_snapshot: String,
    ) -> Result<WritingJob> {
        let job = WritingJob {
            job_id: self.next_id("job").await,
            book_id: book_id.to_string(),
            conversation_id: conversation_id.to_string(),
            session_id: session_id.to_string(),
            source_message_id: source_message_id.to_string(),
            status: JobStatus::Received,
            command_kind,
            prompt_snapshot,
            started_at: None,
            finished_at: None,
            user_facing_message: None,
            changed_files: Vec::new(),
            failure_reason: None,
        };
        let mut db = self.db.write().await;
        db.jobs.insert(job.job_id.clone(), job.clone());
        drop(db);
        self.persist().await?;
        Ok(job)
    }

    pub async fn update_job_status(
        &self,
        job_id: &str,
        status: JobStatus,
        message: Option<String>,
        changed_files: Option<Vec<String>>,
        failure_reason: Option<String>,
    ) -> Result<WritingJob> {
        let mut db = self.db.write().await;
        let job = db
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| anyhow!("missing job"))?;
        match status {
            JobStatus::Accepted | JobStatus::Running => {
                if job.started_at.is_none() {
                    job.started_at = Some(Utc::now());
                }
            }
            JobStatus::Succeeded
            | JobStatus::Failed
            | JobStatus::TimedOut
            | JobStatus::Cancelled => {
                if job.started_at.is_none() {
                    job.started_at = Some(Utc::now());
                }
                job.finished_at = Some(Utc::now());
            }
            JobStatus::Received => {}
        }
        job.status = status;
        if let Some(message) = message {
            job.user_facing_message = Some(message);
        }
        if let Some(changed_files) = changed_files {
            job.changed_files = changed_files;
        }
        if let Some(failure_reason) = failure_reason {
            job.failure_reason = Some(failure_reason);
        }
        let snapshot = job.clone();
        drop(db);
        self.persist().await?;
        Ok(snapshot)
    }

    pub async fn create_revision(
        &self,
        book_id: &str,
        job_id: &str,
        summary: String,
        render_status: RevisionRenderStatus,
    ) -> Result<Revision> {
        let revision = Revision {
            revision_id: self.next_id("revision").await,
            book_id: book_id.to_string(),
            job_id: job_id.to_string(),
            summary,
            created_at: Utc::now(),
            render_status,
        };
        let mut db = self.db.write().await;
        db.revisions
            .insert(revision.revision_id.clone(), revision.clone());
        drop(db);
        self.persist().await?;
        Ok(revision)
    }

    pub async fn latest_revision_for_book(&self, book_id: &str) -> Option<Revision> {
        let db = self.db.read().await;
        db.revisions
            .values()
            .filter(|revision| revision.book_id == book_id)
            .max_by_key(|revision| revision.created_at)
            .cloned()
    }

    pub async fn latest_job_for_book(&self, book_id: &str) -> Option<WritingJob> {
        let db = self.db.read().await;
        db.jobs
            .values()
            .filter(|job| job.book_id == book_id)
            .max_by_key(|job| job.started_at)
            .cloned()
    }

    pub async fn get_book(&self, book_id: &str) -> Option<Book> {
        self.db.read().await.books.get(book_id).cloned()
    }

    pub async fn get_session(&self, session_id: &str) -> Option<AuthoringSession> {
        self.db.read().await.sessions.get(session_id).cloned()
    }

    pub async fn get_job(&self, job_id: &str) -> Option<WritingJob> {
        self.db.read().await.jobs.get(job_id).cloned()
    }

    pub async fn get_revision(&self, revision_id: &str) -> Option<Revision> {
        self.db.read().await.revisions.get(revision_id).cloned()
    }

    pub async fn upsert_repository_binding(
        &self,
        book_id: &str,
        provider: String,
        repository_url: String,
        repository_name: String,
        status: RepositoryBindingStatus,
    ) -> Result<RepositoryBinding> {
        let mut db = self.db.write().await;
        let now = Utc::now();
        let existing_id = db
            .repository_bindings
            .values()
            .find(|binding| binding.book_id == book_id)
            .map(|binding| binding.repository_binding_id.clone());
        let binding = if let Some(binding_id) = existing_id {
            let binding = db
                .repository_bindings
                .get_mut(&binding_id)
                .ok_or_else(|| anyhow!("missing repository binding"))?;
            binding.provider = provider;
            binding.repository_url = repository_url;
            binding.repository_name = repository_name;
            binding.status = status;
            binding.updated_at = now;
            binding.clone()
        } else {
            let binding = RepositoryBinding {
                repository_binding_id: format!("binding-{}", db.next_id + 1),
                book_id: book_id.to_string(),
                provider,
                repository_url,
                repository_name,
                status,
                created_at: now,
                updated_at: now,
            };
            db.next_id += 1;
            db.repository_bindings
                .insert(binding.repository_binding_id.clone(), binding.clone());
            binding
        };
        drop(db);
        self.persist().await?;
        Ok(binding)
    }

    pub async fn get_repository_binding_for_book(
        &self,
        book_id: &str,
    ) -> Option<RepositoryBinding> {
        self.db
            .read()
            .await
            .repository_bindings
            .values()
            .find(|binding| binding.book_id == book_id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn enforces_one_book_per_conversation() {
        let dir = tempdir().unwrap();
        let repo = Repository::load(dir.path()).await.unwrap();
        let conversation = repo
            .resolve_or_create_conversation(
                Provider::Telegram,
                "telegram:123".to_string(),
                "Test".to_string(),
            )
            .await
            .unwrap();
        let one = repo
            .create_book(
                &conversation.conversation_id,
                "One".to_string(),
                "a".to_string(),
            )
            .await
            .unwrap();
        let two = repo
            .create_book(
                &conversation.conversation_id,
                "Two".to_string(),
                "b".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(one.book_id, two.book_id);
    }

    #[tokio::test]
    async fn normalizes_conversation_identity_per_provider() {
        let dir = tempdir().unwrap();
        let repo = Repository::load(dir.path()).await.unwrap();

        let telegram_one = repo
            .resolve_or_create_conversation(
                Provider::Telegram,
                "123".to_string(),
                "Telegram".to_string(),
            )
            .await
            .unwrap();
        let telegram_two = repo
            .resolve_or_create_conversation(
                Provider::Telegram,
                "telegram:123".to_string(),
                "Telegram".to_string(),
            )
            .await
            .unwrap();
        let max_one = repo
            .resolve_or_create_conversation(Provider::Max, "123".to_string(), "MAX".to_string())
            .await
            .unwrap();

        assert_eq!(telegram_one.conversation_id, "telegram:123");
        assert_eq!(telegram_one.conversation_id, telegram_two.conversation_id);
        assert_eq!(telegram_one.provider_chat_id, "telegram:123");
        assert_eq!(max_one.conversation_id, "max:123");

        let snapshot = repo.snapshot().await;
        assert_eq!(snapshot.conversations.len(), 2);
    }

    #[tokio::test]
    async fn stores_and_queries_all_domain_records() {
        let dir = tempdir().unwrap();
        let repo = Repository::load(dir.path()).await.unwrap();
        let conversation = repo
            .resolve_or_create_conversation(
                Provider::Telegram,
                "telegram:123".to_string(),
                "Test".to_string(),
            )
            .await
            .unwrap();
        let book = repo
            .create_book(
                &conversation.conversation_id,
                "Draft".to_string(),
                "books-data/telegram-123".to_string(),
            )
            .await
            .unwrap();
        let session = repo
            .open_session(&conversation.conversation_id, &book.book_id, Utc::now())
            .await
            .unwrap();
        let job = repo
            .create_job(
                &book.book_id,
                &conversation.conversation_id,
                &session.session_id,
                "msg-1",
                CommandKind::Authoring,
                "prompt".to_string(),
            )
            .await
            .unwrap();
        let revision = repo
            .create_revision(
                &book.book_id,
                &job.job_id,
                "Summary".to_string(),
                RevisionRenderStatus::Ready,
            )
            .await
            .unwrap();
        let binding = repo
            .upsert_repository_binding(
                &book.book_id,
                "github".to_string(),
                "https://github.com/example/book".to_string(),
                "book".to_string(),
                RepositoryBindingStatus::Linked,
            )
            .await
            .unwrap();

        assert_eq!(
            repo.get_conversation(&conversation.conversation_id)
                .await
                .unwrap()
                .conversation_id,
            conversation.conversation_id
        );
        assert_eq!(
            repo.find_book_by_conversation(&conversation.conversation_id)
                .await
                .unwrap()
                .book_id,
            book.book_id
        );
        assert_eq!(
            repo.get_session(&session.session_id)
                .await
                .unwrap()
                .session_id,
            session.session_id
        );
        assert_eq!(repo.get_job(&job.job_id).await.unwrap().job_id, job.job_id);
        assert_eq!(
            repo.get_revision(&revision.revision_id)
                .await
                .unwrap()
                .revision_id,
            revision.revision_id
        );
        assert_eq!(
            repo.get_repository_binding_for_book(&book.book_id)
                .await
                .unwrap()
                .repository_binding_id,
            binding.repository_binding_id
        );
    }

    #[tokio::test]
    async fn job_lifecycle_transitions_cover_received_accepted_running_and_succeeded() {
        let dir = tempdir().unwrap();
        let repo = Repository::load(dir.path()).await.unwrap();
        let conversation = repo
            .resolve_or_create_conversation(
                Provider::Telegram,
                "telegram:123".to_string(),
                "Test".to_string(),
            )
            .await
            .unwrap();
        let book = repo
            .create_book(
                &conversation.conversation_id,
                "Draft".to_string(),
                "books-data/telegram-123".to_string(),
            )
            .await
            .unwrap();
        let session = repo
            .open_session(&conversation.conversation_id, &book.book_id, Utc::now())
            .await
            .unwrap();

        let received = repo
            .create_job(
                &book.book_id,
                &conversation.conversation_id,
                &session.session_id,
                "msg-1",
                CommandKind::Authoring,
                "prompt".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(received.status, JobStatus::Received);
        assert!(received.started_at.is_none());
        assert!(received.finished_at.is_none());

        let accepted = repo
            .update_job_status(&received.job_id, JobStatus::Accepted, None, None, None)
            .await
            .unwrap();
        assert_eq!(accepted.status, JobStatus::Accepted);
        assert!(accepted.started_at.is_some());
        assert!(accepted.finished_at.is_none());

        let running = repo
            .update_job_status(&received.job_id, JobStatus::Running, None, None, None)
            .await
            .unwrap();
        assert_eq!(running.status, JobStatus::Running);
        assert!(running.started_at.is_some());
        assert!(running.finished_at.is_none());

        let succeeded = repo
            .update_job_status(
                &received.job_id,
                JobStatus::Succeeded,
                Some("done".to_string()),
                Some(vec!["book.yaml".to_string()]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(succeeded.status, JobStatus::Succeeded);
        assert!(succeeded.started_at.is_some());
        assert!(succeeded.finished_at.is_some());
        assert_eq!(succeeded.changed_files, vec!["book.yaml".to_string()]);
    }

    #[tokio::test]
    async fn job_lifecycle_transitions_cover_failed_and_timed_out() {
        let dir = tempdir().unwrap();
        let repo = Repository::load(dir.path()).await.unwrap();
        let conversation = repo
            .resolve_or_create_conversation(
                Provider::Telegram,
                "telegram:123".to_string(),
                "Test".to_string(),
            )
            .await
            .unwrap();
        let book = repo
            .create_book(
                &conversation.conversation_id,
                "Draft".to_string(),
                "books-data/telegram-123".to_string(),
            )
            .await
            .unwrap();
        let session = repo
            .open_session(&conversation.conversation_id, &book.book_id, Utc::now())
            .await
            .unwrap();

        let failed = repo
            .create_job(
                &book.book_id,
                &conversation.conversation_id,
                &session.session_id,
                "msg-1",
                CommandKind::Authoring,
                "prompt".to_string(),
            )
            .await
            .unwrap();
        let failed = repo
            .update_job_status(
                &failed.job_id,
                JobStatus::Failed,
                Some("failed".to_string()),
                None,
                Some("stderr".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(failed.status, JobStatus::Failed);
        assert!(failed.started_at.is_some());
        assert!(failed.finished_at.is_some());
        assert_eq!(failed.failure_reason.as_deref(), Some("stderr"));

        let timed_out = repo
            .create_job(
                &book.book_id,
                &conversation.conversation_id,
                &session.session_id,
                "msg-2",
                CommandKind::Authoring,
                "prompt".to_string(),
            )
            .await
            .unwrap();
        let timed_out = repo
            .update_job_status(
                &timed_out.job_id,
                JobStatus::TimedOut,
                Some("timed out".to_string()),
                None,
                Some("timeout".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(timed_out.status, JobStatus::TimedOut);
        assert!(timed_out.started_at.is_some());
        assert!(timed_out.finished_at.is_some());
        assert_eq!(timed_out.failure_reason.as_deref(), Some("timeout"));
    }
}
