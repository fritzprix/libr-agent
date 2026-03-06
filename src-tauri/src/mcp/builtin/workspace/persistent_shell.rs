/// Persistent Shell Session Manager
///
/// Provides STDIO-based persistent shell sessions for state preservation
/// (working directory, environment variables) without PTY complexity.
///
/// Key features:
/// - Cross-platform unified logic (bash for Unix, PowerShell/Cmd for Windows)
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

use crate::session_isolation::types::ShellType;
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
    last_known_cwd: String,
}

impl PersistentShell {
    /// Create a new persistent shell session
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for this shell session
    /// * `workspace_path` - Working directory for the shell session
    /// * `shell_type` - Type of shell to spawn (Bash, PowerShell, or Cmd)
    ///
    /// # Platform-specific behavior
    /// - Unix: Spawns `bash --norc --noprofile` (shell_type must be Bash)
    /// - Windows (PowerShell): Spawns `powershell.exe -NoProfile -NoLogo -NonInteractive`
    /// - Windows (Cmd): Spawns `cmd.exe /Q /K` (no echo, keep running)
    pub async fn new(
        session_id: String,
        workspace_path: PathBuf,
        #[cfg_attr(unix, allow(unused_variables))] shell_type: ShellType,
    ) -> Result<Self> {
        #[cfg(unix)]
        let mut cmd = {
            // Verify bash exists using the shared utility
            if !crate::utils::platform::command_exists("bash") {
                return Err(anyhow::anyhow!(
                    "Bash shell not found. Please install bash to use persistent shell features."
                ));
            }
            Command::new("bash")
        };

        #[cfg(windows)]
        let mut cmd = match shell_type {
            ShellType::PowerShell => {
                let mut c = Command::new("powershell.exe");
                c.arg("-NoProfile");
                c.arg("-NoLogo");
                c.arg("-NonInteractive"); // Critical: removes prompts and echo
                debug!("Creating persistent PowerShell session for: {}", session_id);
                c
            }
            ShellType::Cmd => {
                let mut c = Command::new("cmd.exe");
                c.arg("/Q"); // Echo off
                c.arg("/K"); // Keep running (don't exit after first command)
                debug!("Creating persistent Cmd shell for: {}", session_id);
                c
            }
            ShellType::Bash => {
                return Err(anyhow::anyhow!(
                    "Bash shell type is not supported on Windows"
                ));
            }
        };

        // Apply environment isolation to prevent leaking host secrets
        // We do this BEFORE platform-specific environment adjustments (like Unix PATH fix)
        // to ensure whitelisted variables are isolated but specialized ones are preserved.
        cmd.env_clear();
        for (k, v) in crate::mcp::utils::env::get_isolated_env() {
            cmd.env(k, v);
        }

        #[cfg(unix)]
        {
            cmd.arg("--norc");
            cmd.arg("--noprofile");

            // Fix: Add ~/.local/bin to PATH as it's often missing in non-interactive shells
            // This is critical for pip installed binaries
            if let Ok(home) = std::env::var("HOME") {
                let local_bin = PathBuf::from(home).join(".local").join("bin");
                let local_bin_str = local_bin.to_string_lossy();

                if let Some(path_os) = std::env::var_os("PATH") {
                    let path_lossy = path_os.to_string_lossy();
                    if !path_lossy.contains(local_bin_str.as_ref()) {
                        // Prepend to prioritize local binaries using standard path manipulation
                        let mut paths = std::env::split_paths(&path_os).collect::<Vec<_>>();
                        paths.insert(0, local_bin.clone());
                        if let Ok(new_path) = std::env::join_paths(paths) {
                            cmd.env("PATH", new_path);
                        }
                    }
                } else {
                    cmd.env("PATH", &local_bin);
                }
            }

            debug!("Creating persistent bash shell for session: {}", session_id);
        }

        // Set working directory to workspace
        cmd.current_dir(&workspace_path);

        let initial_cwd = workspace_path.to_string_lossy().to_string();
        debug!(
            "Setting persistent shell working directory to: {}",
            initial_cwd
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
            match shell_type {
                ShellType::PowerShell => {
                    // Set encoding to UTF-8 for PowerShell to handle non-ASCII characters correctly
                    // We suppress output with [void] cast to avoid polluting the first command's output
                    let setup_cmd = "[void]([Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8)\n";
                    stdin.write_all(setup_cmd.as_bytes()).await?;
                    stdin.flush().await?;
                    debug!("Configuring PowerShell encoding to UTF-8");
                }
                ShellType::Cmd => {
                    // Set encoding to UTF-8 for cmd.exe to handle non-ASCII characters correctly
                    // chcp 65001 sets the code page to UTF-8
                    let setup_cmd = "chcp 65001 >nul\r\n";
                    stdin.write_all(setup_cmd.as_bytes()).await?;
                    stdin.flush().await?;
                    debug!("Configuring cmd.exe encoding to UTF-8 (chcp 65001)");
                }
                ShellType::Bash => {
                    // Should not reach here on Windows
                }
            }
        }

        debug!(
            "Persistent shell created successfully (PID: {:?})",
            child.id()
        );

        #[allow(unused_mut)]
        let mut shell = Self {
            child,
            stdin,
            stdout,
            stderr,
            session_id,
            last_known_cwd: initial_cwd,
        };

        #[cfg(windows)]
        {
            // Force UTF-8 encoding for console I/O and pipe output
            // This is critical for handling non-ASCII characters in filenames/output
            debug!("Configuring PowerShell encoding to UTF-8");
            let _ = shell.execute("[Console]::InputEncoding = [Console]::OutputEncoding = $OutputEncoding = [System.Text.Encoding]::UTF8").await?;
        }

        Ok(shell)
    }

