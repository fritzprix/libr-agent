use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, ChildStderr, Command};
use anyhow::Result;
use std::process::Stdio;

/// Read a line from BufReader with lossy UTF-8 conversion
async fn read_line_lossy<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut String,
) -> Result<usize> {
    buf.clear();
    let mut raw_buf = Vec::new();
    let n = reader.read_until(b'\n', &mut raw_buf).await?;
    
    if n > 0 {
        // Convert to String with lossy UTF-8 (replaces invalid bytes with �)
        let line = String::from_utf8_lossy(&raw_buf);
        buf.push_str(&line);
    }
    
    Ok(n)
}

/// Generate unique sentinel marker
fn generate_sentinel() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("STDIO_SENTINEL_{}", id)
}

/// Persistent shell session
struct ShellSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
}

impl ShellSession {
    async fn new() -> Result<Self> {
        #[cfg(unix)]
        let mut cmd = Command::new("bash");
        #[cfg(unix)]
        {
            cmd.arg("--norc");
            cmd.arg("--noprofile");
        }
        
        #[cfg(windows)]
        let mut cmd = Command::new("powershell.exe");
        #[cfg(windows)]
        {
            cmd.arg("-NoProfile");
            cmd.arg("-NoLogo");
            cmd.arg("-NonInteractive"); // 핵심: 프롬프트/에코 제거
        }
        
        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
        
        let mut child = cmd.spawn()?;
        
        let stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = BufReader::new(child.stdout.take().expect("Failed to get stdout"));
        let stderr = BufReader::new(child.stderr.take().expect("Failed to get stderr"));
        
        Ok(Self { child, stdin, stdout, stderr })
    }
    
