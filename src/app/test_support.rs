#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tempfile::tempdir;
    use tower::ServiceExt;

    use crate::{
        app::build_router,
        authoring::executor::{DynExecutor, ExecutionOutcome, FakeExecutor},
        core::{
            config::{AppEnvironment, Config},
            models::{
                CommandKind, JobStatus, ReaderContentResponse, ReaderErrorResponse,
                ReaderJobResponse, ReaderRevisionResponse, ReaderSummary, RevisionRenderStatus,
            },
        },
        messaging::handlers::MessageApiResponse,
        reader::{
            content::{ChapterCursor, encode_cursor},
            links::issue_token,
        },
        storage::repository::Repository,
    };

    async fn test_app(executor: DynExecutor) -> (Router, Config) {
        let root = tempdir().unwrap().keep();
        let config = Config {
            environment: AppEnvironment::Test,
            bind_addr: "127.0.0.1:3001".parse().unwrap(),
            data_dir: root.join("data"),
            books_root: root.join("books-data"),
            frontend_dist_dir: root.join("frontend-build"),
            frontend_base_url: "http://localhost:3001".to_string(),
            telegram_bot_username: "bookbot".to_string(),
            max_bot_handle: "bookbot".to_string(),
            reader_token_secret: "secret".to_string(),
            codex_cli_path: "codex".to_string(),
            codex_cli_args: Vec::new(),
            agent_timeout_secs: 5,
        };
        let router = build_router(config.clone(), Some(executor)).await.unwrap();
        (router, config)
    }

    fn telegram_message_payload(message_id: i64, text: &str) -> serde_json::Value {
        telegram_message_payload_for_chat(123456, message_id, text)
    }

    fn telegram_message_payload_for_chat(
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "message": {
                "message_id": message_id,
                "date": 1775385600 + message_id,
                "text": text,
                "chat": {"id": chat_id, "title": "Chat"},
                "from": {"first_name": "Alice"},
                "reply_to_message": null
            }
        })
    }

    fn extract_reader_path(reader_url: &str) -> String {
        let (_, path) = reader_url.split_once("://").unwrap();
        format!("/{}", path.split_once('/').unwrap().1)
    }

    fn fixture_payload(name: &str) -> serde_json::Value {
        match name {
            "telegram-init" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/telegram-init.json"
            ))
            .unwrap(),
            "telegram-status" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/telegram-status.json"
            ))
            .unwrap(),
            "telegram-authoring-reply" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/telegram-authoring-reply.json"
            ))
            .unwrap(),
            "telegram-ignored" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/telegram-ignored.json"
            ))
            .unwrap(),
            "max-init" => {
                serde_json::from_str(include_str!("../../tests/fixtures/messenger/max-init.json"))
                    .unwrap()
            }
            "max-status" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/max-status.json"
            ))
            .unwrap(),
            "max-authoring" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/max-authoring.json"
            ))
            .unwrap(),
            "max-ignored" => serde_json::from_str(include_str!(
                "../../tests/fixtures/messenger/max-ignored.json"
            ))
            .unwrap(),
            other => panic!("unknown fixture: {other}"),
        }
    }

    async fn post_telegram(app: Router, payload: serde_json::Value) -> axum::http::Response<Body> {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/messages/telegram")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn post_max(app: Router, payload: serde_json::Value) -> axum::http::Response<Body> {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/messages/max")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn get(app: Router, uri: impl Into<String>) -> axum::http::Response<Body> {
        app.oneshot(
            Request::builder()
                .uri(uri.into())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn read_json<T: serde::de::DeserializeOwned>(response: axum::http::Response<Body>) -> T {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn readiness_endpoint_works() {
        let (app, _) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);

        let ready_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ready_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn bare_operational_endpoints_work_without_api_prefix() {
        let (app, _) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;

        let health_response = get(app.clone(), "/healthz").await;
        assert_eq!(health_response.status(), StatusCode::OK);

        let ready_response = get(app.clone(), "/readyz").await;
        assert_eq!(ready_response.status(), StatusCode::OK);

        let metrics_response = get(app, "/api/metrics").await;
        assert_eq!(metrics_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn init_persists_state_and_workspace() {
        let (app, config) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_telegram(app, telegram_message_payload(1, "/bookbot init")).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: MessageApiResponse = serde_json::from_slice(&body).unwrap();
        let notification = payload.notification.unwrap();
        assert_eq!(
            notification.message,
            "Book workspace is ready for this conversation."
        );
        assert!(notification.reader_url.is_some());

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let conversation = repo.get_conversation("telegram:123456").await.unwrap();
        let book = repo
            .find_book_by_conversation(&conversation.conversation_id)
            .await
            .unwrap();
        let workspace = PathBuf::from(&book.workspace_path);

        assert!(config.data_dir.join("state.json").exists());
        assert!(workspace.exists());
        assert!(workspace.starts_with(&config.books_root));
        assert!(workspace.join("book.yaml").exists());
        assert!(workspace.join("style.yaml").exists());
    }

    #[tokio::test]
    async fn second_init_does_not_create_duplicate_book() {
        let (app, config) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;

        let first = post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        assert_eq!(first.status(), StatusCode::OK);
        let second = post_telegram(app, telegram_message_payload(2, "/bookbot init")).await;
        assert_eq!(second.status(), StatusCode::OK);

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let snapshot = repo.snapshot().await;
        assert_eq!(snapshot.conversations.len(), 1);
        assert_eq!(snapshot.books.len(), 1);
        assert_eq!(snapshot.conversation_to_book.len(), 1);
    }

    #[tokio::test]
    async fn non_setup_command_does_not_implicitly_create_book() {
        let (app, config) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_telegram(
            app,
            telegram_message_payload(1, "@bookbot write a chapter about testing"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: MessageApiResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.notification.unwrap().message,
            "Run init first so this conversation gets its own book workspace."
        );

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let snapshot = repo.snapshot().await;
        assert_eq!(snapshot.conversations.len(), 1);
        assert!(snapshot.books.is_empty());
        assert!(config.books_root.exists());
        assert_eq!(std::fs::read_dir(&config.books_root).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn setup_and_authoring_flow_work_end_to_end() {
        let executor = Arc::new(FakeExecutor::new(|workspace, _| {
            std::fs::write(
                workspace.join("content/chapters/002-new-section.md"),
                "# New Chapter\n\nFresh content.\n",
            )?;
            let mut manifest: serde_yaml::Value =
                serde_yaml::from_slice(&std::fs::read(workspace.join("book.yaml"))?)?;
            manifest["content"]
                .as_sequence_mut()
                .unwrap()
                .push(serde_yaml::from_str(
                    "{id: chapter-2, kind: chapter, file: content/chapters/002-new-section.md}",
                )?);
            std::fs::write(
                workspace.join("book.yaml"),
                serde_yaml::to_string(&manifest)?,
            )?;
            Ok(ExecutionOutcome {
                exit_code: Some(0),
                timed_out: false,
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }));
        let (app, config) = test_app(executor).await;

        let init_response =
            post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        let init_status = init_response.status();
        let init_body = init_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(
            init_status,
            StatusCode::OK,
            "init body: {}",
            String::from_utf8_lossy(&init_body)
        );

        let response = post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot write an additional chapter"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: MessageApiResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.notification.unwrap().reader_url.is_some());

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let job = repo.latest_job_for_book("book-1").await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded);
        assert!(job.prompt_snapshot.contains("Normalized user instruction"));
        assert!(job.prompt_snapshot.contains("write an additional chapter"));
        assert!(
            !job.prompt_snapshot
                .contains("@bookbot write an additional chapter")
        );
        assert_eq!(
            job.changed_files,
            vec![
                "book.yaml".to_string(),
                "content/chapters/002-new-section.md".to_string()
            ]
        );
        assert!(repo.latest_revision_for_book("book-1").await.is_some());
    }

    #[tokio::test]
    async fn full_system_flow_returns_reader_link_and_rendered_html() {
        let executor = Arc::new(FakeExecutor::new(|workspace, _| {
            std::fs::write(
                workspace.join("content/chapters/002-habits.md"),
                "# Habit Chapter\n\nBusy parents can build durable routines with short cues.\n",
            )?;
            let mut manifest: serde_yaml::Value =
                serde_yaml::from_slice(&std::fs::read(workspace.join("book.yaml"))?)?;
            manifest["content"]
                .as_sequence_mut()
                .unwrap()
                .push(serde_yaml::from_str(
                    "{id: chapter-2, kind: chapter, file: content/chapters/002-habits.md}",
                )?);
            std::fs::write(
                workspace.join("book.yaml"),
                serde_yaml::to_string(&manifest)?,
            )?;
            Ok(ExecutionOutcome {
                exit_code: Some(0),
                timed_out: false,
                stdout: "added habit chapter".to_string(),
                stderr: String::new(),
            })
        }));
        let (app, config) = test_app(executor).await;

        let init: MessageApiResponse = read_json(
            post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await,
        )
        .await;
        assert_eq!(
            init.notification.as_ref().unwrap().message,
            "Book workspace is ready for this conversation."
        );

        let authoring: MessageApiResponse = read_json(
            post_telegram(
                app.clone(),
                telegram_message_payload(
                    2,
                    "@bookbot write a chapter about habit formation for busy parents",
                ),
            )
            .await,
        )
        .await;
        let reader_url = authoring
            .notification
            .as_ref()
            .unwrap()
            .reader_url
            .clone()
            .unwrap();

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let revision = repo.latest_revision_for_book("book-1").await.unwrap();
        let job = repo.latest_job_for_book("book-1").await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded);
        assert_eq!(revision.job_id, job.job_id);

        let token = issue_token(&config.reader_token_secret, "book-1", 1).unwrap();
        let summary: ReaderSummary =
            read_json(get(app.clone(), format!("/api/reader/summary?token={token}")).await).await;
        assert_eq!(
            summary.last_revision_id.as_deref(),
            Some(revision.revision_id.as_str())
        );

        let content: ReaderContentResponse = read_json(
            get(
                app.clone(),
                format!(
                    "/api/reader/content?token={token}&revision_id={}",
                    revision.revision_id
                ),
            )
            .await,
        )
        .await;
        assert_eq!(content.chapter_id, "title-page");

        let continuation: ReaderContentResponse = read_json(
            get(
                app.clone(),
                format!(
                    "/api/reader/content?token={token}&cursor={}&revision_id={}",
                    content.next_cursor.clone().unwrap(),
                    revision.revision_id
                ),
            )
            .await,
        )
        .await;
        assert!(
            continuation
                .html
                .contains("This conversation is ready for authoring.")
        );

        let final_chapter: ReaderContentResponse = read_json(
            get(
                app.clone(),
                format!(
                    "/api/reader/content?token={token}&cursor={}&revision_id={}",
                    continuation.next_cursor.clone().unwrap(),
                    revision.revision_id
                ),
            )
            .await,
        )
        .await;
        assert!(
            final_chapter
                .html
                .contains("Busy parents can build durable routines")
        );

        let reader_response = get(app, extract_reader_path(&reader_url)).await;
        assert_eq!(reader_response.status(), StatusCode::OK);
        let reader_body = reader_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let reader_html = String::from_utf8(reader_body.to_vec()).unwrap();
        assert!(reader_html.contains("Untitled Conversation Book"));
        assert!(reader_html.contains("Busy parents can build durable routines"));
        assert!(reader_html.contains(revision.revision_id.as_str()));
    }

    #[tokio::test]
    async fn authoring_flow_keeps_two_conversations_isolated() {
        let executor = Arc::new(FakeExecutor::new(|workspace, prompt| {
            let (file_name, chapter_id, heading, body) = if prompt.contains("lighthouse") {
                (
                    "content/chapters/002-lighthouse.md",
                    "chapter-lighthouse",
                    "Lighthouse Chapter",
                    "A coastal beacon cuts through the fog.",
                )
            } else {
                (
                    "content/chapters/002-garden.md",
                    "chapter-garden",
                    "Garden Chapter",
                    "Tomatoes climb the trellis by the back fence.",
                )
            };
            std::fs::write(
                workspace.join(file_name),
                format!("# {heading}\n\n{body}\n"),
            )?;
            let mut manifest: serde_yaml::Value =
                serde_yaml::from_slice(&std::fs::read(workspace.join("book.yaml"))?)?;
            manifest["content"]
                .as_sequence_mut()
                .unwrap()
                .push(serde_yaml::from_str(&format!(
                    "{{id: {chapter_id}, kind: chapter, file: {file_name}}}"
                ))?);
            std::fs::write(
                workspace.join("book.yaml"),
                serde_yaml::to_string(&manifest)?,
            )?;
            Ok(ExecutionOutcome {
                exit_code: Some(0),
                timed_out: false,
                stdout: heading.to_string(),
                stderr: String::new(),
            })
        }));
        let (app, config) = test_app(executor).await;

        let init_one: MessageApiResponse = read_json(
            post_telegram(
                app.clone(),
                telegram_message_payload_for_chat(123456, 1, "/bookbot init"),
            )
            .await,
        )
        .await;
        let init_two: MessageApiResponse = read_json(
            post_telegram(
                app.clone(),
                telegram_message_payload_for_chat(654321, 2, "/bookbot init"),
            )
            .await,
        )
        .await;
        assert!(init_one.notification.unwrap().reader_url.is_some());
        assert!(init_two.notification.unwrap().reader_url.is_some());

        let first_authoring: MessageApiResponse = read_json(
            post_telegram(
                app.clone(),
                telegram_message_payload_for_chat(123456, 3, "@bookbot write a lighthouse scene"),
            )
            .await,
        )
        .await;
        let second_authoring: MessageApiResponse = read_json(
            post_telegram(
                app.clone(),
                telegram_message_payload_for_chat(654321, 4, "@bookbot write a garden scene"),
            )
            .await,
        )
        .await;

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let first_book = repo
            .find_book_by_conversation("telegram:123456")
            .await
            .unwrap();
        let second_book = repo
            .find_book_by_conversation("telegram:654321")
            .await
            .unwrap();
        assert_ne!(first_book.book_id, second_book.book_id);
        assert_ne!(first_book.workspace_path, second_book.workspace_path);

        let first_token = issue_token(&config.reader_token_secret, &first_book.book_id, 1).unwrap();
        let second_token =
            issue_token(&config.reader_token_secret, &second_book.book_id, 1).unwrap();

        let first_content: ReaderContentResponse = read_json(
            get(
                app.clone(),
                format!(
                    "/api/reader/content?token={first_token}&cursor={}&revision_id={}",
                    encode_cursor(&ChapterCursor {
                        revision_id: repo
                            .latest_revision_for_book(&first_book.book_id)
                            .await
                            .unwrap()
                            .revision_id,
                        chapter_index: 2,
                    }),
                    repo.latest_revision_for_book(&first_book.book_id)
                        .await
                        .unwrap()
                        .revision_id
                ),
            )
            .await,
        )
        .await;
        let second_content: ReaderContentResponse = read_json(
            get(
                app.clone(),
                format!(
                    "/api/reader/content?token={second_token}&cursor={}&revision_id={}",
                    encode_cursor(&ChapterCursor {
                        revision_id: repo
                            .latest_revision_for_book(&second_book.book_id)
                            .await
                            .unwrap()
                            .revision_id,
                        chapter_index: 2,
                    }),
                    repo.latest_revision_for_book(&second_book.book_id)
                        .await
                        .unwrap()
                        .revision_id
                ),
            )
            .await,
        )
        .await;

        assert!(first_content.html.contains("coastal beacon"));
        assert!(!first_content.html.contains("Tomatoes climb the trellis"));
        assert!(second_content.html.contains("Tomatoes climb the trellis"));
        assert!(!second_content.html.contains("coastal beacon"));

        let first_reader = get(
            app.clone(),
            extract_reader_path(
                first_authoring
                    .notification
                    .as_ref()
                    .unwrap()
                    .reader_url
                    .as_ref()
                    .unwrap(),
            ),
        )
        .await;
        let second_reader = get(
            app,
            extract_reader_path(
                second_authoring
                    .notification
                    .as_ref()
                    .unwrap()
                    .reader_url
                    .as_ref()
                    .unwrap(),
            ),
        )
        .await;
        let first_reader_html = String::from_utf8(
            first_reader
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let second_reader_html = String::from_utf8(
            second_reader
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(first_reader_html.contains("coastal beacon"));
        assert!(!first_reader_html.contains("Tomatoes climb the trellis"));
        assert!(second_reader_html.contains("Tomatoes climb the trellis"));
        assert!(!second_reader_html.contains("coastal beacon"));
    }

    #[tokio::test]
    async fn authoring_failure_marks_job_failed_without_revision() {
        let executor = Arc::new(FakeExecutor::new(|_, _| {
            Ok(ExecutionOutcome {
                exit_code: Some(17),
                timed_out: false,
                stdout: String::new(),
                stderr: "tool failed".to_string(),
            })
        }));
        let (app, config) = test_app(executor).await;

        post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot fail this authoring run"),
        )
        .await;

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let job = repo.latest_job_for_book("book-1").await.unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.failure_reason.as_deref(), Some("tool failed"));
        assert_eq!(repo.snapshot().await.revisions.len(), 1);
    }

    #[tokio::test]
    async fn authoring_timeout_marks_job_timed_out_without_revision() {
        let executor = Arc::new(FakeExecutor::new(|_, _| {
            Ok(ExecutionOutcome {
                exit_code: None,
                timed_out: true,
                stdout: String::new(),
                stderr: "codex execution timed out".to_string(),
            })
        }));
        let (app, config) = test_app(executor).await;

        post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot keep writing forever"),
        )
        .await;

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let job = repo.latest_job_for_book("book-1").await.unwrap();
        assert_eq!(job.status, JobStatus::TimedOut);
        assert_eq!(
            job.failure_reason.as_deref(),
            Some("codex execution timed out")
        );
        assert_eq!(repo.snapshot().await.revisions.len(), 1);
    }

    #[tokio::test]
    async fn launcher_failure_marks_job_failed_distinctly() {
        let executor = Arc::new(FakeExecutor::new(|_, _| {
            Err(anyhow::anyhow!("binary missing"))
        }));
        let (app, config) = test_app(executor).await;

        post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        let response = post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot write the next chapter"),
        )
        .await;
        let payload: MessageApiResponse = read_json(response).await;
        assert_eq!(
            payload.notification.unwrap().message,
            "The writing job could not be started. Please try again."
        );

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let job = repo.latest_job_for_book("book-1").await.unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert!(
            job.failure_reason
                .as_deref()
                .unwrap_or_default()
                .contains("launcher failure")
        );
        assert_eq!(repo.snapshot().await.revisions.len(), 1);
    }

    #[tokio::test]
    async fn reader_endpoints_require_valid_token_and_return_content() {
        let executor = Arc::new(FakeExecutor::new(|workspace, _| {
            std::fs::write(
                workspace.join("content/chapters/002-reader.md"),
                "# Reader Chapter\n\nVisible content.\n",
            )?;
            let mut manifest: serde_yaml::Value =
                serde_yaml::from_slice(&std::fs::read(workspace.join("book.yaml"))?)?;
            manifest["content"]
                .as_sequence_mut()
                .unwrap()
                .push(serde_yaml::from_str(
                    "{id: chapter-2, kind: chapter, file: content/chapters/002-reader.md}",
                )?);
            std::fs::write(
                workspace.join("book.yaml"),
                serde_yaml::to_string(&manifest)?,
            )?;
            Ok(ExecutionOutcome {
                exit_code: Some(0),
                timed_out: false,
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }));
        let (app, config) = test_app(executor).await;

        post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;

        post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot add another visible chapter"),
        )
        .await;

        let book_id = "book-1";
        let token = issue_token(&config.reader_token_secret, book_id, 1).unwrap();

        let summary = get(app.clone(), format!("/api/reader/summary?token={token}")).await;
        assert_eq!(summary.status(), StatusCode::OK);
        let summary: ReaderSummary = read_json(summary).await;
        assert!(summary.last_revision_id.is_some());
        assert_eq!(summary.chapter_count, 3);

        let first = get(
            app.clone(),
            format!(
                "/api/reader/content?token={token}&revision_id={}",
                summary.last_revision_id.clone().unwrap()
            ),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);
        let first: ReaderContentResponse = read_json(first).await;
        assert_eq!(first.chapter_id, "title-page");
        assert!(first.next_cursor.is_some());

        let second = get(
            app.clone(),
            format!(
                "/api/reader/content?token={token}&cursor={}&revision_id={}",
                first.next_cursor.clone().unwrap(),
                first.revision_id
            ),
        )
        .await;
        assert_eq!(second.status(), StatusCode::OK);
        let second: ReaderContentResponse = read_json(second).await;
        assert_eq!(second.chapter_id, "chapter-1");

        let third = get(
            app.clone(),
            format!(
                "/api/reader/content?token={token}&cursor={}&revision_id={}",
                second.next_cursor.clone().unwrap(),
                second.revision_id
            ),
        )
        .await;
        assert_eq!(third.status(), StatusCode::OK);
        let third: ReaderContentResponse = read_json(third).await;
        assert_eq!(third.chapter_id, "chapter-2");
        assert!(!third.has_more);

        let revision = get(app.clone(), format!("/api/reader/revision?token={token}")).await;
        assert_eq!(revision.status(), StatusCode::OK);
        let revision: ReaderRevisionResponse = read_json(revision).await;
        assert_eq!(revision.render_status, RevisionRenderStatus::Ready);
        assert!(revision.content_hash.is_some());

        let job = get(app.clone(), format!("/api/reader/job?token={token}")).await;
        assert_eq!(job.status(), StatusCode::OK);
        let job: ReaderJobResponse = read_json(job).await;
        assert_eq!(job.status, JobStatus::Succeeded);
        assert!(job.finished_at.is_some());

        let invalid = get(app, "/api/reader/summary?token=bad-token").await;
        assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
        let invalid: ReaderErrorResponse = read_json(invalid).await;
        assert_eq!(invalid.code, "access_denied");
    }

    #[tokio::test]
    async fn reader_endpoints_reject_unknown_and_stale_book_revisions() {
        let counter = Arc::new(AtomicUsize::new(0));
        let executor = {
            let counter = counter.clone();
            Arc::new(FakeExecutor::new(move |workspace, _| {
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let file = format!("content/chapters/00{}-extra.md", current + 1);
                std::fs::write(
                    workspace.join(&file),
                    format!("# Chapter {current}\n\nVisible content {current}.\n"),
                )?;
                let mut manifest: serde_yaml::Value =
                    serde_yaml::from_slice(&std::fs::read(workspace.join("book.yaml"))?)?;
                manifest["content"]
                    .as_sequence_mut()
                    .unwrap()
                    .push(serde_yaml::from_str(&format!(
                        "{{id: chapter-{current}, kind: chapter, file: {file}}}"
                    ))?);
                std::fs::write(
                    workspace.join("book.yaml"),
                    serde_yaml::to_string(&manifest)?,
                )?;
                Ok(ExecutionOutcome {
                    exit_code: Some(0),
                    timed_out: false,
                    stdout: "ok".to_string(),
                    stderr: String::new(),
                })
            }))
        };
        let (app, config) = test_app(executor).await;

        post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot add one more chapter"),
        )
        .await;

        let token = issue_token(&config.reader_token_secret, "book-1", 1).unwrap();
        let summary = get(app.clone(), format!("/api/reader/summary?token={token}")).await;
        let summary: ReaderSummary = read_json(summary).await;
        let stale_revision_id = summary.last_revision_id.unwrap();

        post_telegram(
            app.clone(),
            telegram_message_payload(3, "@bookbot add yet another chapter"),
        )
        .await;

        let stale = get(
            app.clone(),
            format!("/api/reader/content?token={token}&revision_id={stale_revision_id}"),
        )
        .await;
        assert_eq!(stale.status(), StatusCode::CONFLICT);
        let stale: ReaderErrorResponse = read_json(stale).await;
        assert_eq!(stale.code, "stale_revision");

        let missing_book_token = issue_token(&config.reader_token_secret, "book-999", 1).unwrap();
        let missing = get(
            app,
            format!("/api/reader/summary?token={missing_book_token}"),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
        let missing: ReaderErrorResponse = read_json(missing).await;
        assert_eq!(missing.code, "access_denied");
    }

    #[tokio::test]
    async fn reader_endpoints_surface_render_failures_explicitly() {
        let executor = Arc::new(FakeExecutor::new(|workspace, _| {
            std::fs::write(workspace.join("book.yaml"), "title: broken\ncontent: [")?;
            Ok(ExecutionOutcome {
                exit_code: Some(0),
                timed_out: false,
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }));
        let (app, config) = test_app(executor).await;

        post_telegram(app.clone(), telegram_message_payload(1, "/bookbot init")).await;
        post_telegram(
            app.clone(),
            telegram_message_payload(2, "@bookbot make the render fail"),
        )
        .await;

        let token = issue_token(&config.reader_token_secret, "book-1", 1).unwrap();

        let revision = get(app.clone(), format!("/api/reader/revision?token={token}")).await;
        assert_eq!(revision.status(), StatusCode::OK);
        let revision: ReaderRevisionResponse = read_json(revision).await;
        assert_eq!(revision.render_status, RevisionRenderStatus::Ready);
        assert!(revision.render_error.is_none());

        let job = get(app.clone(), format!("/api/reader/job?token={token}")).await;
        assert_eq!(job.status(), StatusCode::OK);
        let job: ReaderJobResponse = read_json(job).await;
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(
            job.user_facing_message.as_deref(),
            Some("The draft changed, but render refresh failed.")
        );

        let content = get(app, format!("/api/reader/content?token={token}")).await;
        assert_eq!(content.status(), StatusCode::OK);
        let content: ReaderContentResponse = read_json(content).await;
        assert_eq!(content.chapter_id, "title-page");
    }

    #[tokio::test]
    async fn ignores_non_bot_messages() {
        let (app, _) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_telegram(app, fixture_payload("telegram-ignored")).await;
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: MessageApiResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.ignored);
    }

    #[tokio::test]
    async fn pre_setup_authoring_is_rejected() {
        let (app, _) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_telegram(app, fixture_payload("telegram-authoring-reply")).await;
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            status,
            StatusCode::OK,
            "body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: MessageApiResponse = serde_json::from_slice(&body).unwrap();
        assert!(
            payload
                .notification
                .unwrap()
                .message
                .contains("Run init first")
        );
    }

    #[tokio::test]
    async fn setup_command_is_handled_in_rust_for_telegram_fixture() {
        let (app, config) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_telegram(app, fixture_payload("telegram-init")).await;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: MessageApiResponse = read_json(response).await;
        assert!(payload.handled);
        assert!(!payload.ignored);
        assert_eq!(
            payload.notification.unwrap().message,
            "Book workspace is ready for this conversation."
        );

        let repo = Repository::load(&config.data_dir).await.unwrap();
        assert!(
            repo.find_book_by_conversation("telegram:123456")
                .await
                .is_some()
        );
        assert_eq!(
            repo.latest_job_for_book("book-1")
                .await
                .unwrap()
                .command_kind,
            CommandKind::Init
        );
    }

    #[tokio::test]
    async fn status_command_is_handled_in_rust_for_max_fixture() {
        let (app, _) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;

        let init = post_max(app.clone(), fixture_payload("max-init")).await;
        assert_eq!(init.status(), StatusCode::OK);

        let response = post_max(app, fixture_payload("max-status")).await;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: MessageApiResponse = read_json(response).await;
        let notification = payload.notification.unwrap();
        assert!(notification.message.contains("Book status"));
        assert!(notification.reader_url.is_some());
    }

    #[tokio::test]
    async fn max_non_bot_messages_are_ignored() {
        let (app, config) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_max(app, fixture_payload("max-ignored")).await;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: MessageApiResponse = read_json(response).await;
        assert!(payload.ignored);
        assert!(payload.notification.is_none());

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let snapshot = repo.snapshot().await;
        assert!(snapshot.conversations.is_empty());
        assert!(snapshot.books.is_empty());
    }

    #[tokio::test]
    async fn max_pre_setup_authoring_is_rejected() {
        let (app, config) = test_app(Arc::new(FakeExecutor::new(|_, _| unreachable!()))).await;
        let response = post_max(app, fixture_payload("max-authoring")).await;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: MessageApiResponse = read_json(response).await;
        assert_eq!(
            payload.notification.unwrap().message,
            "Run init first so this conversation gets its own book workspace."
        );

        let repo = Repository::load(&config.data_dir).await.unwrap();
        let snapshot = repo.snapshot().await;
        assert_eq!(snapshot.conversations.len(), 1);
        assert!(snapshot.books.is_empty());
    }
}