    /// Get current working directory of the shell
    pub fn get_cwd(&self) -> &str {
        &self.last_known_cwd
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
    ///
    /// Execute a command in the persistent shell
    ///
    /// # Arguments
    ///
    /// * `command` - Shell command to execute
    ///
    /// # Returns
    ///
    /// Tuple of (stdout, stderr, exit_code, cwd)
    ///
    /// # Algorithm
    ///
    /// 1. Send command + newline
    /// 2. Send unique sentinel marker
    /// 3. Send CWD capture command
    /// 4. Send exit code capture command
    /// 5. Read stdout/stderr until sentinel found
    /// 6. Parse exit code and CWD
    /// 7. Return collected output
    pub async fn execute(&mut self, command: &str) -> Result<(String, String, i32, String)> {
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
            // Capture exit code BEFORE echoing sentinel (which would reset $?)
            self.stdin
                .write_all(
                    format!("__code=$?; echo '{sentinel}'; echo \"__CWD__$(pwd)\"; echo \"EXIT_CODE_$__code\"\n")
                        .as_bytes(),
                )
                .await?;
        }

        #[cfg(windows)]
        {
            self.stdin
                .write_all(format!("Write-Output '{}'\n", sentinel).as_bytes())
                .await?;

            // Capture CWD
            self.stdin
                .write_all("Write-Output \"__CWD__$((Get-Location).Path)\"\n".as_bytes())
                .await?;

            // Robust exit code capture for PowerShell (PS 5.1 compatible):
            // If $LASTEXITCODE is non-zero OR $? is false:
            //   If $LASTEXITCODE is 0 (meaning $? was false but LASTEXITCODE wasn't set), return 1.
            //   Else return $LASTEXITCODE.
            // Else return 0.
            // Note: Ternary operator (?:) is not supported in PS 5.1, so we use if/else statements.
            self.stdin
                .write_all("Write-Output \"EXIT_CODE_$(if ($LASTEXITCODE -ne 0 -or -not $?) { if ($LASTEXITCODE -eq 0) { 1 } else { $LASTEXITCODE } } else { 0 })\"\n".as_bytes())
                .await?;
        }

        self.stdin.flush().await?;

        self.read_until_sentinel(&sentinel).await
    }

