/// Persistent Shell Session Manager
///
/// Provides STDIO-based persistent shell sessions for state preservation
/// (working directory, environment variables) without PTY complexity.
///
/// Key features:
/// - Cross-platform unified logic (bash for Unix, PowerShell for Windows)
/// - Sentinel-based command synchronization (no timing dependencies)
/// - UTF-8 lossy conversion for robust encoding handling
/// - Separate stdout/stderr streams
/// - Exit code capture for error handling
use anyhow::Result;
#[cfg(windows)]
use base64::engine::general_purpose;
#[cfg(windows)]
use base64::Engine;

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tracing::{debug, warn};

/// Read a line from BufReader with lossy UTF-8 conversion
///
/// This handles PowerShell error messages that may contain non-UTF8 characters
/// (e.g., Windows CP949 encoding for Korean error messages).
///
/// # Arguments
/// * `reader` - The async reader
/// * `buf` - The string buffer to store the decoded line
/// * `raw_buf` - The raw byte buffer to store read bytes (must be preserved across calls for cancellation safety)
async fn read_line_lossy<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut String,
    raw_buf: &mut Vec<u8>,
) -> Result<usize> {
    buf.clear();
    // We append to raw_buf. If this future is cancelled, raw_buf preserves the partial read.
    let n = reader.read_until(b'\n', raw_buf).await?;

    if !raw_buf.is_empty() {
        // Convert to String with lossy UTF-8 (replaces invalid bytes with )
        let line = String::from_utf8_lossy(raw_buf);
        buf.push_str(&line);
        raw_buf.clear();
    }

    Ok(n)
}

/// Generate unique sentinel marker for command completion detection
fn generate_sentinel() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("STDIO_SENTINEL_{id}")
}

/// Persistent shell session with state preservation
///
/// Maintains a single shell process with redirected stdio streams,
/// allowing commands to preserve working directory, environment variables,
/// and other shell state across multiple executions.
pub struct PersistentShell {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
    session_id: String,
}

