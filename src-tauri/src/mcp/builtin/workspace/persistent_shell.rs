/// Persistent Shell Session Manager (PTY-based)
///
/// Provides true interactive shell sessions using pseudo-terminals (PTY).
///
/// Key features:
/// - Cross-platform PTY support (Windows ConPTY, Unix PTY) via `portable-pty`
/// - Merged stdout/stderr stream (standard PTY behavior)
/// - Background reader thread for non-blocking output capture
/// - Backward compatibility for atomic command execution via sentinel logic
use anyhow::Result;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, warn};

/// Shared output buffer for PTY reader
type OutputBuffer = Arc<Mutex<Vec<u8>>>;

/// Generate unique sentinel marker for command completion detection
fn generate_sentinel() -> String {
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("STDIO_SENTINEL_{id}")
}

/// Persistent shell session with PTY support
pub struct PersistentShell {
    /// PTY Master (for writing input)
    /// Wrapped in Mutex for Sync (Send is provided by Box<dyn ... + Send>)
    master: Mutex<Box<dyn MasterPty + Send>>,

    /// Child process handle
    child: Mutex<Box<dyn Child + Send>>,

    /// Writer to the PTY master
    writer: Mutex<Box<dyn Write + Send>>,

    /// Shared output buffer populated by background reader thread
    output_buffer: OutputBuffer,

    session_id: String,
    last_known_cwd: String,
}