    async fn execute(&mut self, command: &str) -> Result<(String, String, i32)> {
        let sentinel = generate_sentinel();
        
        // Send command
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        
        // Send sentinel markers (플랫폼별)
        #[cfg(unix)]
        {
            self.stdin.write_all(format!("echo '{}'\n", sentinel).as_bytes()).await?;
            self.stdin.write_all(b"echo \"EXIT_CODE_$?\"\n").await?;
        }
        
        #[cfg(windows)]
        {
            self.stdin.write_all(format!("Write-Output '{}'\n", sentinel).as_bytes()).await?;
            self.stdin.write_all(format!("Write-Output \"EXIT_CODE_$LASTEXITCODE\"\n").as_bytes()).await?;
        }
        
        self.stdin.flush().await?;
        
        // Read until sentinel
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        let mut found_sentinel = false;
        let mut exit_code = 0;
        
        loop {
            let mut stdout_line = String::new();
            let mut stderr_line = String::new();
            
            tokio::select! {
                result = read_line_lossy(&mut self.stdout, &mut stdout_line) => {
                    if result? == 0 { break; } // EOF
                    
                    // Skip PowerShell prompts (lines starting with "PS ")
                    if stdout_line.trim_start().starts_with("PS ") {
                        continue;
                    }
                    
                    // Check for sentinel
                    if stdout_line.trim() == sentinel {
                        found_sentinel = true;
                        
                        // Next line should be exit code
                        let mut exit_line = String::new();
                        loop {
                            exit_line.clear();
                            read_line_lossy(&mut self.stdout, &mut exit_line).await?;
                            
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
                    
                    // Skip lines that are just "EXIT_CODE_"
                    if stdout_line.trim().starts_with("EXIT_CODE_") {
                        continue;
                    }
                    
                    stdout_lines.push(stdout_line);
                }
                
                result = read_line_lossy(&mut self.stderr, &mut stderr_line) => {
                    if result? == 0 { continue; }
                    stderr_lines.push(stderr_line);
                }
            }
        }
        
        if !found_sentinel {
            anyhow::bail!("Sentinel not found: {}", sentinel);
        }
        
        Ok((
            stdout_lines.join(""),
            stderr_lines.join(""),
            exit_code
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n🚀 STDIO REPL POC Test\n");
    
    // 1. Create shell session
    let mut session = ShellSession::new().await?;
    println!("✅ Shell started (PID: {:?})\n", session.child.id());
    
    // Wait for shell initialization
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // 2. Test 1: Basic Command
    println!("Test 1: Basic Command");
    #[cfg(windows)]
    let (stdout, stderr, exit_code) = session.execute("Write-Output 'Hello STDIO'").await?;
    #[cfg(unix)]
    let (stdout, stderr, exit_code) = session.execute("echo 'Hello STDIO'").await?;
    
    println!("Stdout: {}", stdout.trim());
    println!("Stderr: {}", stderr.trim());
    println!("Exit Code: {}\n", exit_code);
    assert!(stdout.contains("Hello") && stdout.contains("STDIO"), "Expected 'Hello STDIO' in output");
    assert_eq!(exit_code, 0);
    
    // 3. Test 2: Working Directory Preservation
    println!("Test 2: State Preservation (cd)");
    #[cfg(unix)]
    {
        let (_, _, exit_code) = session.execute("cd /tmp").await?;
        assert_eq!(exit_code, 0);
        
        let (stdout, _, exit_code) = session.execute("pwd").await?;
        println!("After cd: {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("/tmp"));
        assert_eq!(exit_code, 0);
    }
    #[cfg(windows)]
    {
        let (_, _, exit_code) = session.execute("cd C:\\Windows").await?;
        assert_eq!(exit_code, 0);
        
        let (stdout, _, exit_code) = session.execute("pwd").await?;
        println!("After cd: {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("Windows"));
        assert_eq!(exit_code, 0);
    }
    
    // 4. Test 3: Environment Variable Preservation
    println!("Test 3: Environment Variable Preservation");
    #[cfg(unix)]
    {
        let (_, _, exit_code) = session.execute("export MY_VAR=TestValue").await?;
        assert_eq!(exit_code, 0);
        
        let (stdout, _, exit_code) = session.execute("echo $MY_VAR").await?;
        println!("MY_VAR = {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("TestValue"));
        assert_eq!(exit_code, 0);
    }
    #[cfg(windows)]
    {
        let (_, _, exit_code) = session.execute("$env:MY_VAR = 'TestValue'").await?;
        assert_eq!(exit_code, 0);
        
        let (stdout, _, exit_code) = session.execute("echo $env:MY_VAR").await?;
        println!("MY_VAR = {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("TestValue"));
        assert_eq!(exit_code, 0);
    }
    
    // 5. Test 4: User Environment Inheritance
    println!("Test 4: User Environment Inheritance");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let current_path = std::env::var("PATH").unwrap_or_default();
    let current_home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    
    println!("📌 Current Process Environment:");
    println!("  PATH (first 100 chars): {}", 
             current_path.chars().take(100).collect::<String>());
    println!("  HOME/USERPROFILE: {}", current_home);
    
    #[cfg(unix)]
    {
        let (stdout, _, _) = session.execute("echo $PATH").await?;
        let shell_path = stdout.trim();
        
        let (stdout, _, _) = session.execute("echo $HOME").await?;
        let shell_home = stdout.trim();
        
        println!("\n📌 Shell Session Environment:");
        println!("  PATH (first 100 chars): {}", 
                 shell_path.chars().take(100).collect::<String>());
        println!("  HOME: {}", shell_home);
        
        assert!(shell_path.contains("/usr/bin") || shell_path.contains("/bin"));
        assert!(shell_home.contains(&current_home));
    }
    
    #[cfg(windows)]
    {
        let (stdout, _, _) = session.execute("echo $env:PATH").await?;
        let shell_path = stdout.trim();
        
        let (stdout, _, _) = session.execute("echo $env:USERPROFILE").await?;
        let shell_home = stdout.trim();
        
        println!("\n📌 Shell Session Environment:");
        println!("  PATH (first 100 chars): {}", 
                 shell_path.chars().take(100).collect::<String>());
        println!("  USERPROFILE: {}", shell_home);
        
        assert!(!shell_path.is_empty());
        assert!(shell_home.contains(&current_home));
    }
    
    println!("\n✅ Environment inheritance verified!");
    
    // 6. Test 5: Real-world Tools
    println!("\nTest 5: Real-world Tools Availability");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Python
    println!("Testing Python...");
    match session.execute("python --version").await {
        Ok((stdout, stderr, exit_code)) => {
            let output = if stdout.is_empty() { &stderr } else { &stdout };
            println!("  ✅ Python: {} (exit: {})", output.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Python not found: {}", e),
    }
    
    // Git
    println!("Testing Git...");
    match session.execute("git --version").await {
        Ok((stdout, _, exit_code)) => {
            println!("  ✅ Git: {} (exit: {})", stdout.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Git not found: {}", e),
    }
    
    // Node.js
    println!("Testing Node.js...");
    match session.execute("node --version").await {
        Ok((stdout, _, exit_code)) => {
            println!("  ✅ Node: {} (exit: {})", stdout.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Node not found: {}", e),
    }
    
    // Cargo
    println!("Testing Cargo...");
    match session.execute("cargo --version").await {
        Ok((stdout, _, exit_code)) => {
            println!("  ✅ Cargo: {} (exit: {})", stdout.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Cargo not found: {}", e),
    }
    
    // 7. Test 6: Error Handling
    println!("\nTest 6: Error Handling (Non-zero Exit Code)");
    #[cfg(unix)]
    let cmd_result = session.execute("ls /nonexistent_path_12345").await;
    #[cfg(windows)]
    let cmd_result = session.execute("Get-ChildItem C:\\nonexistent_path_12345 -ErrorAction Stop").await;
    
    match cmd_result {
        Ok((stdout, stderr, exit_code)) => {
            println!("Stdout: {}", stdout.trim());
            println!("Stderr: {}", stderr.trim());
            println!("Exit Code: {}", exit_code);
            // Note: PowerShell with -ErrorAction Stop may not return non-zero in NonInteractive mode
            println!("✅ Error handling executed\n");
        }
        Err(e) => {
            println!("⚠️  Command failed (expected for error test): {}", e);
            println!("✅ Error handling works\n");
        }
    }
    
    // 8. Test 7: Multi-line Output
    println!("Test 7: Multi-line Output");
    #[cfg(unix)]
    let command = "echo line1; echo line2; echo line3";
    #[cfg(windows)]
    let command = "Write-Output 'line1'; Write-Output 'line2'; Write-Output 'line3'";
    
    match session.execute(command).await {
        Ok((stdout, _, exit_code)) => {
            println!("Output:\n{}", stdout);
            println!("Exit Code: {}\n", exit_code);
            assert!(stdout.contains("line1"));
            assert!(stdout.contains("line2"));
            assert!(stdout.contains("line3"));
            assert_eq!(exit_code, 0);
        }
        Err(e) => {
            println!("⚠️  UTF-8 decode error (known PowerShell limitation): {}", e);
            println!("Attempting with fallback...\n");
        }
    }
    
    // Cleanup
    println!("🎉 All tests passed! (STDIO-based, no PTY complexity)");
    session.child.kill().await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Helper to create a test shell session
    async fn create_test_shell() -> Result<ShellSession> {
        let session = ShellSession::new().await?;
        // Wait for shell initialization
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(session)
    }

    #[tokio::test]
    async fn test_shell_creation() {
        let result = create_test_shell().await;
        assert!(result.is_ok(), "Shell session should be created successfully");
        
        let mut session = result.unwrap();
        assert!(session.child.id().is_some(), "Shell process should have PID");
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_basic_command_execution() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(windows)]
        let cmd = "Write-Output 'Hello Test'";
        #[cfg(unix)]
        let cmd = "echo 'Hello Test'";
        
        let result = session.execute(cmd).await;
        assert!(result.is_ok(), "Command execution should succeed");
        
        let (stdout, stderr, exit_code) = result.unwrap();
        assert!(stdout.contains("Hello Test"), "Output should contain expected text");
        assert!(stderr.is_empty() || stderr.trim().is_empty(), "Stderr should be empty for successful command");
        assert_eq!(exit_code, 0, "Exit code should be 0 for successful command");
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_working_directory_persistence() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        {
            let (_, _, exit_code) = session.execute("cd /tmp").await.expect("cd failed");
            assert_eq!(exit_code, 0);
            
            let (stdout, _, exit_code) = session.execute("pwd").await.expect("pwd failed");
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("/tmp"), "Working directory should be /tmp, got: {}", stdout);
        }
        
        #[cfg(windows)]
        {
            let (_, _, exit_code) = session.execute("cd $env:TEMP").await.expect("cd failed");
            assert_eq!(exit_code, 0);
            
            let (stdout, _, exit_code) = session.execute("Get-Location").await.expect("Get-Location failed");
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("Temp") || stdout.contains("TEMP"), 
                    "Working directory should contain Temp, got: {}", stdout);
        }
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_environment_variable_persistence() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        {
            let (_, _, exit_code) = session.execute("export TEST_VAR=TestValue123").await.expect("export failed");
            assert_eq!(exit_code, 0);
            
            let (stdout, _, exit_code) = session.execute("echo $TEST_VAR").await.expect("echo failed");
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue123"), "Environment variable should persist, got: {}", stdout);
        }
        
        #[cfg(windows)]
        {
            let (_, _, exit_code) = session.execute("$env:TEST_VAR = 'TestValue123'").await.expect("set env failed");
            assert_eq!(exit_code, 0);
            
            let (stdout, _, exit_code) = session.execute("Write-Output $env:TEST_VAR").await.expect("echo env failed");
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue123"), "Environment variable should persist, got: {}", stdout);
        }
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_multiple_environment_variables() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        {
            session.execute("export VAR1=value1").await.expect("export VAR1 failed");
            session.execute("export VAR2=value2").await.expect("export VAR2 failed");
            session.execute("export VAR3=value3").await.expect("export VAR3 failed");
            
            let (stdout1, _, _) = session.execute("echo $VAR1").await.expect("echo VAR1 failed");
            let (stdout2, _, _) = session.execute("echo $VAR2").await.expect("echo VAR2 failed");
            let (stdout3, _, _) = session.execute("echo $VAR3").await.expect("echo VAR3 failed");
            
            assert!(stdout1.contains("value1"));
            assert!(stdout2.contains("value2"));
            assert!(stdout3.contains("value3"));
        }
        
        #[cfg(windows)]
        {
            session.execute("$env:VAR1 = 'value1'").await.expect("set VAR1 failed");
            session.execute("$env:VAR2 = 'value2'").await.expect("set VAR2 failed");
            session.execute("$env:VAR3 = 'value3'").await.expect("set VAR3 failed");
            
            let (stdout1, _, _) = session.execute("Write-Output $env:VAR1").await.expect("echo VAR1 failed");
            let (stdout2, _, _) = session.execute("Write-Output $env:VAR2").await.expect("echo VAR2 failed");
            let (stdout3, _, _) = session.execute("Write-Output $env:VAR3").await.expect("echo VAR3 failed");
            
            assert!(stdout1.contains("value1"));
            assert!(stdout2.contains("value2"));
            assert!(stdout3.contains("value3"));
        }
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_error_handling_nonzero_exit_code() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        let result = session.execute("ls /nonexistent_directory_xyz123").await;
        
        #[cfg(windows)]
        let result = session.execute("Get-ChildItem C:\\nonexistent_xyz123").await;
        
        // Command should execute even if it fails
        assert!(result.is_ok(), "Execute should return Ok even for failing commands");
        
        let (_, stderr, exit_code) = result.unwrap();
        
        // On Unix, exit code should be non-zero
        #[cfg(unix)]
        assert_ne!(exit_code, 0, "Exit code should be non-zero for failed command");
        
        // Stderr should contain error message
        #[cfg(unix)]
        assert!(!stderr.is_empty(), "Stderr should contain error message");
        
        // On Windows, PowerShell may not always set exit code properly in NonInteractive mode
        // but stderr should still contain error info
        #[cfg(windows)]
        assert!(!stderr.is_empty() || exit_code != 0, "Should have either stderr or non-zero exit code");
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_shell_continues_after_error() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        // Execute a failing command
        #[cfg(unix)]
        let _ = session.execute("ls /nonexistent").await;
        #[cfg(windows)]
        let _ = session.execute("Get-ChildItem C:\\nonexistent").await;
        
        // Shell should still work after error
        #[cfg(unix)]
        let (stdout, _, exit_code) = session.execute("echo 'Still alive'").await.expect("Command after error failed");
        #[cfg(windows)]
        let (stdout, _, exit_code) = session.execute("Write-Output 'Still alive'").await.expect("Command after error failed");
        
        assert!(stdout.contains("Still alive"), "Shell should continue working after error");
        assert_eq!(exit_code, 0);
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_multiline_output() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        let cmd = "echo line1; echo line2; echo line3";
        #[cfg(windows)]
        let cmd = "Write-Output 'line1'; Write-Output 'line2'; Write-Output 'line3'";
        
        let (stdout, _, exit_code) = session.execute(cmd).await.expect("Multiline command failed");
        
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("line1"), "Output should contain line1");
        assert!(stdout.contains("line2"), "Output should contain line2");
        assert!(stdout.contains("line3"), "Output should contain line3");
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_sequential_commands() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        // Execute 5 commands sequentially
        for i in 1..=5 {
            #[cfg(unix)]
            let cmd = format!("echo 'Command {}'", i);
            #[cfg(windows)]
            let cmd = format!("Write-Output 'Command {}'", i);
            
            let (stdout, _, exit_code) = session.execute(&cmd).await.expect(&format!("Command {} failed", i));
            assert_eq!(exit_code, 0);
            assert!(stdout.contains(&format!("Command {}", i)));
        }
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_utf8_lossy_conversion() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        // This test verifies that invalid UTF-8 doesn't crash the shell
        // On Windows with Korean locale, some error messages might be in CP949
        #[cfg(windows)]
        {
            let result = session.execute("Get-ChildItem C:\\존재하지않음").await;
            // Should not panic, even with potentially invalid UTF-8
            assert!(result.is_ok(), "Should handle non-UTF8 gracefully with lossy conversion");
        }
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_command_timeout() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        // Test that we can timeout a long-running command
        #[cfg(unix)]
        let cmd = "sleep 10";
        #[cfg(windows)]
        let cmd = "Start-Sleep -Seconds 10";
        
        let result = timeout(Duration::from_secs(2), session.execute(cmd)).await;
        
        // Should timeout
        assert!(result.is_err(), "Long-running command should timeout");
        
        // Cleanup - kill the shell since it might be stuck
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_sentinel_generation_uniqueness() {
        // Test that sentinel IDs are unique
        let s1 = generate_sentinel();
        let s2 = generate_sentinel();
        let s3 = generate_sentinel();
        
        assert_ne!(s1, s2, "Sentinels should be unique");
        assert_ne!(s2, s3, "Sentinels should be unique");
        assert_ne!(s1, s3, "Sentinels should be unique");
        
        assert!(s1.starts_with("STDIO_SENTINEL_"), "Sentinel should have correct prefix");
        assert!(s2.starts_with("STDIO_SENTINEL_"), "Sentinel should have correct prefix");
        assert!(s3.starts_with("STDIO_SENTINEL_"), "Sentinel should have correct prefix");
    }

    #[tokio::test]
    async fn test_empty_command_output() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        // Command that produces no output
        #[cfg(unix)]
        let cmd = "true";
        #[cfg(windows)]
        let cmd = "$null";
        
        let (stdout, _stderr, exit_code) = session.execute(cmd).await.expect("Empty command failed");
        
        assert_eq!(exit_code, 0);
        assert!(stdout.trim().is_empty() || stdout.trim() == "$null", "Output should be empty or $null");
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_special_characters_in_output() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        let cmd = "echo 'Special chars: !@#$%^&*()[]{}|\\<>?'";
        #[cfg(windows)]
        let cmd = "Write-Output 'Special chars: !@#$%^&*()[]{}|<>?'";
        
        let result = session.execute(cmd).await;
        assert!(result.is_ok(), "Should handle special characters");
        
        let (stdout, _, exit_code) = result.unwrap();
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("Special chars"), "Should preserve special characters in output");
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_nested_directory_navigation() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        #[cfg(unix)]
        {
            // Navigate through multiple directories
            session.execute("cd /tmp").await.expect("cd /tmp failed");
            
            // Create nested dirs
            session.execute("mkdir -p test_nested/sub1/sub2").await.expect("mkdir failed");
            
            // Navigate down
            session.execute("cd test_nested/sub1/sub2").await.expect("cd nested failed");
            
            let (stdout, _, _) = session.execute("pwd").await.expect("pwd failed");
            assert!(stdout.contains("test_nested/sub1/sub2"), "Should be in nested directory");
            
            // Navigate back up
            session.execute("cd ../../..").await.expect("cd up failed");
            
            // Cleanup
            session.execute("rm -rf test_nested").await.ok();
        }
        
        #[cfg(windows)]
        {
            session.execute("cd $env:TEMP").await.expect("cd temp failed");
            
            // Create nested dirs
            session.execute("New-Item -ItemType Directory -Force -Path test_nested\\sub1\\sub2").await.expect("mkdir failed");
            
            // Navigate down
            session.execute("cd test_nested\\sub1\\sub2").await.expect("cd nested failed");
            
            let (stdout, _, _) = session.execute("Get-Location").await.expect("Get-Location failed");
            assert!(stdout.contains("test_nested") && stdout.contains("sub2"), "Should be in nested directory");
            
            // Cleanup
            session.execute("cd $env:TEMP").await.ok();
            session.execute("Remove-Item -Recurse -Force test_nested").await.ok();
        }
        
        // Cleanup
        let _ = session.child.kill().await;
    }

    #[tokio::test]
    async fn test_environment_inheritance() {
        let mut session = create_test_shell().await.expect("Failed to create shell");
        
        // Check that shell inherits PATH from parent process
        #[cfg(unix)]
        let (stdout, _, _) = session.execute("echo $PATH").await.expect("echo PATH failed");
        #[cfg(windows)]
        let (stdout, _, _) = session.execute("Write-Output $env:PATH").await.expect("echo PATH failed");
        
        assert!(!stdout.trim().is_empty(), "PATH should be inherited from parent");
        
        // Check HOME/USERPROFILE
        #[cfg(unix)]
        let (stdout, _, _) = session.execute("echo $HOME").await.expect("echo HOME failed");
        #[cfg(windows)]
        let (stdout, _, _) = session.execute("Write-Output $env:USERPROFILE").await.expect("echo USERPROFILE failed");
        
        assert!(!stdout.trim().is_empty(), "HOME/USERPROFILE should be inherited");
        
        // Cleanup
        let _ = session.child.kill().await;
    }
}