impl PersistentShell {
    /// Create a new persistent shell session
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for this shell session
    /// * `workspace_path` - Working directory for the shell session
    ///
    /// # Platform-specific behavior
    /// - Unix: Spawns `bash --norc --noprofile`
    /// - Windows: Spawns `powershell.exe -NoProfile -NoLogo -NonInteractive`
    pub async fn new(session_id: String, workspace_path: PathBuf) -> Result<Self> {
        #[cfg(unix)]
        let mut cmd = Command::new("bash");
        #[cfg(unix)]
        {
            cmd.arg("--norc");
            cmd.arg("--noprofile");
            debug!("Creating persistent bash shell for session: {}", session_id);
        }

        #[cfg(windows)]
        let mut cmd = Command::new("powershell.exe");
        #[cfg(windows)]
        {
            cmd.arg("-NoProfile");
            cmd.arg("-NoLogo");
            cmd.arg("-NonInteractive"); // Critical: removes prompts and echo
            debug!("Creating persistent PowerShell session for: {}", session_id);
        }

        // Set working directory to workspace
        cmd.current_dir(&workspace_path);
        debug!(
            "Setting persistent shell working directory to: {}",
            workspace_path.display()
        );

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        #[allow(unused_mut)]
        let mut stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = BufReader::new(child.stdout.take().expect("Failed to get stdout"));
        let stderr = BufReader::new(child.stderr.take().expect("Failed to get stderr"));

        #[cfg(windows)]
        {
            // Set encoding to UTF-8 for Windows PowerShell to handle non-ASCII characters correctly
            // We suppress output with [void] cast to avoid polluting the first command's output
            let setup_cmd = "[void]([Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8)\n";
            stdin.write_all(setup_cmd.as_bytes()).await?;
            stdin.flush().await?;
        }

        debug!(
            "Persistent shell created successfully (PID: {:?})",
            child.id()
        );

        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            session_id,
        })
    }

    /// Execute a command in the persistent shell
    ///
    /// # Arguments
    /// * `command` - Shell command to execute
    ///
    /// # Returns
    /// Tuple of (stdout, stderr, exit_code)
    ///
    /// # Algorithm
    /// 1. Send command + newline
    /// 2. Send unique sentinel marker
    /// 3. Send exit code capture command
    /// 4. Read stdout/stderr until sentinel found
    /// 5. Parse exit code from next line
    /// 6. Return collected output
    pub async fn execute(&mut self, command: &str) -> Result<(String, String, i32)> {
        let sentinel = generate_sentinel();

        debug!(
            "Executing command in session {}: {}",
            self.session_id, command
        );

        // Send command
        #[cfg(windows)]
        {
            // Encode command to Base64 to avoid encoding issues in the pipe
            // This ensures that characters like Korean are transmitted correctly
            // regardless of the current console code page.
            let encoded = general_purpose::STANDARD.encode(command);
            // We use Invoke-Expression to execute the decoded string
            let wrapper = format!(
                "Invoke-Expression ([System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{}')))\n",
                encoded
            );
            self.stdin.write_all(wrapper.as_bytes()).await?;
        }

        #[cfg(unix)]
        {
            // Wrap in group with /dev/null redirection to prevent stdin consumption
            // Use { ...; } to preserve side effects like 'cd' or 'export'
            // We use multiple lines to handle comments in command safely
            self.stdin.write_all(b"{\n").await?;
            self.stdin.write_all(command.as_bytes()).await?;
            self.stdin.write_all(b"\n} < /dev/null\n").await?;
        }

        // Send sentinel markers (platform-specific exit code syntax)
        #[cfg(unix)]
        {
            self.stdin
                .write_all(format!("echo '{sentinel}'\n").as_bytes())
                .await?;
            self.stdin.write_all(b"echo \"EXIT_CODE_$?\"\n").await?;
        }

        #[cfg(windows)]
        {
            self.stdin
                .write_all(format!("Write-Output '{}'\n", sentinel).as_bytes())
                .await?;
            self.stdin
                .write_all("Write-Output \"EXIT_CODE_$LASTEXITCODE\"\n".as_bytes())
                .await?;
        }

        self.stdin.flush().await?;

        self.read_until_sentinel(&sentinel).await
    }

    async fn read_until_sentinel(&mut self, sentinel: &str) -> Result<(String, String, i32)> {
        // Read output until sentinel found
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        let mut found_sentinel = false;
        let mut exit_code = 0;

        // Raw buffers for cancellation safety
        let mut stdout_raw_buf = Vec::new();
        let mut stderr_raw_buf = Vec::new();

        loop {
            let mut stdout_line = String::new();
            let mut stderr_line = String::new();

            tokio::select! {
                result = read_line_lossy(&mut self.stdout, &mut stdout_line, &mut stdout_raw_buf) => {
                    let n = result?;
                    if n == 0 && stdout_line.is_empty() { break; } // EOF

                    // Skip PowerShell prompts (lines starting with "PS ")
                    if stdout_line.trim_start().starts_with("PS ") {
                        continue;
                    }

                    // Check for sentinel
                    let trimmed_line = stdout_line.trim_end();
                    if trimmed_line.ends_with(sentinel) {
                        found_sentinel = true;

                        // Extract content before sentinel if any
                        let content_len = trimmed_line.len() - sentinel.len();
                        if content_len > 0 {
                            let content = &trimmed_line[..content_len];
                            stdout_lines.push(content.to_string());
                        }

                        // Next line should be exit code
                        let mut exit_line = String::new();
                        loop {
                            exit_line.clear();
                            // We reuse stdout_raw_buf here, it should be empty after previous read_line_lossy
                            read_line_lossy(&mut self.stdout, &mut exit_line, &mut stdout_raw_buf).await?;

                            // Skip prompts in exit code line too
                            if exit_line.trim_start().starts_with("PS ") {
                                continue;
                            }

                            if let Some(code_str) = exit_line.trim().strip_prefix("EXIT_CODE_") {
                                exit_code = code_str.parse().unwrap_or(0);
                            }
                            break;
                        }

                        break;
                    }

                    // Skip orphaned exit code markers
                    if stdout_line.trim().starts_with("EXIT_CODE_") {
                        continue;
                    }

                    stdout_lines.push(stdout_line);
                }

                result = read_line_lossy(&mut self.stderr, &mut stderr_line, &mut stderr_raw_buf) => {
                    let n = result?;
                    if n == 0 && stderr_line.is_empty() { continue; }
                    stderr_lines.push(stderr_line);
                }
            }
        }

        if !found_sentinel {
            warn!(
                "Sentinel not found for session {}: {}",
                self.session_id, sentinel
            );
            anyhow::bail!("Sentinel not found: {sentinel}");
        }
        let stdout = stdout_lines.join("");
        let stderr = stderr_lines.join("");

        debug!(
            "Command completed (exit: {}, stdout: {} bytes, stderr: {} bytes)",
            exit_code,
            stdout.len(),
            stderr.len()
        );

        Ok((stdout, stderr, exit_code))
    }

    /// Execute a command with user input (Two-Tool Pattern)
    ///
    /// Injects user input via stdin before executing the command.
    /// This is used for interactive commands like sudo that require password input.
    ///
    /// # Arguments
    /// * `command` - Shell command to execute
    /// * `user_input` - Input to inject via stdin
    ///
    /// # Returns
    /// Tuple of (stdout, stderr, exit_code)
    ///
    /// # Security
    /// Input is passed via stdin pipe, not visible in process command line
    pub async fn execute_with_input(
        &mut self,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32)> {
        let sentinel = generate_sentinel();

        debug!(
            "Executing command with input in session {}: {}",
            self.session_id, command
        );

        // Send command with heredoc for input (Unix) or piped input (Windows)
        #[cfg(unix)]
        {
            // Use a unique sentinel for the heredoc to avoid conflicts with input content
            let input_sentinel = format!("INPUT_SENTINEL_{}", generate_sentinel());

            // Wrap command in a block and feed input via heredoc
            // Format: { command; } <<'SENTINEL'
            // input
            // SENTINEL
            //
            // We use single quotes around SENTINEL to prevent variable expansion in input
            let heredoc_cmd =
                format!("{{ {command}; }} <<'{input_sentinel}'\n{user_input}\n{input_sentinel}\n");

            self.stdin.write_all(heredoc_cmd.as_bytes()).await?;

            // Send sentinel markers for exit code capture
            self.stdin
                .write_all(format!("echo '{sentinel}'\n").as_bytes())
                .await?;
            self.stdin.write_all(b"echo \"EXIT_CODE_$?\"\n").await?;
        }

        #[cfg(windows)]
        {
            // Send command first
            self.stdin.write_all(command.as_bytes()).await?;
            self.stdin.write_all(b"\n").await?;

            // Send user input (stdin injection)
            self.stdin.write_all(user_input.as_bytes()).await?;
            self.stdin.write_all(b"\n").await?;

            // Send sentinel markers
            self.stdin
                .write_all(format!("Write-Output '{}'\n", sentinel).as_bytes())
                .await?;
            self.stdin
                .write_all("Write-Output \"EXIT_CODE_$LASTEXITCODE\"\n".as_bytes())
                .await?;
        }

        self.stdin.flush().await?;

        self.read_until_sentinel(&sentinel).await
    }

    /// Get the session ID
    #[allow(dead_code)]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the process ID if available
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Terminate the shell session
    pub async fn terminate(&mut self) -> Result<()> {
        debug!("Terminating persistent shell session: {}", self.session_id);
        self.child.kill().await?;
        Ok(())
    }
}

