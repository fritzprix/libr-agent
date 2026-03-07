    use super::super::*;
use anyhow::Result;

    #[tokio::test]
    async fn test_shell_creation_and_reuse() -> Result<()> {
        let manager = PersistentShellManager::new();
        let session_id = "test-session".to_string();
        let workspace_path = std::env::temp_dir().join("test_shell_reuse");
        std::fs::create_dir_all(&workspace_path)?;

        // First call should create new shell
        let shell1 = manager
            .get_or_create_shell(session_id.clone(), workspace_path.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let pid1 = shell1.lock().await.pid();

        // Second call should reuse same shell
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
    async fn test_execute_basic_command() -> Result<()> {
        let manager = PersistentShellManager::new();
        let session_id = "test-exec".to_string();
        let workspace_path = std::env::temp_dir().join("test_execute_basic");
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
    async fn test_state_persistence_across_commands() -> Result<()> {
        let manager = PersistentShellManager::new();
        let session_id = "test-state".to_string();
        let workspace_path = std::env::temp_dir().join("test_state_persistence");
        std::fs::create_dir_all(&workspace_path)?;

        #[cfg(unix)]
        {
            // Set environment variable
            manager
                .execute(
                    session_id.clone(),
                    workspace_path.clone(),
                    "export TEST_VAR=TestValue",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            // Verify it persists
            let (stdout, _, exit_code, _cwd) = manager
                .execute(session_id.clone(), workspace_path.clone(), "echo $TEST_VAR")
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue"));
        }

        #[cfg(windows)]
        {
            // Set environment variable
            manager
                .execute(
                    session_id.clone(),
                    workspace_path.clone(),
                    "$env:TEST_VAR='TestValue'",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            // Verify it persists
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
        let ws1 = std::env::temp_dir().join("test_cleanup_1");
        let ws2 = std::env::temp_dir().join("test_cleanup_2");
        let ws3 = std::env::temp_dir().join("test_cleanup_3");
        std::fs::create_dir_all(&ws1)?;
        std::fs::create_dir_all(&ws2)?;
        std::fs::create_dir_all(&ws3)?;

        // Create multiple shells
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

        // Cleanup all
        manager
            .cleanup_all()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Verify all shells are removed
        let shells = manager.shells.lock().await;
        assert_eq!(shells.len(), 0, "All shells should be cleaned up");

        let _ = std::fs::remove_dir_all(&ws1);
        let _ = std::fs::remove_dir_all(&ws2);
        let _ = std::fs::remove_dir_all(&ws3);
        Ok(())
    }
