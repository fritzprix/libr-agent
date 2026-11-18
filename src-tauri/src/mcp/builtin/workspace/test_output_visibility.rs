#[cfg(test)]
mod tests {
    use super::super::WorkspaceServer;
    use crate::entity::settings;
    use crate::session::SessionManager;
    use sea_orm::{ConnectionTrait, Database, Schema};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    struct WorkspaceTestHarness {
        _guard: tokio::sync::MutexGuard<'static, ()>,
        server: WorkspaceServer,
    }

    async fn create_server() -> WorkspaceTestHarness {
        let guard = crate::state::lock_test_global_state().await;
        crate::state::reset_state();

        let temp_dir = tempdir().unwrap();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create settings database");
        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(settings::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create settings table");
        let session_stmt = schema.create_table_from_entity(crate::entity::session::Entity);
        db.execute(db.get_database_backend().build(&session_stmt))
            .await
            .expect("Failed to create sessions table");
        crate::lifecycle::repositories::init_repositories(&db).await;
        crate::state::init_session_bus(crate::agent::session_bus::SessionBus::new());
        crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));

        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).unwrap());
        WorkspaceTestHarness {
            _guard: guard,
            server: WorkspaceServer::new("test-session".to_string(), session_manager),
        }
    }

    #[tokio::test]
    async fn test_read_process_output_visibility() {
        let harness = create_server().await;
        let server = &harness.server;

        // Use handle_spawn_process directly
        let exec_args = json!({
            "command": "echo \"Hello World output line 1\nHello World output line 2\"",
        });

        let result = server
            .handle_spawn_process(exec_args, "test-session")
            .await
            .expect("Execution failed");
        let data = result.structured_content.expect("No data returned");
        let process_id = data["process_id"]
            .as_str()
            .expect("data has no process_id")
            .to_string();

        let wait_args = json!({ "processId": process_id, "timeout": 15 });
        let wait_res = server
            .call_tool(
                "waitForProcess",
                wait_args,
                Some("test-session".to_string()),
            )
            .await
            .expect("Wait failed");
        let wait_data = wait_res
            .structured_content
            .as_ref()
            .expect("No structured_content");
        let status = wait_data["status"].as_str().unwrap();
        assert!(
            status == "finished" || status == "failed",
            "Process did not finish: status={}",
            status
        );

        let read_args = json!({
            "processId": process_id,
            "stream": "stdout"
        });
        let read_res = server
            .call_tool(
                "readProcessOutput",
                read_args,
                Some("test-session".to_string()),
            )
            .await
            .expect("Read failed");

        let text_content = &read_res.content.unwrap()[0];
        match text_content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                println!("Result text: {}", text);
                assert!(text.contains("Hello World output line 1"));
                assert!(text.contains("Hello World output line 2"));
            }
            _ => panic!("Expected text content"),
        }
    }

    fn result_text(result: &crate::mcp::types::MCPResult) -> String {
        match &result.content.as_ref().expect("no content")[0] {
            crate::mcp::types::MCPContent::Text { text, .. } => text.clone(),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_immediate_exit_result_reports_exit_code_stderr_and_process_id() {
        let result = super::super::code_execution::shell::async_exec::immediate_exit_result(
            "proc-123",
            "nginx -g 'daemon on;'",
            Some(3),
            &["open() \"/run/nginx.pid\" failed (13: Permission denied)".to_string()],
        );
        let text = result_text(&result);
        assert!(
            text.contains("Process exited immediately with code 3"),
            "{text}"
        );
        assert!(text.contains("proc-123"), "{text}");
        assert!(text.contains("Permission denied"), "{text}");
        assert!(!text.contains("failed to start"), "{text}");

        let data = result.structured_content.expect("No structured_content");
        assert_eq!(data["process_id"], "proc-123");
        assert_eq!(data["exit_code"], 3);
        assert_eq!(
            data["stderr_tail"][0],
            "open() \"/run/nginx.pid\" failed (13: Permission denied)"
        );
    }

    #[test]
    fn test_immediate_exit_result_spawn_error_keeps_failed_to_start_with_cause() {
        let result = super::super::code_execution::shell::async_exec::immediate_exit_result(
            "proc-456",
            "does-not-matter",
            None,
            &[
                "LibrAgent: process error: Failed to spawn process: No such file or directory"
                    .to_string(),
            ],
        );
        let text = result_text(&result);
        assert!(text.contains("Process failed to start"), "{text}");
        assert!(text.contains("No such file or directory"), "{text}");
        let data = result.structured_content.expect("No structured_content");
        assert!(data["exit_code"].is_null());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_spawn_process_fast_nonzero_exit_returns_details() {
        let harness = create_server().await;
        let server = &harness.server;

        let result = server
            .handle_spawn_process(
                json!({ "command": "echo boom-stderr 1>&2; exit 3" }),
                "test-session",
            )
            .await
            .expect("Execution failed");
        let text = result_text(&result);
        assert!(
            text.contains("Process exited immediately with code 3"),
            "{text}"
        );
        assert!(text.contains("boom-stderr"), "{text}");

        let data = result.structured_content.expect("No structured_content");
        assert_eq!(data["exit_code"], 3);
        assert!(data["process_id"].as_str().is_some_and(|id| !id.is_empty()));
    }

    #[tokio::test]
    async fn test_poll_process_tail_visibility() {
        let harness = create_server().await;
        let server = &harness.server;

        #[cfg(windows)]
        let command = "powershell -NoProfile -Command \"Write-Output 'Tail line 1'; Write-Output 'Tail line 2'\"";
        #[cfg(not(windows))]
        let command = "echo \"Tail line 1\\nTail line 2\"";
        let exec_args = json!({
            "command": command,
        });

        let result = server
            .handle_spawn_process(exec_args, "test-session")
            .await
            .expect("Execution failed");
        let process_id = result.structured_content.unwrap()["process_id"]
            .as_str()
            .unwrap()
            .to_string();

        // Wait for process to finish
        let wait_args = json!({ "processId": process_id, "timeout": 15 });
        let wait_res = server
            .call_tool(
                "waitForProcess",
                wait_args,
                Some("test-session".to_string()),
            )
            .await
            .expect("Wait failed");
        let wait_data = wait_res
            .structured_content
            .as_ref()
            .expect("No structured_content");
        let status = wait_data["status"].as_str().unwrap();
        assert!(
            status == "finished" || status == "failed",
            "Process did not finish: status={}",
            status
        );

        let read_args = json!({
            "processId": process_id,
            "stream": "stdout",
            "mode": "tail",
            "lines": 5
        });

        let poll_res = server
            .call_tool(
                "readProcessOutput",
                read_args,
                Some("test-session".to_string()),
            )
            .await
            .expect("Poll failed");

        let text_content = &poll_res.content.unwrap()[0];
        match text_content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                println!("Poll Result text: {}", text);
                assert!(text.contains("Tail line 1"));
                assert!(text.contains("Tail line 2"));
                assert!(text
                    .contains("Internal output files (absolute paths, not workspace-relative):"));
                assert!(text.contains("[STDOUT]"));
            }
            _ => panic!("Expected text content"),
        }
    }

    /// A value stored once reaches a later one-off shell as an env var, unchanged
    /// and unexpanded, and stays masked in the tool result.
    #[tokio::test]
    async fn a_stored_value_reaches_a_one_off_shell_and_is_masked() {
        let harness = create_server().await;
        let server = &harness.server;
        let value = "tok s3cr3t & # + ; \" $(whoami)";
        server
            .shell_manager
            .store_secret("test-session", "API_TOKEN", value.to_string())
            .await
            .expect("store secret");

        #[cfg(unix)]
        let (tool, command) = (
            "runShell",
            r#"printf '%s' "$API_TOKEN" > token.txt; printf '%s' "$API_TOKEN""#,
        );
        #[cfg(windows)]
        let (tool, command) = (
            "runPowerShell",
            "[IO.File]::WriteAllText(\"$PWD\\token.txt\", $env:API_TOKEN); Write-Output $env:API_TOKEN",
        );

        let result = server
            .call_tool(
                tool,
                json!({ "command": command }),
                Some("test-session".to_string()),
            )
            .await
            .expect("run shell");

        let token_file = server.get_workspace_dir("test-session").join("token.txt");
        let written = std::fs::read_to_string(&token_file)
            .unwrap_or_else(|error| panic!("token file: {error}; result: {result:?}"));
        assert_eq!(written, value);
        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(!serialized.contains("s3cr3t"), "{serialized}");
    }

    /// A program that prints to both streams and exits 2 (argparse-style).
    #[cfg(unix)]
    const EXIT_2_COMMAND: &str = "sh -c 'echo out-line; echo err-line 1>&2; exit 2'";
    #[cfg(windows)]
    const EXIT_2_COMMAND: &str = "cmd /c \"echo out-line& echo err-line 1>&2& exit 2\"";

    #[cfg(unix)]
    const SHELL_TOOLS: (&str, &str) = ("runShell", "runInPersistentShell");
    #[cfg(windows)]
    const SHELL_TOOLS: (&str, &str) = ("runPowerShell", "runInPersistentPowerShell");

    async fn run(server: &WorkspaceServer, tool: &str, command: &str) -> String {
        let result = server
            .call_tool(
                tool,
                json!({ "command": command }),
                Some("test-session".to_string()),
            )
            .await
            .expect("run shell tool");
        result_text(&result)
    }

    /// #2706: a non-zero exit must still show what the program printed, on both
    /// streams, and exit 2 must not be described as shell misuse.
    #[tokio::test]
    async fn a_nonzero_exit_still_shows_stdout_and_stderr_in_both_shell_tools() {
        let harness = create_server().await;

        for tool in [SHELL_TOOLS.0, SHELL_TOOLS.1] {
            let text = run(&harness.server, tool, EXIT_2_COMMAND).await;

            assert!(text.contains("out-line"), "{tool} dropped stdout: {text}");
            assert!(text.contains("err-line"), "{tool} dropped stderr: {text}");
            assert!(text.contains("exit code: 2"), "{tool}: {text}");
            assert!(
                !text.contains("Misuse of shell command"),
                "{tool} called a completed program shell misuse: {text}"
            );
        }
    }

    /// #2880: for a command that exits in milliseconds, the stderr pipe can still
    /// hold data when the stdout sentinel arrives. Looping the fast-failing
    /// command exercises that race repeatedly, and the trailing clean call catches
    /// any stderr left unread in the pipe (it would leak into the next command).
    #[tokio::test]
    async fn a_fast_nonzero_exit_never_drops_or_leaks_stderr_in_the_persistent_shell() {
        let harness = create_server().await;

        for round in 0..20 {
            let text = run(&harness.server, SHELL_TOOLS.1, EXIT_2_COMMAND).await;
            assert!(
                text.contains("out-line"),
                "round {round} dropped stdout: {text}"
            );
            assert!(
                text.contains("err-line"),
                "round {round} dropped stderr: {text}"
            );
            assert!(text.contains("exit code: 2"), "round {round}: {text}");
        }

        let clean = run(&harness.server, SHELL_TOOLS.1, "echo clean-after").await;
        assert!(
            clean.contains("clean-after"),
            "clean run lost stdout: {clean}"
        );
        assert!(
            !clean.contains("err-line"),
            "stderr from a previous round leaked into the next command: {clean}"
        );
    }

    /// #2706: an exit code from one command must not leak into the next one. On
    /// PowerShell `$LASTEXITCODE` used to stay at 2, so `Write-Output hello`
    /// looked like a failure with no output.
    #[tokio::test]
    async fn a_failed_command_does_not_make_the_next_persistent_command_fail() {
        let harness = create_server().await;
        let persistent = SHELL_TOOLS.1;

        let failed = run(&harness.server, persistent, EXIT_2_COMMAND).await;
        assert!(failed.contains("exit code: 2"), "{failed}");

        #[cfg(unix)]
        let next = "echo hello-after-failure";
        #[cfg(windows)]
        let next = "Write-Output hello-after-failure";
        let text = run(&harness.server, persistent, next).await;

        assert!(text.contains("hello-after-failure"), "{text}");
        assert!(text.contains("exit code: 0"), "{text}");
    }

    /// #2805: cmdlet-only failure must be non-zero in both shells (capture `$?`
    /// before any frame `Write-Output`).
    #[cfg(windows)]
    #[tokio::test]
    async fn a_cmdlet_only_failure_is_nonzero_in_both_powershell_tools() {
        let harness = create_server().await;
        let command = "Get-Item -LiteralPath '/nonexistent-libr-2805'";

        for tool in [SHELL_TOOLS.0, SHELL_TOOLS.1] {
            let text = run(&harness.server, tool, command).await;
            assert!(
                text.contains("exit code: 1"),
                "{tool} should report exit 1 for a missing path: {text}"
            );
        }
    }

    /// #2805: a stale native `$LASTEXITCODE` of 0 must not mask a later cmdlet failure.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_stale_native_zero_does_not_mask_a_later_cmdlet_failure() {
        let harness = create_server().await;
        let command = "cmd /c exit 0; Get-Item -LiteralPath '/nonexistent-libr-2805-stale'";

        for tool in [SHELL_TOOLS.0, SHELL_TOOLS.1] {
            let text = run(&harness.server, tool, command).await;
            assert!(
                text.contains("exit code: 1"),
                "{tool} should report exit 1 when a cmdlet fails after a native 0: {text}"
            );
        }
    }

    /// #2805: a later successful native in a `;` chain wins (documented rule).
    #[cfg(windows)]
    #[tokio::test]
    async fn a_later_successful_native_overwrites_an_earlier_failure() {
        let harness = create_server().await;
        let command = "cmd /c exit 2; cmd /c exit 0";

        for tool in [SHELL_TOOLS.0, SHELL_TOOLS.1] {
            let text = run(&harness.server, tool, command).await;
            assert!(
                text.contains("exit code: 0"),
                "{tool} should report exit 0 when the last native succeeds: {text}"
            );
        }
    }

    /// #2805 / G1.1: native stderr under `2>&1` must not flip a 0 exit on PS 5.1.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_successful_native_with_stderr_redirect_stays_exit_zero() {
        let harness = create_server().await;
        let command = r#"cmd /c "echo warn 1>&2 & exit 0" 2>&1"#;

        for tool in [SHELL_TOOLS.0, SHELL_TOOLS.1] {
            let text = run(&harness.server, tool, command).await;
            assert!(
                text.contains("exit code: 0"),
                "{tool} should keep exit 0 when a native writes stderr under 2>&1: {text}"
            );
        }
    }

    /// #2805 / G1.2: after `$Error` hits the 256-record cap, a later cmdlet
    /// failure in the persistent shell must still be exit 1.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_cmdlet_failure_stays_nonzero_after_error_cap_in_persistent_shell() {
        let harness = create_server().await;
        let persistent = SHELL_TOOLS.1;

        let fill = "1..300 | ForEach-Object { Get-Item -LiteralPath \"/libr-fill-$_\" -ErrorAction SilentlyContinue }";
        let _ = run(&harness.server, persistent, fill).await;

        let text = run(
            &harness.server,
            persistent,
            "Get-Item -LiteralPath '/nonexistent-libr-2805-cap'",
        )
        .await;
        assert!(
            text.contains("exit code: 1"),
            "persistent shell should still report exit 1 after $Error is capped: {text}"
        );
    }

    /// #2805 / G1.3: isolated exit 130 uses the interrupted result shape.
    #[tokio::test]
    async fn an_isolated_signal_exit_is_reported_as_interrupted() {
        let harness = create_server().await;
        let isolated = SHELL_TOOLS.0;

        #[cfg(unix)]
        let command = "sh -c 'exit 130'";
        #[cfg(windows)]
        let command = "exit 130";

        let text = run(&harness.server, isolated, command).await;
        assert!(
            text.contains("exit code: 130"),
            "isolated interrupt should report exit 130: {text}"
        );
        assert!(
            text.contains("interrupted") || text.contains("SIGINT") || text.contains("Notice:"),
            "isolated interrupt should use the interrupt notice, not a plain failure: {text}"
        );
        assert!(
            !text.contains('✗'),
            "isolated interrupt must not look like a hard failure: {text}"
        );
    }
}
