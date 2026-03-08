use super::super::*;
use crate::session_isolation::types::ShellType;
use anyhow::Result;

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
        PersistentShell::new("test-basic".to_string(), temp_dir.clone(), ShellType::Bash).await?;
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
        PersistentShell::new("test-safety".to_string(), temp_dir.clone(), ShellType::Bash).await?;
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

        let (stdout, _, exit_code, _) = shell.execute_with_input(command, dangerous_input).await?;

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
    let (stdout, _, exit_code, _cwd) = shell.execute(&format!("echo '{}'", unicode_str)).await?;
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
