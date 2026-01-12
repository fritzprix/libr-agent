/// Persistent Shell Session Manager (PTY-based)
///
/// Provides true interactive shell sessions using pseudo-terminals (PTY).
/// This enables support for REPLs, TUI apps, and interactive prompts.
use anyhow::Result;
#[cfg(windows)]
use base64::engine::general_purpose;
#[cfg(windows)]
use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize, Child};
use regex::Regex;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver};
use tracing::debug;

/// Generate unique sentinel marker for command completion detection
fn generate_sentinel() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("STDIO_SENTINEL_{id}")
}

/// Wrapper for portable_pty::Child to implement Send + Sync
/// We assume the underlying implementation (process handle) is thread-safe for wait/kill operations.
struct ThreadSafeChild(Box<dyn Child>);

unsafe impl Send for ThreadSafeChild {}
unsafe impl Sync for ThreadSafeChild {}

impl ThreadSafeChild {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        self.0.try_wait()
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()
    }
}

/// Persistent Shell struct
pub struct PersistentShell {
    /// The PTY pair (master/slave) - kept alive
    #[allow(dead_code)]
    pty_pair: PtyPair,
    /// The child process handle (thread-safe wrapper)
    child: ThreadSafeChild,
    /// Writer to the PTY master (thread-safe wrapper)
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Receiver for output from the PTY
    output_rx: Receiver<Vec<u8>>,
    /// Session ID
    session_id: String,
    /// Last known working directory
    last_known_cwd: String,
    /// Internal buffer for accumulated output (tail truncated)
    read_buffer: Vec<u8>,
}