    async fn read_until_sentinel(
        &mut self,
        sentinel: &str,
    ) -> Result<(String, String, i32, String)> {
        // Read output until sentinel found
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        let mut found_sentinel = false;
        let mut exit_code = 0;
        let mut cwd = String::new();

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

                        // Next lines should be CWD and exit code
                        let mut metadata_line = String::new();
                        let mut captured_code = false;

                        // We need to loop because sometimes there might be empty lines
                        loop {
                            metadata_line.clear();
                            // We reuse stdout_raw_buf here, it should be empty after previous read_line_lossy
                            read_line_lossy(&mut self.stdout, &mut metadata_line, &mut stdout_raw_buf).await?;

                            // Skip prompts
                            if metadata_line.trim_start().starts_with("PS ") {
                                continue;
                            }

                            let clean_line = metadata_line.trim();
                            if clean_line.is_empty() {
                                continue;
                            }

                            if let Some(cwd_str) = clean_line.strip_prefix("__CWD__") {
                                cwd = cwd_str.to_string();
                            } else if let Some(code_str) = clean_line.strip_prefix("EXIT_CODE_") {
                                exit_code = code_str.parse().unwrap_or(0);
                                captured_code = true;
                            }

                            // Break if we have both (or sufficient attempts made and we found at least exit code)
                            if captured_code {
                                break;
                            }
                        }

                        break;
                    }

                    // Skip leaked metadata if they appear in wrong order (defensive)
                    if stdout_line.trim().starts_with("EXIT_CODE_") || stdout_line.trim().starts_with("__CWD__") {
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

        // Update cached CWD
        self.last_known_cwd = cwd.clone();

        debug!(
            "Command completed (exit: {}, stdout: {} bytes, stderr: {} bytes, cwd: {})",
            exit_code,
            stdout.len(),
            stderr.len(),
            cwd
        );

        Ok((stdout, stderr, exit_code, cwd))
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
    /// Tuple of (stdout, stderr, exit_code, cwd)
    ///
    /// # Security
    /// Input is passed via stdin pipe, not visible in process command line
    pub async fn execute_with_input(
        &mut self,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32, String)> {
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
            self.stdin.write_all(b"echo \"__CWD__$(pwd)\"\n").await?;
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

            // Capture CWD
            self.stdin
                .write_all("Write-Output \"__CWD__$((Get-Location).Path)\"\n".as_bytes())
                .await?;

            // Robust exit code capture for PowerShell (PS 5.1 compatible)
            self.stdin
                .write_all("Write-Output \"EXIT_CODE_$(if ($LASTEXITCODE -ne 0 -or -not $?) { if ($LASTEXITCODE -eq 0) { 1 } else { $LASTEXITCODE } } else { 0 })\"\n".as_bytes())
                .await?;
        }

        self.stdin.flush().await?;

        let (stdout, stderr, exit_code, cwd) = self.read_until_sentinel(&sentinel).await?;

        // Update cached CWD
        self.last_known_cwd = cwd.clone();

        Ok((stdout, stderr, exit_code, cwd))
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

    /// Verify that bash is available in the test environment.
    ///
    /// This covers the "bash present" path of the existence check added to
    /// `PersistentShell::new`.  The complementary "bash not found" path
    /// requires a container/environment without bash and is validated in CI
    /// through the platform-specific skipped-test mechanism.
    #[test]
    #[cfg(unix)]
    fn test_bash_exists_for_persistent_shell() {
        assert!(
            crate::utils::platform::command_exists("bash"),
            "bash must be present for persistent shell tests to run"
        );
    }

    #[tokio::test]
    async fn test_basic_command() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_basic_command");
        std::fs::create_dir_all(&temp_dir)?;

        #[cfg(unix)]
        let mut shell =
            PersistentShell::new("test-basic".to_string(), temp_dir.clone(), ShellType::Bash)
                .await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-basic".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

        #[cfg(unix)]
        let (stdout, _, exit_code, _) = shell.execute("echo 'Hello World'").await?;
        #[cfg(windows)]
        let (stdout, _, exit_code, _) = shell.execute("Write-Output 'Hello World'").await?;

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

        #[cfg(unix)]
        let mut shell =
            PersistentShell::new("test-cd".to_string(), temp_dir.clone(), ShellType::Bash).await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-cd".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

        #[cfg(unix)]
        {
            shell.execute("cd /tmp").await?;
            let (stdout, _, exit_code, cwd) = shell.execute("pwd").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("/tmp"));
            assert_eq!(cwd, "/tmp");
        }

