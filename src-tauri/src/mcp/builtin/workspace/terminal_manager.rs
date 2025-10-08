use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::mcp::builtin::workspace::utils::constants::{
    MAX_ACTIVE_TERMINALS, MAX_TERMINAL_BUFFER_LINES, MAX_TERMINAL_BUFFER_SIZE,
};
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel, SessionIsolationManager};

// Represents a single chunk of output from a terminal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalChunk {
    pub seq: u64,
    pub text: String,
    pub is_stderr: bool,
    pub timestamp: u64,
}

// Represents an active terminal session
#[derive(Debug)]
pub struct Terminal {
    pub id: String,
    pub command: Mutex<String>,
    pub args: Mutex<Vec<String>>,
    pub created_at: u64,
    pub session_id: String,

    // Use a oneshot channel to signal termination
    kill_sender: Mutex<Option<oneshot::Sender<()>>>,

    output_buffer: Mutex<VecDeque<TerminalChunk>>,
    next_seq: AtomicU64,

    running: AtomicBool,
    exit_code: Mutex<Option<i32>>,

    max_buffer_size: usize,
    current_buffer_size: AtomicU64,
}

impl Terminal {
    fn new(
        id: String,
        command: String,
        args: Vec<String>,
        session_id: String,
        kill_sender: oneshot::Sender<()>,
    ) -> Self {
        Self {
            id,
            command: Mutex::new(command),
            args: Mutex::new(args),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            session_id,
            kill_sender: Mutex::new(Some(kill_sender)),
            output_buffer: Mutex::new(VecDeque::with_capacity(MAX_TERMINAL_BUFFER_LINES)),
            next_seq: AtomicU64::new(0),
            running: AtomicBool::new(true),
            exit_code: Mutex::new(None),
            max_buffer_size: MAX_TERMINAL_BUFFER_SIZE,
            current_buffer_size: AtomicU64::new(0),
        }
    }