impl Drop for PersistentShell {
    fn drop(&mut self) {
        debug!("Dropping persistent shell session: {}", self.session_id);
        // Best effort kill - ignore errors in drop
        let _ = self.child.start_kill();
    }
}

impl std::fmt::Debug for PersistentShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistentShell")
            .field("session_id", &self.session_id)
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_command() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_basic_command");
        std::fs::create_dir_all(&temp_dir)?;
        let mut shell = PersistentShell::new("test-basic".to_string(), temp_dir.clone()).await?;

        #[cfg(unix)]
        let (stdout, _, exit_code) = shell.execute("echo 'Hello World'").await?;
        #[cfg(windows)]
        let (stdout, _, exit_code) = shell.execute("Write-Output 'Hello World'").await?;

        assert_eq!(exit_code, 0);
        assert!(stdout.contains("Hello World"));

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_working_directory_persistence() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_working_dir");
        std::fs::create_dir_all(&temp_dir)?;
        let mut shell = PersistentShell::new("test-cd".to_string(), temp_dir.clone()).await?;

        #[cfg(unix)]
        {
            shell.execute("cd /tmp").await?;
            let (stdout, _, exit_code) = shell.execute("pwd").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("/tmp"));
        }

        #[cfg(windows)]
        {
            shell.execute("cd C:\\Windows").await?;
            let (stdout, _, exit_code) = shell.execute("pwd").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("C:\\Windows"));
        }

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_environment_variable_persistence() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_env_vars");
        std::fs::create_dir_all(&temp_dir)?;
        let mut shell = PersistentShell::new("test-env".to_string(), temp_dir.clone()).await?;

        #[cfg(unix)]
        {
            shell.execute("export MY_VAR=TestValue").await?;
            let (stdout, _, exit_code) = shell.execute("echo $MY_VAR").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue"));
        }

        #[cfg(windows)]
        {
            shell.execute("$env:MY_VAR='TestValue'").await?;
            let (stdout, _, exit_code) = shell.execute("echo $env:MY_VAR").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue"));
        }

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
    #[tokio::test]
    async fn test_input_injection_safety() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_input_safety");
        std::fs::create_dir_all(&temp_dir)?;
        let mut shell = PersistentShell::new("test-safety".to_string(), temp_dir.clone()).await?;

        // Test case: Command that ignores input, followed by input that looks like a command
        // If injection is possible, "touch injected_file" might be executed
        let injected_file = temp_dir.join("injected_file");
        if injected_file.exists() {
            std::fs::remove_file(&injected_file)?;
        }

        #[cfg(unix)]
        {
            let command = "echo 'ignoring input'";
            let dangerous_input = "touch injected_file\nexit 1";

            let (stdout, _, exit_code) = shell.execute_with_input(command, dangerous_input).await?;

            assert_eq!(exit_code, 0);
            assert!(stdout.contains("ignoring input"));
            assert!(!injected_file.exists(), "Injected command was executed!");
        }

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_stdin_isolation() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_stdin_isolation");
        std::fs::create_dir_all(&temp_dir)?;
        let mut shell =
            PersistentShell::new("test-isolation".to_string(), temp_dir.clone()).await?;

        #[cfg(unix)]
        {
            // 'cat' without args reads from stdin.
            // If stdin is not isolated, it might hang or consume subsequent commands.
            // With isolation, it should read EOF immediately and exit.
            let (stdout, _, exit_code) =
                tokio::time::timeout(std::time::Duration::from_secs(2), shell.execute("cat"))
                    .await
                    .map_err(|_| anyhow::anyhow!("Timeout"))??;

            assert_eq!(exit_code, 0);
            assert_eq!(stdout, "");
        }

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
