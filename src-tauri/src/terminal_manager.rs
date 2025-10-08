use serde::Serialize;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct TerminalOutput {
    pub index: usize,
    pub line: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct TerminalReadResult {
    pub output: Vec<TerminalOutput>,
    pub next_index: usize,
    pub closed: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct TerminalSummary {
    pub terminal_id: String,
    pub session_id: Option<String>,
    #[serde(skip)]
    pub created_at: std::time::Instant,
    #[serde(skip)]
    pub last_accessed: std::time::Instant,
    pub closed: bool,
}

pub struct TerminalSession {
    pub terminal_id: String,
    pub session_id: Option<String>,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<Mutex<std::time::Instant>>,
    pub env: HashMap<String, String>,
    child: Arc<Mutex<Child>>,
    output_buffer: Arc<RwLock<Vec<String>>>,
    closed: Arc<tokio::sync::watch::Sender<bool>>,
    close_watch: tokio::sync::watch::Receiver<bool>,
}

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalSession")
            .field("terminal_id", &self.terminal_id)
            .field("session_id", &self.session_id)
            .field("created_at", &self.created_at)
            .field("last_accessed", &self.last_accessed)
            .field("env", &self.env)
            .field("output_buffer", &self.output_buffer)
            .finish()
    }
}

impl TerminalSession {
    fn new(
        terminal_id: String,
        session_id: Option<String>,
        shell: Option<String>,
        env: Option<HashMap<String, String>>,
    ) -> Result<Self, String> {
        let shell_cmd = shell.unwrap_or_else(|| {
            if cfg!(windows) {
                "cmd.exe".to_string()
            } else {
                "sh".to_string()
            }
        });

        let mut command = Command::new(&shell_cmd);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let effective_env = env.unwrap_or_default();
        for (key, value) in &effective_env {
            command.env(key, value);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to spawn shell '{shell_cmd}': {e}"))?;

        let stdout = child.stdout.take().expect("Failed to open stdout");

        let output_buffer = Arc::new(RwLock::new(Vec::new()));
        let (closed_tx, closed_rx) = tokio::sync::watch::channel(false);

        // Background task to read stdout
        let buffer_clone = output_buffer.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut buffer = buffer_clone.write().await;
                buffer.push(line);
            }
        });

        Ok(Self {
            terminal_id,
            session_id,
            created_at: std::time::Instant::now(),
            last_accessed: Arc::new(Mutex::new(std::time::Instant::now())),
            env: effective_env,
            child: Arc::new(Mutex::new(child)),
            output_buffer,
            closed: Arc::new(closed_tx),
            close_watch: closed_rx,
        })
    }

    pub async fn write(&self, input: &str) -> Result<(), String> {
        let mut child_guard = self.child.lock().await;
        if let Some(stdin) = child_guard.stdin.as_mut() {
            let command = if !input.ends_with('\n') {
                format!("{input}\n")
            } else {
                input.to_string()
            };
            stdin
                .write_all(command.as_bytes())
                .await
                .map_err(|e| format!("Failed to write to terminal: {e}"))?;
            *self.last_accessed.lock().await = std::time::Instant::now();
            Ok(())
        } else {
            Err("Terminal stdin is not available.".to_string())
        }
    }

    pub async fn read(&self, since_index: Option<usize>) -> Result<TerminalReadResult, String> {
        let start_index = since_index.unwrap_or(0);
        let buffer = self.output_buffer.read().await;
        let next_index = buffer.len();

        let new_output = buffer[start_index..]
            .iter()
            .enumerate()
            .map(|(i, line)| TerminalOutput {
                index: start_index + i,
                line: line.clone(),
            })
            .collect();

        *self.last_accessed.lock().await = std::time::Instant::now();
        Ok(TerminalReadResult {
            output: new_output,
            next_index,
            closed: *self.close_watch.borrow(),
        })
    }

    pub async fn close(&self) -> Result<(), String> {
        let mut child = self.child.lock().await;
        child
            .kill()
            .await
            .map_err(|e| format!("Failed to kill terminal process: {e}"))?;
        let _ = self.closed.send(true);
        info!("Closed terminal {}", self.terminal_id);
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        *self.close_watch.borrow()
    }

    pub async fn summary(&self) -> TerminalSummary {
        TerminalSummary {
            terminal_id: self.terminal_id.clone(),
            session_id: self.session_id.clone(),
            created_at: self.created_at,
            last_accessed: *self.last_accessed.lock().await,
            closed: self.is_closed(),
        }
    }
}

#[derive(Default, Debug)]
pub struct TerminalManager {
    terminals: Arc<RwLock<HashMap<String, Arc<TerminalSession>>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn open_new_terminal(
        &self,
        session_id: Option<String>,
        shell: Option<String>,
        env: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        let terminal_id = Uuid::new_v4().to_string();
        let session = Arc::new(TerminalSession::new(
            terminal_id.clone(),
            session_id,
            shell,
            env,
        )?);
        self.terminals
            .write()
            .await
            .insert(terminal_id.clone(), session);
        info!("Opened new terminal: {}", terminal_id);
        Ok(terminal_id)
    }

    pub async fn write_to_terminal(&self, terminal_id: &str, input: &str) -> Result<(), String> {
        let terminals = self.terminals.read().await;
        if let Some(session) = terminals.get(terminal_id) {
            session.write(input).await
        } else {
            Err(format!("Terminal '{terminal_id}' not found"))
        }
    }

    pub async fn read_terminal_output(
        &self,
        terminal_id: &str,
        since_index: Option<usize>,
    ) -> Result<TerminalReadResult, String> {
        let terminals = self.terminals.read().await;
        if let Some(session) = terminals.get(terminal_id) {
            session.read(since_index).await
        } else {
            Err(format!("Terminal '{terminal_id}' not found"))
        }
    }

    pub async fn close_terminal(&self, terminal_id: &str) -> Result<(), String> {
        let mut terminals = self.terminals.write().await;
        if let Some(session) = terminals.remove(terminal_id) {
            session.close().await
        } else {
            Err(format!("Terminal '{terminal_id}' not found"))
        }
    }

    pub async fn list_terminals(&self, session_id: Option<&str>) -> Vec<TerminalSummary> {
        let terminals = self.terminals.read().await;
        let mut summaries = Vec::new();
        for s in terminals.values() {
            if session_id.is_none() || s.session_id.as_deref() == session_id {
                summaries.push(s.summary().await);
            }
        }
        summaries
    }
}