    async fn add_chunk(&self, text: String, is_stderr: bool) {
        let mut buffer = self.output_buffer.lock().await;
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let chunk_size = text.len();

        let new_chunk = TerminalChunk {
            seq,
            text,
            is_stderr,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        buffer.push_back(new_chunk);
        self.current_buffer_size
            .fetch_add(chunk_size as u64, Ordering::SeqCst);

        while self.current_buffer_size.load(Ordering::SeqCst) > self.max_buffer_size as u64
            || buffer.len() > MAX_TERMINAL_BUFFER_LINES
        {
            if let Some(old_chunk) = buffer.pop_front() {
                self.current_buffer_size
                    .fetch_sub(old_chunk.text.len() as u64, Ordering::SeqCst);
            } else {
                break;
            }
        }
    }
}

#[derive(Debug)]
pub struct TerminalManager {
    terminals: RwLock<HashMap<String, Arc<Terminal>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            terminals: RwLock::new(HashMap::new()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn open_terminal(
        &self,
        command: String,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
        env: HashMap<String, String>,
        isolation_level: IsolationLevel,
        session_id: String,
        isolation_manager: &SessionIsolationManager,
    ) -> Result<String, String> {
        let mut terminals = self.terminals.write().await;
        if terminals.len() >= MAX_ACTIVE_TERMINALS {
            return Err("Maximum number of active terminals reached".to_string());
        }

        let mut cmd = isolation_manager
            .create_isolated_command(IsolatedProcessConfig {
                session_id: session_id.clone(),
                workspace_path: working_dir.unwrap_or_default(),
                command: command.clone(),
                args: args.clone(),
                env_vars: env,
                isolation_level,
            })
            .await?;

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn command: {e}"))?;

        let terminal_id = Uuid::new_v4().to_string();
        let (kill_tx, kill_rx) = oneshot::channel();

        let terminal = Arc::new(Terminal::new(
            terminal_id.clone(),
            command,
            args,
            session_id,
            kill_tx,
        ));

        Self::spawn_output_readers(terminal.clone(), &mut child);
        Self::spawn_process_monitor(terminal.clone(), child, kill_rx);

        terminals.insert(terminal_id.clone(), terminal);
        Ok(terminal_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn attach_process_to_terminal(
        &self,
        terminal_id: &str,
        command: String,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
        env: HashMap<String, String>,
        isolation_level: IsolationLevel,
        session_id: String,
        isolation_manager: &SessionIsolationManager,
    ) -> Result<(), String> {
        let terminal = {
            let terminals = self.terminals.read().await;
            terminals.get(terminal_id).cloned()
        }
        .ok_or_else(|| "Terminal not found".to_string())?;

        if terminal.session_id != session_id {
            return Err("Access denied: Terminal belongs to another session".to_string());
        }

        if terminal.running.load(Ordering::SeqCst) {
            return Err("Terminal is already running a process.".to_string());
        }

        // Reset terminal state
        *terminal.command.lock().await = command.clone();
        *terminal.args.lock().await = args.clone();
        *terminal.exit_code.lock().await = None;
        terminal.next_seq.store(0, Ordering::SeqCst);
        terminal.current_buffer_size.store(0, Ordering::SeqCst);
        terminal.output_buffer.lock().await.clear();

        let mut cmd = isolation_manager
            .create_isolated_command(IsolatedProcessConfig {
                session_id,
                workspace_path: working_dir.unwrap_or_default(),
                command,
                args,
                env_vars: env,
                isolation_level,
            })
            .await?;

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn command: {e}"))?;

        let (kill_tx, kill_rx) = oneshot::channel();
        *terminal.kill_sender.lock().await = Some(kill_tx);

        Self::spawn_output_readers(terminal.clone(), &mut child);
        Self::spawn_process_monitor(terminal.clone(), child, kill_rx);

        terminal.running.store(true, Ordering::SeqCst);

        Ok(())
    }

    pub async fn read_terminal(
        &self,
        terminal_id: &str,
        from_seq: u64,
    ) -> Result<TerminalOutput, String> {
        let terminal = {
            let terminals = self.terminals.read().await;
            terminals.get(terminal_id).cloned()
        };

        if let Some(terminal) = terminal {
            let buffer = terminal.output_buffer.lock().await;
            let outputs = buffer
                .iter()
                .filter(|chunk| chunk.seq >= from_seq)
                .cloned()
                .collect();

            let next_seq = terminal.next_seq.load(Ordering::SeqCst);
            let running = terminal.running.load(Ordering::SeqCst);
            let exit_code = *terminal.exit_code.lock().await;

            Ok(TerminalOutput {
                terminal_id: terminal_id.to_string(),
                outputs,
                next_seq,
                running,
                exit_code,
            })
        } else {
            Err("Terminal not found".to_string())
        }
    }

    pub async fn close_terminal(&self, terminal_id: &str) -> Result<TerminalCloseResult, String> {
        let terminal = {
            let mut terminals = self.terminals.write().await;
            terminals.remove(terminal_id)
        };

        if let Some(terminal) = terminal {
            let was_running = terminal.running.load(Ordering::SeqCst);
            if let Some(kill_tx) = terminal.kill_sender.lock().await.take() {
                let _ = kill_tx.send(());
            }
            let exit_code = *terminal.exit_code.lock().await;
            Ok(TerminalCloseResult {
                terminal_id: terminal_id.to_string(),
                exit_code,
                was_running,
            })
        } else {
            Err("Terminal not found".to_string())
        }
    }

    pub async fn list_terminals(&self, session_id: Option<&str>) -> Vec<TerminalSummary> {
        let mut summaries = Vec::new();
        for t in self.terminals.read().await.values() {
            if session_id.is_none() || session_id == Some(&t.session_id) {
                summaries.push(TerminalSummary {
                    id: t.id.clone(),
                    command: t.command.lock().await.clone(),
                    running: t.running.load(Ordering::SeqCst),
                    created_at: t.created_at,
                    buffer_size: t.current_buffer_size.load(Ordering::SeqCst) as usize,
                    last_seq: t.next_seq.load(Ordering::SeqCst),
                });
            }
        }
        summaries
    }

    fn spawn_output_readers(terminal: Arc<Terminal>, child: &mut Child) {
        let stdout = child.stdout.take().expect("Stdout not captured");
        let stderr = child.stderr.take().expect("Stderr not captured");

        let stdout_terminal = terminal.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                stdout_terminal.add_chunk(line, false).await;
            }
        });

        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                terminal.add_chunk(line, true).await;
            }
        });
    }

    fn spawn_process_monitor(
        terminal: Arc<Terminal>,
        mut child: Child,
        kill_rx: oneshot::Receiver<()>,
    ) {
        tokio::spawn(async move {
            tokio::select! {
                result = child.wait() => {
                    match result {
                        Ok(status) => *terminal.exit_code.lock().await = status.code(),
                        Err(e) => {
                            error!("Failed to wait for process: {}", e);
                            *terminal.exit_code.lock().await = Some(-1);
                        }
                    }
                },
                _ = kill_rx => {
                    if let Err(e) = child.kill().await {
                        warn!("Failed to kill process {}: {}", terminal.id, e);
                    }
                    *terminal.exit_code.lock().await = Some(-1);
                }
            }
            terminal.running.store(false, Ordering::SeqCst);
            info!("Terminal {} process terminated.", terminal.id);
        });
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminalOutput {
    pub terminal_id: String,
    pub outputs: Vec<TerminalChunk>,
    pub next_seq: u64,
    pub running: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminalCloseResult {
    pub terminal_id: String,
    pub exit_code: Option<i32>,
    pub was_running: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminalSummary {
    pub id: String,
    pub command: String,
    pub running: bool,
    pub created_at: u64,
    pub buffer_size: usize,
    pub last_seq: u64,
}
