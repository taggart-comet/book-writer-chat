use std::{path::Path, process::Stdio, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use crate::core::config::Config;

#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub workspace: std::path::PathBuf,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionOutcome>;
}

pub type DynExecutor = Arc<dyn AgentExecutor>;

pub struct RealExecutor {
    config: Config,
}

impl RealExecutor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl AgentExecutor for RealExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionOutcome> {
        // MVP contract: the backend passes the prompt package over stdin and executes
        // Codex CLI inside the conversation-owned workspace.
        let mut command = Command::new(&self.config.codex_cli_path);
        command
            .args(&self.config.codex_cli_args)
            .current_dir(&request.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(request.prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture codex stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture codex stderr"))?;

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

        let timeout_duration = Duration::from_secs(self.config.agent_timeout_secs);
        let status = tokio::select! {
            output = child.wait() => Some(output?),
            _ = tokio::time::sleep(timeout_duration) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                None
            }
        };

        let stdout = stdout_task.await??;
        let mut stderr = stderr_task.await??;

        if let Some(status) = status {
            Ok(ExecutionOutcome {
                exit_code: status.code(),
                timed_out: false,
                stdout,
                stderr,
            })
        } else {
            if stderr.is_empty() {
                stderr = "codex execution timed out".to_string();
            }
            Ok(ExecutionOutcome {
                exit_code: None,
                timed_out: true,
                stdout,
                stderr,
            })
        }
    }
}

pub struct FakeExecutor {
    handler: Arc<dyn Fn(&Path, &str) -> Result<ExecutionOutcome> + Send + Sync>,
}

impl FakeExecutor {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&Path, &str) -> Result<ExecutionOutcome> + Send + Sync + 'static,
    {
        Self {
            handler: Arc::new(handler),
        }
    }
}

#[async_trait]
impl AgentExecutor for FakeExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionOutcome> {
        (self.handler)(&request.workspace, &request.prompt)
    }
}