        #[cfg(windows)]
        {
            shell.execute("cd C:\\Windows").await?;
            let (stdout, _, exit_code, cwd) = shell.execute("pwd").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("C:\\Windows"));
            assert_eq!(cwd, "C:\\Windows");
        }

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_environment_variable_persistence() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_env_vars");
        std::fs::create_dir_all(&temp_dir)?;

        #[cfg(unix)]
        let mut shell =
            PersistentShell::new("test-env".to_string(), temp_dir.clone(), ShellType::Bash).await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-env".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

        #[cfg(unix)]
        {
            shell.execute("export MY_VAR=TestValue").await?;
            let (stdout, _, exit_code, _) = shell.execute("echo $MY_VAR").await?;
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue"));
        }

        #[cfg(windows)]
        {
            shell.execute("$env:MY_VAR='TestValue'").await?;
            let (stdout, _, exit_code, _) = shell.execute("echo $env:MY_VAR").await?;
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

        #[cfg(unix)]
        let mut shell =
            PersistentShell::new("test-safety".to_string(), temp_dir.clone(), ShellType::Bash)
                .await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-safety".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

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

            let (stdout, _, exit_code, _) =
                shell.execute_with_input(command, dangerous_input).await?;

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

        #[cfg(unix)]
        let mut shell = PersistentShell::new(
            "test-isolation".to_string(),
            temp_dir.clone(),
            ShellType::Bash,
        )
        .await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-isolation".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

        #[cfg(unix)]
        {
            // 'cat' without args reads from stdin.
            // If stdin is not isolated, it might hang or consume subsequent commands.
            // With isolation, it should read EOF immediately and exit.
            let (stdout, _, exit_code, _) =
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

    #[tokio::test]
    async fn test_command_without_newline() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_no_newline");
        std::fs::create_dir_all(&temp_dir)?;

        #[cfg(unix)]
        let mut shell = PersistentShell::new(
            "test-no-newline".to_string(),
            temp_dir.clone(),
            ShellType::Bash,
        )
        .await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-no-newline".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

        #[cfg(unix)]
        let (stdout, _, exit_code, _) = shell.execute("printf 'NoNewline'").await?;
        #[cfg(windows)]
        let (stdout, _, exit_code, _) = shell.execute("Write-Host -NoNewline 'NoNewline'").await?;

        assert_eq!(exit_code, 0);
        #[cfg(unix)]
        assert_eq!(stdout, "NoNewline");
        #[cfg(windows)]
        assert!(
            stdout.contains("NoNewline"),
            "Output should contain 'NoNewline', got: {}",
            stdout
        );

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    #[cfg_attr(windows, ignore)] // Encoding in CI/Test environment on Windows is flaky
    async fn test_unicode_handling() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_unicode");
        std::fs::create_dir_all(&temp_dir)?;

        #[cfg(unix)]
        let mut shell = PersistentShell::new(
            "test-unicode".to_string(),
            temp_dir.clone(),
            ShellType::Bash,
        )
        .await?;
        #[cfg(windows)]
        let mut shell = PersistentShell::new(
            "test-unicode".to_string(),
            temp_dir.clone(),
            ShellType::PowerShell,
        )
        .await?;

        let unicode_str = "안녕하세요 Hello World";

        #[cfg(unix)]
        let (stdout, _, exit_code, _cwd) =
            shell.execute(&format!("echo '{}'", unicode_str)).await?;
        #[cfg(windows)]
        let (stdout, _, exit_code, _cwd) = shell
            .execute(&format!("Write-Output '{}'", unicode_str))
            .await?;

        assert_eq!(exit_code, 0);
        assert!(
            stdout.contains(unicode_str),
            "Output '{}' did not contain '{}'",
            stdout,
            unicode_str
        );

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
