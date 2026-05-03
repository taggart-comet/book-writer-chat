use std::{collections::HashMap, path::Path, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{app::metrics::Metrics, core::config::Config, storage::repository::Repository};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub repository: Repository,
    pub metrics: Metrics,
    pub conversation_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    pub session_launcher: DynSessionLauncher,
}

pub async fn conversation_lock(state: &AppState, conversation_id: &str) -> Arc<Mutex<()>> {
    let mut locks = state.conversation_locks.lock().await;
    locks
        .entry(conversation_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLaunchResult {
    pub session_id: String,
    pub session_log_path: PathBuf,
    pub launched_at: DateTime<Utc>,
}

#[async_trait]
pub trait SessionLauncher: Send + Sync {
    async fn launch(
        &self,
        workspace: &Path,
        title: &str,
        initial_prompt: &str,
    ) -> Result<SessionLaunchResult>;

    async fn resume(
        &self,
        workspace: &Path,
        session_id: &str,
        prompt: &str,
    ) -> Result<SessionLaunchResult>;
}

pub type DynSessionLauncher = Arc<dyn SessionLauncher>;

pub struct RealSessionLauncher {
    config: Config,
}

impl RealSessionLauncher {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SessionLauncher for RealSessionLauncher {
    async fn launch(
        &self,
        workspace: &Path,
        title: &str,
        initial_prompt: &str,
    ) -> Result<SessionLaunchResult> {
        self.run_exec(workspace, title, None, initial_prompt).await
    }

    async fn resume(
        &self,
        workspace: &Path,
        session_id: &str,
        prompt: &str,
    ) -> Result<SessionLaunchResult> {
        self.run_exec(workspace, session_id, Some(session_id), prompt)
            .await
    }
}

impl RealSessionLauncher {
    async fn run_exec(
        &self,
        workspace: &Path,
        title: &str,
        session_id: Option<&str>,
        prompt: &str,
    ) -> Result<SessionLaunchResult> {
        use std::{env, process::Stdio};

        use tokio::{io::AsyncReadExt, process::Command};
        use walkdir::WalkDir;

        tracing::info!(
            workspace = %workspace.display(),
            title,
            session_id = session_id.unwrap_or(""),
            codex_cli_path = %self.config.codex_cli_path,
            "starting codex exec"
        );

        let mut command = Command::new(&self.config.codex_cli_path);
        command
            .args(build_codex_exec_args(
                &self.config.codex_cli_args,
                workspace,
                session_id,
                prompt,
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().context("failed to launch codex session")?;

        let mut stdout = child
            .stdout
            .take()
            .context("failed to capture codex session stdout")?;
        let mut stderr = child
            .stderr
            .take()
            .context("failed to capture codex session stderr")?;

        let stdout_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            stdout.read_to_end(&mut buffer).await?;
            Ok::<String, std::io::Error>(String::from_utf8_lossy(&buffer).to_string())
        });
        let stderr_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            stderr.read_to_end(&mut buffer).await?;
            Ok::<String, std::io::Error>(String::from_utf8_lossy(&buffer).to_string())
        });

        let status = child.wait().await?;
        let stdout = stdout_task.await??;
        let stderr = stderr_task.await??;

        if status.success() {
            tracing::info!(
                workspace = %workspace.display(),
                title,
                session_id = session_id.unwrap_or(""),
                stdout = stdout.trim(),
                stderr = stderr.trim(),
                "codex exec completed"
            );
        } else {
            tracing::error!(
                workspace = %workspace.display(),
                title,
                session_id = session_id.unwrap_or(""),
                exit_code = status.code().unwrap_or_default(),
                stdout = stdout.trim(),
                stderr = stderr.trim(),
                "codex exec failed"
            );
        }

        anyhow::ensure!(
            status.success(),
            "codex session launch failed for `{title}`: {}",
            stderr.trim()
        );

        let (session_id, launched_at) = parse_session_launch_output(&stdout).map_err(|error| {
            tracing::error!(
                workspace = %workspace.display(),
                title,
                stdout = stdout.trim(),
                stderr = stderr.trim(),
                error = %error,
                "failed to parse codex exec output"
            );
            error
        })?;
        let launched_at = launched_at.unwrap_or_else(Utc::now);

        let home = env::var("HOME").context("HOME is required to resolve Codex session logs")?;
        let sessions_root = PathBuf::from(home).join(".codex/sessions");
        let session_log_path = WalkDir::new(&sessions_root)
            .into_iter()
            .filter_map(Result::ok)
            .find_map(|entry| {
                let path = entry.path();
                let file_name = path.file_name()?.to_str()?;
                if entry.file_type().is_file()
                    && file_name.contains(&session_id)
                    && file_name.ends_with(".jsonl")
                {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
            .with_context(|| format!("failed to locate session log for `{session_id}`"))
            .map_err(|error| {
                tracing::error!(
                    workspace = %workspace.display(),
                    session_id,
                    sessions_root = %sessions_root.display(),
                    error = %error,
                    "failed to locate codex session log"
                );
                error
            })?;

        tracing::info!(
            workspace = %workspace.display(),
            session_id,
            session_log_path = %session_log_path.display(),
            "resolved codex session log path"
        );

        Ok(SessionLaunchResult {
            session_id,
            session_log_path,
            launched_at,
        })
    }
}

fn build_codex_exec_args(
    codex_cli_args: &[String],
    workspace: &Path,
    session_id: Option<&str>,
    prompt: &str,
) -> Vec<std::ffi::OsString> {
    let mut args = codex_cli_args
        .iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
    args.push(std::ffi::OsString::from("-C"));
    args.push(workspace.as_os_str().to_os_string());
    args.push(std::ffi::OsString::from("exec"));
    if let Some(session_id) = session_id {
        args.push(std::ffi::OsString::from("resume"));
        args.push(std::ffi::OsString::from(session_id));
    }
    args.push(std::ffi::OsString::from("--json"));
    args.push(std::ffi::OsString::from("--skip-git-repo-check"));
    args.push(std::ffi::OsString::from(prompt));
    args
}

fn parse_session_launch_output(stdout: &str) -> Result<(String, Option<DateTime<Utc>>)> {
    let mut session_id = None;
    let mut launched_at = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line)
            .with_context(|| "failed to parse Codex session launch output")?;

        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            let payload = value.get("payload").and_then(Value::as_object);
            session_id = payload
                .and_then(|payload| payload.get("id"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            launched_at = payload
                .and_then(|payload| payload.get("timestamp"))
                .and_then(Value::as_str)
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc));
            if session_id.is_some() {
                break;
            }
        }

        if value.get("type").and_then(Value::as_str) == Some("thread.started") {
            session_id = value
                .get("thread_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            if session_id.is_some() {
                break;
            }
        }
    }

    Ok((
        session_id.context("codex launch did not report a session id")?,
        launched_at,
    ))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::TimeZone;

    use super::{build_codex_exec_args, parse_session_launch_output};

    #[test]
    fn parses_legacy_session_meta_output() {
        let stdout = r#"{"type":"session_meta","payload":{"id":"session-42","timestamp":"2026-05-02T06:14:13Z"}}"#;

        let (session_id, launched_at) = parse_session_launch_output(stdout).unwrap();

        assert_eq!(session_id, "session-42");
        assert_eq!(
            launched_at,
            Some(chrono::Utc.with_ymd_and_hms(2026, 5, 2, 6, 14, 13).unwrap())
        );
    }

    #[test]
    fn parses_current_thread_started_output() {
        let stdout =
            r#"{"type":"thread.started","thread_id":"019de752-b0b3-7e02-875f-3089d4539752"}"#;

        let (session_id, launched_at) = parse_session_launch_output(stdout).unwrap();

        assert_eq!(session_id, "019de752-b0b3-7e02-875f-3089d4539752");
        assert_eq!(launched_at, None);
    }

    #[test]
    fn builds_resume_args_with_global_cd_before_exec() {
        let args = build_codex_exec_args(
            &["--profile".to_string(), "test".to_string()],
            Path::new("/tmp/workspace"),
            Some("session-42"),
            "Continue the draft",
        );
        let args = args
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "--profile",
                "test",
                "-C",
                "/tmp/workspace",
                "exec",
                "resume",
                "session-42",
                "--json",
                "--skip-git-repo-check",
                "Continue the draft",
            ]
        );
    }
}