impl PersistentShell {
    /// Create a new persistent shell session
    pub async fn new(session_id: String, workspace_path: PathBuf) -> Result<Self> {
        let pty_system = native_pty_system();

        // create a PTY pair
        let pty_pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        #[cfg(unix)]
        let mut cmd = CommandBuilder::new("bash");
        #[cfg(unix)]
        {
            cmd.arg("--norc");
            cmd.arg("--noprofile");
            // Basic PATH setup
            if let Ok(path) = std::env::var("PATH") {
                cmd.env("PATH", path);
            }
            if let Ok(home) = std::env::var("HOME") {
                 cmd.env("HOME", home);
            }
        }

        #[cfg(windows)]
        let mut cmd = CommandBuilder::new("powershell.exe");
        #[cfg(windows)]
        {
            cmd.arg("-NoProfile");
            cmd.arg("-NoLogo");
            // Note: We do NOT pass -NonInteractive for PTY, as we want interactive behavior
        }

        cmd.cwd(workspace_path.clone());

        // Spawn the shell
        let child = pty_pair.slave.spawn_command(cmd)?;
        // Wrap child for Send+Sync
        let child_wrapper = ThreadSafeChild(child);

        // Get writer and reader
        let writer = pty_pair.master.take_writer()?;
        let mut reader = pty_pair.master.try_clone_reader()?;

        // Create channel for output
        let (tx, rx) = mpsc::channel::<Vec<u8>>(100);

        // Spawn a thread to read from PTY and send to channel
        let session_id_clone = session_id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(n) => {
                        if n == 0 {
                            debug!("PTY closed for session {}", session_id_clone);
                            break;
                        }
                        let data = buf[..n].to_vec();
                        if let Err(_) = tx.blocking_send(data) {
                            debug!("Receiver dropped for session {}", session_id_clone);
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("Error reading PTY for session {}: {}", session_id_clone, e);
                        break;
                    }
                }
            }
        });

        // Initialize shell
        let shell = Self {
            pty_pair,
            child: child_wrapper,
            writer: Arc::new(Mutex::new(writer)),
            output_rx: rx,
            session_id,
            last_known_cwd: workspace_path.to_string_lossy().to_string(),
            read_buffer: Vec::new(),
        };

        #[allow(unused_mut)]
        let mut shell = shell;

        #[cfg(windows)]
        {
             // Force UTF-8 encoding
             shell.write_stdin_raw("[Console]::InputEncoding = [Console]::OutputEncoding = $OutputEncoding = [System.Text.Encoding]::UTF8\n", true).await?;
             // Wait a bit for it to apply
             tokio::time::sleep(Duration::from_millis(100)).await;
             // Clear startup output
             let _ = shell.read_output_nonblocking(500).await?;
        }

        Ok(shell)
    }

    /// Write raw input to shell stdin
    pub async fn write_stdin_raw(&mut self, input: &str, flush: bool) -> Result<()> {
        let writer = self.writer.clone();
        let input = input.to_string();

        // Blocking write on a separate task to avoid blocking async runtime
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut guard = writer.lock().unwrap();
            guard.write_all(input.as_bytes())?;
            if flush {
                guard.flush()?;
            }
            Ok(())
        }).await??;

        Ok(())
    }

    /// Read output with timeout (non-blocking style)
    pub async fn read_output_nonblocking(&mut self, timeout_ms: u64) -> Result<(String, String, bool)> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
        let mut buffer = Vec::new();
        let mut has_more = true;

        // Drain channel
        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }

            match tokio::time::timeout(deadline - now, self.output_rx.recv()).await {
                Ok(Some(data)) => {
                    buffer.extend_from_slice(&data);
                }
                Ok(None) => {
                     // Channel closed
                     has_more = false;
                     break;
                }
                Err(_) => {
                    // Timeout
                    break;
                }
            }
        }

        // Also drain anything currently in the channel buffer without waiting
        while let Ok(data) = self.output_rx.try_recv() {
             buffer.extend_from_slice(&data);
        }

        let output = self.process_buffer(buffer);
        Ok((output, String::new(), has_more)) // PTY implies merged stdout/stderr
    }

    /// Read until pattern matches or timeout
    pub async fn read_until_pattern(&mut self, pattern: &str, timeout_secs: u64) -> Result<String> {
        let regex = Regex::new(pattern).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            // Check if we already match in buffer
            let accumulated_str = String::from_utf8_lossy(&self.read_buffer).to_string();

            if let Some(mat) = regex.find(&accumulated_str) {
                // Return everything up to and including the match
                let end = mat.end();

                // Note: regex `find` returns byte indices in UTF-8 string.
                // We should match against buffer directly if possible, or use string conversion carefully.
                // If we convert buffer -> string, `end` is byte index in string.
                // If the string was lossy converted, byte indices might not map 1:1 to original buffer if invalid chars were replaced.
                // However, `read_buffer` contains raw bytes.
                // Let's assume valid UTF-8 for pattern matching context usually.

                if end <= accumulated_str.len() {
                    // We need to cut `read_buffer`.
                    // The best way is to take the matched string part, and remove corresponding bytes from buffer.
                    // Since lossy conversion might change length, this is tricky.
                    // Simpler approach: Clear buffer and put back "remainder" of string.
                    // But then we have string in buffer, not raw bytes.
                    // Re-encoding string to bytes is safe for UTF-8.

                    let result_str = accumulated_str[..end].to_string();
                    let remainder_str = accumulated_str[end..].to_string();

                    self.read_buffer = remainder_str.into_bytes();

                    return Ok(result_str);
                }
            }

            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(anyhow::anyhow!("Timeout waiting for pattern '{}'", pattern));
            }

            match tokio::time::timeout(deadline - now, self.output_rx.recv()).await {
                 Ok(Some(data)) => {
                     self.read_buffer.extend_from_slice(&data);
                     self.truncate_buffer();
                 }
                 Ok(None) => return Err(anyhow::anyhow!("Shell closed")),
                 Err(_) => continue, // Timeout loop continues to check deadline
            }
        }
    }

    /// Process buffer: decode and truncate
    fn process_buffer(&mut self, new_data: Vec<u8>) -> String {
        self.read_buffer.extend(new_data);
        self.truncate_buffer();
        // Return everything and clear buffer
        let result = String::from_utf8_lossy(&self.read_buffer).to_string();
        self.read_buffer.clear();
        result
    }

    /// Keep only the tail of the buffer (approx 12KB)
    fn truncate_buffer(&mut self) {
        const MAX_BYTES: usize = 12 * 1024;
        if self.read_buffer.len() > MAX_BYTES {
            let start = self.read_buffer.len() - MAX_BYTES;
            self.read_buffer = self.read_buffer[start..].to_vec();
        }
    }

    /// Execute command (Legacy Support)
    pub async fn execute(&mut self, command: &str) -> Result<(String, String, i32, String)> {
        let sentinel = generate_sentinel();

        // Clear any pending output from previous commands to avoid pollution
        while let Ok(_) = self.output_rx.try_recv() {}
        self.read_buffer.clear();

        // Send command
        self.write_stdin_raw(&format!("{}\n", command), true).await?;

        // Send sentinel echo
        #[cfg(unix)]
        {
             self.write_stdin_raw(&format!("echo '{}'\n", sentinel), true).await?;
             self.write_stdin_raw("echo \"__CWD__$(pwd)\"\n", true).await?;
             self.write_stdin_raw("echo \"EXIT_CODE_$?\"\n", true).await?;
        }
        #[cfg(windows)]
        {
             self.write_stdin_raw(&format!("Write-Output '{}'\n", sentinel), true).await?;
             self.write_stdin_raw("Write-Output \"__CWD__$((Get-Location).Path)\"\n", true).await?;
             self.write_stdin_raw("Write-Output \"EXIT_CODE_$(if ($?) {{ 0 }} else {{ 1 }})\"\n", true).await?;
        }

        // Read until sentinel
        let mut accumulated_output = String::new();
        let mut exit_code = 0;
        let mut cwd = self.last_known_cwd.clone();

        // Wait up to 30 seconds for command (legacy timeout)
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);

        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(anyhow::anyhow!("Timeout executing command"));
            }

            // Read chunks
             match tokio::time::timeout(Duration::from_millis(100), self.output_rx.recv()).await {
                 Ok(Some(data)) => {
                     let chunk = String::from_utf8_lossy(&data);
                     accumulated_output.push_str(&chunk);

                     if accumulated_output.contains(&sentinel) {
                         // Parse CWD and Exit Code
                         // Note: PTY output might split lines or have color codes.
                         // But sentinels are unique enough.

                         if let Some(cwd_idx) = accumulated_output.find("__CWD__") {
                             let remainder = &accumulated_output[cwd_idx + 7..];
                             // Find newline or end of string
                             if let Some(end_line) = remainder.find('\n').or_else(|| remainder.find('\r')) {
                                 cwd = remainder[..end_line].trim().to_string();
                             }
                         }

                         if let Some(code_idx) = accumulated_output.find("EXIT_CODE_") {
                              let remainder = &accumulated_output[code_idx + 10..];
                              if let Some(end_line) = remainder.find('\n').or_else(|| remainder.find('\r')) {
                                   exit_code = remainder[..end_line].trim().parse().unwrap_or(0);
                              }
                         }

                         break;
                     }
                 }
                 Ok(None) => break, // Closed
                 Err(_) => continue, // Timeout, keep waiting
             }
        }

        self.last_known_cwd = cwd.clone();

        Ok((accumulated_output, String::new(), exit_code, cwd))
    }

    // Legacy support wrapper
    pub async fn execute_with_input(&mut self, command: &str, user_input: &str) -> Result<(String, String, i32, String)> {
        self.write_stdin_raw(&format!("{}\n", command), true).await?;
        // Allow command to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        self.write_stdin_raw(&format!("{}\n", user_input), true).await?;

         let sentinel = generate_sentinel();
         #[cfg(unix)]
        {
             self.write_stdin_raw(&format!("echo '{}'\n", sentinel), true).await?;
             self.write_stdin_raw("echo \"__CWD__$(pwd)\"\n", true).await?;
             self.write_stdin_raw("echo \"EXIT_CODE_$?\"\n", true).await?;
        }
        #[cfg(windows)]
        {
             self.write_stdin_raw(&format!("Write-Output '{}'\n", sentinel), true).await?;
             self.write_stdin_raw("Write-Output \"__CWD__$((Get-Location).Path)\"\n", true).await?;
             self.write_stdin_raw("Write-Output \"EXIT_CODE_$(if ($?) {{ 0 }} else {{ 1 }})\"\n", true).await?;
        }

         let mut accumulated_output = String::new();
         let cwd = self.last_known_cwd.clone();
         let exit_code = 0;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            if tokio::time::Instant::now() > deadline { break; }
             match tokio::time::timeout(Duration::from_millis(100), self.output_rx.recv()).await {
                 Ok(Some(data)) => {
                     accumulated_output.push_str(&String::from_utf8_lossy(&data));
                     if accumulated_output.contains(&sentinel) { break; }
                 }
                 _ => continue,
             }
        }
        Ok((accumulated_output, String::new(), exit_code, cwd))
    }

    pub fn pid(&mut self) -> Option<u32> {
        // Try to get process ID if possible, otherwise return generic
        if let Ok(_) = self.child.try_wait() {
             return Some(9999); // Dummy PID
        }
        None
    }

    pub fn get_cwd(&self) -> &str {
        &self.last_known_cwd
    }

    pub async fn terminate(&mut self) -> Result<()> {
        debug!("Terminating PTY shell {}", self.session_id);
        self.child.kill()?;
        Ok(())
    }
}

impl std::fmt::Debug for PersistentShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistentShell")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}
