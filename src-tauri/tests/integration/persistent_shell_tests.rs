/// Integration tests for PersistentShell and PersistentShellManager.
///
/// Moved from src-tauri/src/mcp/builtin/workspace/persistent_shell/tests/
/// so they run in CI via `cargo test --tests`.
use anyhow::Result;
use tauri_mcp_agent_lib::mcp::builtin::workspace::persistent_shell::{
    PersistentShell, PersistentShellManager,
};
use tauri_mcp_agent_lib::session_isolation::types::ShellType;

// ── PersistentShell tests ────────────────────────────────────────────────────

#[test]
#[cfg(unix)]
fn test_bash_exists_for_persistent_shell() {
    assert!(
        tauri_mcp_agent_lib::utils::platform::command_exists("bash"),
        "bash must be present for persistent shell tests to run"
    );
}

#[tokio::test]
async fn test_basic_command() -> Result<()> {
    let temp_dir = std::env::temp_dir().join("ps_test_basic_command");
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
    let temp_dir = std::env::temp_dir().join("ps_test_working_dir");
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
    let temp_dir = std::env::temp_dir().join("ps_test_env_vars");
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
    let temp_dir = std::env::temp_dir().join("ps_test_input_safety");
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
    let temp_dir = std::env::temp_dir().join("ps_test_stdin_isolation");
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
    let temp_dir = std::env::temp_dir().join("ps_test_no_newline");
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
    let temp_dir = std::env::temp_dir().join("ps_test_unicode");
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

// ── PersistentShellManager tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_shell_creation_and_reuse() -> Result<()> {
    let manager = PersistentShellManager::new();
    let session_id = "test-session".to_string();
    let workspace_path = std::env::temp_dir().join("ps_test_shell_reuse");
    std::fs::create_dir_all(&workspace_path)?;

    let shell1 = manager
        .get_or_create_shell(session_id.clone(), workspace_path.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let pid1 = shell1.lock().await.pid();

    let shell2 = manager
        .get_or_create_shell(session_id.clone(), workspace_path.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let pid2 = shell2.lock().await.pid();

    assert_eq!(pid1, pid2, "Should reuse same shell instance");

    manager
        .terminate_shell(&session_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let _ = std::fs::remove_dir_all(&workspace_path);
    Ok(())
}

#[tokio::test]
async fn test_execute_basic_command_via_manager() -> Result<()> {
    let manager = PersistentShellManager::new();
    let session_id = "test-exec".to_string();
    let workspace_path = std::env::temp_dir().join("ps_test_execute_basic");
    std::fs::create_dir_all(&workspace_path)?;

    #[cfg(unix)]
    let (stdout, _, exit_code, _cwd) = manager
        .execute(
            session_id.clone(),
            workspace_path.clone(),
            "echo 'Hello World'",
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    #[cfg(windows)]
    let (stdout, _, exit_code, _cwd) = manager
        .execute(
            session_id.clone(),
            workspace_path.clone(),
            "Write-Output 'Hello World'",
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Hello World"));

    manager
        .terminate_shell(&session_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let _ = std::fs::remove_dir_all(&workspace_path);
    Ok(())
}

#[tokio::test]
async fn test_state_persistence_across_commands_via_manager() -> Result<()> {
    let manager = PersistentShellManager::new();
    let session_id = "test-state".to_string();
    let workspace_path = std::env::temp_dir().join("ps_test_state_persistence");
    std::fs::create_dir_all(&workspace_path)?;

    #[cfg(unix)]
    {
        manager
            .execute(
                session_id.clone(),
                workspace_path.clone(),
                "export TEST_VAR=TestValue",
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        let (stdout, _, exit_code, _cwd) = manager
            .execute(session_id.clone(), workspace_path.clone(), "echo $TEST_VAR")
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        assert_eq!(exit_code, 0);
        assert!(stdout.contains("TestValue"));
    }

    #[cfg(windows)]
    {
        manager
            .execute(
                session_id.clone(),
                workspace_path.clone(),
                "$env:TEST_VAR='TestValue'",
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        let (stdout, _, exit_code, _cwd) = manager
            .execute(
                session_id.clone(),
                workspace_path.clone(),
                "echo $env:TEST_VAR",
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        assert_eq!(exit_code, 0);
        assert!(stdout.contains("TestValue"));
    }

    manager
        .terminate_shell(&session_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let _ = std::fs::remove_dir_all(&workspace_path);
    Ok(())
}

#[tokio::test]
async fn test_cleanup_all() -> Result<()> {
    let manager = PersistentShellManager::new();
    let ws1 = std::env::temp_dir().join("ps_test_cleanup_1");
    let ws2 = std::env::temp_dir().join("ps_test_cleanup_2");
    let ws3 = std::env::temp_dir().join("ps_test_cleanup_3");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    std::fs::create_dir_all(&ws3)?;

    manager
        .get_or_create_shell("session1".to_string(), ws1.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    manager
        .get_or_create_shell("session2".to_string(), ws2.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    manager
        .get_or_create_shell("session3".to_string(), ws3.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    manager
        .cleanup_all()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    assert_eq!(
        manager.shell_count().await,
        0,
        "All shells should be cleaned up"
    );

    let _ = std::fs::remove_dir_all(&ws1);
    let _ = std::fs::remove_dir_all(&ws2);
    let _ = std::fs::remove_dir_all(&ws3);
    Ok(())
}