impl std::fmt::Debug for PersistentShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistentShell")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl PersistentShell {
    /// Create a new persistent shell session
    pub async fn new(session_id: String, workspace_path: PathBuf) -> Result<Self> {
        let pty_system = native_pty_system();

        // Create a PTY
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        // Prepare Command
        #[cfg(windows)]
        let mut cmd = CommandBuilder::new("powershell.exe");
        #[cfg(windows)]
        {
            // PowerShell specific args
            cmd.arg("-NoLogo");
            cmd.arg("-NoProfile");
        }

        #[cfg(unix)]
        let mut cmd = CommandBuilder::new("bash");
        #[cfg(unix)]
        {
            cmd.arg("--norc");
            cmd.arg("--noprofile");

            // Fix PATH for local binaries
            if let Ok(home) = std::env::var("HOME") {
                let local_bin = format!("{}/.local/bin", home);
                if let Ok(path) = std::env::var("PATH") {
                    if !path.contains(&local_bin) {
                        let new_path = format!("{}:{}", local_bin, path);
                        cmd.env("PATH", new_path);
                    }
                } else {
                    cmd.env("PATH", local_bin);
                }
            }
        }

        // Set working directory
        cmd.cwd(workspace_path.clone());

        debug!("Spawning PTY shell for session: {}", session_id);
        let child = pair.slave.spawn_command(cmd)?;

        // Setup Reader Bridge
        let mut reader = pair.master.try_clone_reader()?;
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = output_buffer.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        let mut buffer = buffer_clone.lock().unwrap();
                        buffer.extend_from_slice(&buf[..n]);
                    }
                    Ok(_) => break, // EOF
                    Err(_) => break, // Error
                }
            }
        });

        // Get writer
        let writer = pair.master.take_writer()?;

        let mut shell = Self {
            master: Mutex::new(pair.master),
            child: Mutex::new(child),
            writer: Mutex::new(writer),
            output_buffer,
            session_id: session_id.clone(),
            last_known_cwd: workspace_path.to_string_lossy().to_string(),
        };

        // Initialize Windows encoding
        #[cfg(windows)]
        {
            let setup_cmd = "[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\r\n";
            shell.write_input(setup_cmd).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            shell.read_output().await;
        }

        Ok(shell)
    }

    /// Write input to the PTY
    pub async fn write_input(&mut self, input: &str) -> Result<()> {
        let input_bytes = input.to_string().into_bytes();

        let mut writer = self.writer.lock().unwrap();
        writer.write_all(&input_bytes)?;
        writer.flush()?;

        Ok(())
    }

    /// Read available output from the buffer (non-blocking)
    /// Handles UTF-8 decoding safely by checking for incomplete sequences.
    pub async fn read_output(&self) -> String {
        let mut buffer = self.output_buffer.lock().unwrap();
        if buffer.is_empty() {
            return String::new();
        }

        // Attempt to convert the full buffer to UTF-8
        match String::from_utf8(buffer.clone()) {
            Ok(s) => {
                buffer.clear();
                s
            },
            Err(e) => {
                // If error is due to incomplete sequence at end, keep the remainder
                let valid_up_to = e.utf8_error().valid_up_to();
                let error_len = e.utf8_error().error_len();

                if error_len.is_none() {
                    // Incomplete sequence at end
                    let valid_bytes = buffer[..valid_up_to].to_vec();
                    // Keep the remaining bytes in buffer
                    *buffer = buffer[valid_up_to..].to_vec();
                    String::from_utf8_lossy(&valid_bytes).to_string()
                } else {
                    // Invalid sequence in middle - consume all and replace
                    let data = buffer.clone();
                    buffer.clear();
                    String::from_utf8_lossy(&data).to_string()
                }
            }
        }
    }

    /// Get current working directory of the shell
    pub fn get_cwd(&self) -> &str {
        &self.last_known_cwd
    }

    /// Get the process ID
    pub fn pid(&self) -> Option<u32> {
        self.child.lock().unwrap().process_id()
    }

    /// Terminate the shell session
    pub async fn terminate(&mut self) -> Result<()> {
        debug!("Terminating persistent shell session: {}", self.session_id);
        self.child.lock().unwrap().kill()?;
        Ok(())
    }

    /// Execute a command (Atomic Mode) - Backward Compatibility
    pub async fn execute(&mut self, command: &str) -> Result<(String, String, i32, String)> {
        let sentinel = generate_sentinel();

        #[cfg(unix)]
        let full_command = format!(
            "{{ {}; }} < /dev/null\necho '{}'\necho \"__CWD__$(pwd)\"\necho \"EXIT_CODE_$?\"\n",
            command, sentinel
        );

        #[cfg(windows)]
        let full_command = format!(
            "{}\r\nWrite-Output '{}'\r\nWrite-Output \"__CWD__$((Get-Location).Path)\"\r\nWrite-Output \"EXIT_CODE_$?\"\r\n",
            command, sentinel
        );

        {
            let mut buffer = self.output_buffer.lock().unwrap();
            buffer.clear();
        }

        self.write_input(&full_command).await?;

        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(300);

        let mut collected_output = String::new();
        let mut exit_code = 0;
        let mut cwd = self.last_known_cwd.clone();
        let mut found_all = false;

        loop {
            if start_time.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for command completion");
            }

            let new_data = self.read_output().await;
            if !new_data.is_empty() {
                collected_output.push_str(&new_data);

                if collected_output.contains("EXIT_CODE_") {
                    if collected_output.contains(&sentinel) {
                        found_all = true;

                        let lines: Vec<&str> = collected_output.lines().collect();
                        let mut clean_lines = Vec::new();
                        let mut parsing_metadata = false;

                        for line in lines {
                            let trimmed = line.trim();

                            if trimmed.contains(&sentinel) && !trimmed.starts_with("echo") && !trimmed.starts_with("Write-Output") {
                                parsing_metadata = true;
                                continue;
                            }

                            // Robust CWD parsing: look for __CWD__ anywhere in line (due to PTY prefixes)
                            if let Some(pos) = trimmed.find("__CWD__") {
                                cwd = trimmed[pos + 7..].to_string();
                                continue;
                            }

                            // Robust Exit Code parsing
                            if let Some(pos) = trimmed.find("EXIT_CODE_") {
                                let code_str = &trimmed[pos + 10..];
                                if code_str == "True" {
                                    exit_code = 0;
                                } else if code_str == "False" {
                                    exit_code = 1;
                                } else {
                                    exit_code = code_str.parse().unwrap_or(0);
                                }
                                continue;
                            }

                            if !parsing_metadata {
                                 if line.contains(&sentinel) || line.contains("__CWD__") || line.contains("EXIT_CODE_") {
                                     continue;
                                 }
                                 clean_lines.push(line);
                            }
                        }

                        collected_output = clean_lines.join("\n");
                        break;
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if !found_all {
             anyhow::bail!("Command output incomplete (missing sentinel or exit code)");
        }

        self.last_known_cwd = cwd.clone();

        Ok((collected_output, String::new(), exit_code, cwd))
    }

    /// Execute command with user input (Two-Tool Pattern) - Backward Compatibility
    pub async fn execute_with_input(
        &mut self,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32, String)> {
        let sentinel = generate_sentinel();
        let input_sentinel = format!("INPUT_SENTINEL_{}", generate_sentinel());

        #[cfg(unix)]
        let full_sequence = format!(
            "{{ {}; }} <<'{input_sentinel}'\n{user_input}\n{input_sentinel}\n\
             echo '{}'\necho \"__CWD__$(pwd)\"\necho \"EXIT_CODE_$?\"\n",
            command, sentinel
        );

        #[cfg(windows)]
        let full_sequence = format!(
            "{}\r\n{}\r\nWrite-Output '{}'\r\nWrite-Output \"__CWD__$((Get-Location).Path)\"\r\nWrite-Output \"EXIT_CODE_$?\"\r\n",
            command, user_input, sentinel
        );

        {
            let mut buffer = self.output_buffer.lock().unwrap();
            buffer.clear();
        }

        self.write_input(&full_sequence).await?;

        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(300);
        let mut collected_output = String::new();
        let mut exit_code = 0;
        let mut cwd = self.last_known_cwd.clone();
        let mut found_all = false;

        loop {
            if start_time.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for command completion");
            }
            let new_data = self.read_output().await;
            if !new_data.is_empty() {
                collected_output.push_str(&new_data);
                if collected_output.contains("EXIT_CODE_") {
                    if collected_output.contains(&sentinel) {
                        found_all = true;
                        if let Some(pos) = collected_output.find(&sentinel) {
                            let tail = &collected_output[pos..];
                            for line in tail.lines() {
                                 // Robust parsing
                                 if let Some(pos) = line.find("__CWD__") {
                                     cwd = line[pos + 7..].trim().to_string();
                                 }
                                 if let Some(pos) = line.find("EXIT_CODE_") {
                                     let val = line[pos + 10..].trim();
                                     if val == "True" { exit_code = 0; }
                                     else if val == "False" { exit_code = 1; }
                                     else { exit_code = val.parse().unwrap_or(0); }
                                 }
                            }
                            collected_output.truncate(pos);
                        }
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if !found_all {
             anyhow::bail!("Command output incomplete");
        }

        self.last_known_cwd = cwd.clone();
        Ok((collected_output, String::new(), exit_code, cwd))
    }
}

impl Drop for PersistentShell {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pty_echo() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let mut shell = PersistentShell::new("test-pty".into(), temp_dir).await?;

        // Wait for prompt
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _initial = shell.read_output().await;

        shell.write_input("echo hello\n").await?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        let output = shell.read_output().await;
        assert!(output.contains("hello"));

        Ok(())
    }

    #[tokio::test]
    async fn test_basic_command() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_basic_command");
        std::fs::create_dir_all(&temp_dir)?;
        let mut shell = PersistentShell::new("test-basic".to_string(), temp_dir.clone()).await?;

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
        let mut shell = PersistentShell::new("test-cd".to_string(), temp_dir.clone()).await?;

        #[cfg(unix)]
        {
            shell.execute("cd /tmp").await?;
            let (stdout, _, exit_code, cwd) = shell.execute("pwd").await?;

            assert_eq!(exit_code, 0);
            assert!(stdout.trim().ends_with("/tmp"), "Stdout '{}' does not end with /tmp", stdout.trim());
            assert_eq!(cwd, "/tmp", "CWD captured from shell metadata is incorrect");
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
        let mut shell = PersistentShell::new("test-env".to_string(), temp_dir.clone()).await?;

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
            // The danger: simple concatenation "echo ...\ntouch ...\n"
            let dangerous_input = "touch injected_file\nexit 1";

            let (stdout, _, exit_code, _) =
                shell.execute_with_input(command, dangerous_input).await?;

            assert_eq!(exit_code, 0);
            assert!(stdout.contains("ignoring input"));

            // Check if file was created
            let file_exists = injected_file.exists();
            if file_exists {
                 // Clean up
                 let _ = std::fs::remove_file(&injected_file);
            }
            assert!(!file_exists, "Injected command was executed!");
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
            // With PTY and < /dev/null redirection, it should finish immediately with empty output.
            let (stdout, _, exit_code, _) =
                tokio::time::timeout(std::time::Duration::from_secs(2), shell.execute("cat"))
                    .await
                    .map_err(|_| anyhow::anyhow!("Timeout"))??;

            assert_eq!(exit_code, 0);
        }

        shell.terminate().await?;
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
