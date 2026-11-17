//! Integration tests for `liblet` headless execution (#2703).
//!
//! Validates all acceptance criteria from #2703 and `docs/specs/liblet-cli-interface.md`:
//! - Headless invocation is `-p / --prompt`, or a prompt piped on `stdin`
//! - No TTY and no prompt is a usage error (`2`); there is no auto-headless
//! - There is no `--hook`
//! - Final result on `stdout` (`text`, `json`, and `stream-json` full trace)
//! - Default mode (`normal`) does not skip approvals; approval stop exits `3`,
//!   user question/secret stop exits `4`, timeout exits `124`, interrupt exits `130`/`143`
//! - Session is ephemeral: inherits profile settings/assistants without touching
//!   or appearing in the shared GUI/TUI session list
//! - Multiple headless runs proceed concurrently without blocking one another

use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tauri_mcp_agent_lib::agent::state::{
    admit_user_prompt, PendingApprovalData, PendingApprovalKind, UserPromptPayload, UserQuestion,
};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::headless::cli::{
    exit_codes, parse_cli, HeadlessArgs, LibletCommand, OutputFormat, TtyState,
};
use tauri_mcp_agent_lib::headless::output::HeadlessStatus;
use tauri_mcp_agent_lib::headless::runner::InterruptSignal;
use tauri_mcp_agent_lib::headless::writeback::write_back_run;
use tauri_mcp_agent_lib::headless::{init_ephemeral_database, EphemeralHeadlessRuntime};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::scheduled_task_repository::{
    CreateScheduledTaskParams, ScheduledTaskRepository, SqliteScheduledTaskRepository,
};
use tauri_mcp_agent_lib::repositories::{
    AssistantRepository, SessionMetadata, SessionRepository, SessionStatus, SettingsRepository,
    SqliteAssistantRepository, SqliteSessionRepository, SqliteSettingsRepository,
};
use tauri_mcp_agent_lib::scheduled::TASK_CATEGORY_GLOBAL;
use tempfile::TempDir;
use tokio::sync::{oneshot, watch};

fn svec(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_string()).collect()
}

fn default_headless_args(prompt: &str, workspace: &std::path::Path) -> HeadlessArgs {
    HeadlessArgs {
        prompt: prompt.to_string(),
        cd: Some(workspace.to_path_buf()),
        assistant: None,
        model: None,
        provider: None,
        isolation: WorkspaceIsolationMode::Host,
        execution_mode: None,
        output_format: OutputFormat::Text,
        timeout_secs: Some(10),
        session: None,
    }
}

fn make_assistant_message(
    session_id: &str,
    id: &str,
    text: &str,
    tool_calls: Option<Vec<ToolCall>>,
) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: if text.is_empty() {
            Vec::new()
        } else {
            vec![MCPContent::Text {
                text: text.to_string(),
            }]
        },
        tool_calls,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: 1000,
        updated_at: 1000,
        source: None,
        error: None,
        metadata: None,
    }
}

fn make_tool_result_message(session_id: &str, id: &str, tool_call_id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls: None,
        tool_call_id: Some(tool_call_id.to_string()),
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: 1001,
        updated_at: 1001,
        source: None,
        error: None,
        metadata: None,
    }
}

#[test]
fn cli_mode_selection_and_validation_matrix() {
    let tty = TtyState {
        stdin_is_tty: true,
        stdout_is_tty: true,
    };
    let non_tty = TtyState {
        stdin_is_tty: false,
        stdout_is_tty: false,
    };

    // 1. -p / --prompt selects headless even on a TTY
    let cmd = parse_cli(&svec(&["-p", "check status"]), tty, None).unwrap();
    assert!(matches!(cmd, LibletCommand::Headless(_)));

    // 2. Piped stdin without -p selects headless
    let cmd = parse_cli(&svec(&[]), non_tty, Some("piped task\n")).unwrap();
    match cmd {
        LibletCommand::Headless(h) => assert_eq!(h.prompt, "piped task"),
        other => panic!("expected Headless, got {other:?}"),
    }

    // 3. No TTY and no prompt is a usage error (exit code 2); no auto-headless
    let err = parse_cli(&svec(&[]), non_tty, None).unwrap_err();
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);

    // 4. Positional prompt when stdin/stdout is not a TTY is a usage error (exit code 2)
    let err = parse_cli(&svec(&["positional prompt"]), non_tty, Some("piped")).unwrap_err();
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);

    // 5. --hook is rejected with usage error (exit code 2)
    let err = parse_cli(&svec(&["--hook", "my-hook", "-p", "hi"]), tty, None).unwrap_err();
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
    let err_eq = parse_cli(&svec(&["--hook=my-hook", "-p", "hi"]), tty, None).unwrap_err();
    assert_eq!(err_eq.exit_code(), exit_codes::USAGE_ERROR);
}

#[tokio::test]
async fn headless_text_and_stream_json_trace_and_ephemeral_isolation() {
    let shared_dir = TempDir::new().unwrap();
    let workspace_dir = TempDir::new().unwrap();
    let shared_db_path = shared_dir.path().join("shared_profile.db");

    // Seed shared profile DB with settings, an assistant, and an existing GUI session.
    let shared_db = init_ephemeral_database(&shared_db_path).await.unwrap();
    let shared_settings = SqliteSettingsRepository::new(shared_db.clone());
    let shared_assistants = SqliteAssistantRepository::new(shared_db.clone());
    let shared_sessions = SqliteSessionRepository::new(shared_db.clone());

    shared_settings
        .set(
            "preferredModel",
            json!({ "provider": "anthropic", "model": "claude-3-7-sonnet" }),
        )
        .await
        .unwrap();

    shared_assistants
        .create_assistant(
            "asst-reviewer".to_string(),
            "Code Reviewer".to_string(),
            json!({
                "name": "Code Reviewer",
                "systemPrompt": "You review diffs concisely."
            })
            .to_string(),
        )
        .await
        .unwrap();

    let gui_session = SessionMetadata {
        id: "gui-session-1".to_string(),
        name: Some("Existing GUI Session".to_string()),
        status: SessionStatus::Idle,
        model: "claude-3-7-sonnet".to_string(),
        provider: "anthropic".to_string(),
        assistant_id: Some("asst-reviewer".to_string()),
        parent_session_id: None,
        lineage_id: Some("gui-session-1".to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 100,
        updated_at: 100,
        last_viewed_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        last_message_at: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    };
    shared_sessions.upsert_session(&gui_session).await.unwrap();

    // Run 1: `--output-format text` inheriting profile settings and assistant by name
    let mut text_args = default_headless_args("review main.rs", workspace_dir.path());
    text_args.assistant = Some("code reviewer".to_string());
    text_args.output_format = OutputFormat::Text;

    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&shared_db_path), &text_args)
        .await
        .expect("bootstrap ephemeral headless runtime");

    assert_eq!(runtime.resolved.model, "claude-3-7-sonnet");
    assert_eq!(runtime.resolved.provider, "anthropic");
    assert_eq!(
        runtime.resolved.assistant_id.as_deref(),
        Some("asst-reviewer")
    );
    assert_eq!(runtime.resolved.execution_mode, ExecutionMode::Normal);

    // Ephemeral DB must NOT contain the shared GUI session
    let ephemeral_sessions = runtime.session_repo.get_all_sessions().await.unwrap();
    assert_eq!(ephemeral_sessions.len(), 1);
    assert_eq!(ephemeral_sessions[0].id, runtime.session_id);

    let ephemeral_dir_path = runtime.env.temp_dir.path().to_path_buf();
    let (_interrupt_tx, interrupt_rx) = watch::channel(None);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let payload = runtime
        .run_with_driver(
            &text_args,
            interrupt_rx,
            |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                    session.messages.write().await.push(make_assistant_message(
                        &session_id,
                        "msg-final",
                        "LGTM: no issues found.",
                        None,
                    ));
                    session.metadata.status = SessionStatus::Idle;
                }
                Ok(())
            },
            &mut stdout,
            &mut stderr,
        )
        .await;

    assert_eq!(payload.exit_code, exit_codes::SUCCESS);
    assert_eq!(payload.status, HeadlessStatus::Completed);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "LGTM: no issues found.\n"
    );
    assert!(stderr.is_empty());
    assert!(
        !ephemeral_dir_path.exists(),
        "ephemeral temp dir must be cleaned up when run finishes"
    );

    // #2860: the finished fresh run wrote back exactly one new session row;
    // the GUI's own session is untouched.
    let remaining_shared = shared_sessions.get_all_sessions().await.unwrap();
    assert_eq!(remaining_shared.len(), 2);
    let ids: Vec<&str> = remaining_shared.iter().map(|s| s.id.as_str()).collect();
    assert!(
        ids.contains(&"gui-session-1"),
        "gui-session-1 survives: {ids:?}"
    );
    assert!(
        ids.contains(&payload.session_id.as_str()),
        "run 1 wrote back its session: {ids:?}"
    );
    assert!(payload.session_id.starts_with("headless-"));
    assert_eq!(payload.forked_from, None);

    // Run 2: `--output-format stream-json` produces full NDJSON trace (`init`, `message`, `tool_use`, `tool_result`, `result`)
    let mut stream_args = default_headless_args("run check", workspace_dir.path());
    stream_args.output_format = OutputFormat::StreamJson;

    let runtime2 = EphemeralHeadlessRuntime::bootstrap(Some(&shared_db_path), &stream_args)
        .await
        .unwrap();
    let (_interrupt_tx2, interrupt_rx2) = watch::channel(None);
    let mut stream_stdout = Vec::new();
    let mut stream_stderr = Vec::new();

    let payload2 = runtime2
        .run_with_driver(
            &stream_args,
            interrupt_rx2,
            |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                    session.messages.write().await.push(make_assistant_message(
                        &session_id,
                        "msg-tool-call",
                        "Checking files...",
                        Some(vec![ToolCall {
                            id: "call_read_1".to_string(),
                            r#type: "function".to_string(),
                            function: ToolCallFunction {
                                name: "workspace__readFile".to_string(),
                                arguments: r#"{"path":"README.md"}"#.to_string(),
                            },
                        }]),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    let mut msgs = session.messages.write().await;
                    msgs.push(make_tool_result_message(
                        &session_id,
                        "msg-tool-res",
                        "call_read_1",
                        "# Project Title",
                    ));
                    msgs.push(make_assistant_message(
                        &session_id,
                        "msg-done",
                        "Read README.md successfully.",
                        None,
                    ));
                    drop(msgs);
                    session.metadata.status = SessionStatus::Idle;
                }
                Ok(())
            },
            &mut stream_stdout,
            &mut stream_stderr,
        )
        .await;

    assert_eq!(payload2.exit_code, exit_codes::SUCCESS);
    let output_str = String::from_utf8(stream_stdout).unwrap();
    let lines: Vec<Value> = output_str
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("valid NDJSON line"))
        .collect();

    let event_types: Vec<&str> = lines.iter().map(|v| v["type"].as_str().unwrap()).collect();
    assert_eq!(event_types.first(), Some(&"init"));
    assert!(event_types.contains(&"message"));
    assert!(event_types.contains(&"tool_use"));
    assert!(event_types.contains(&"tool_result"));
    assert_eq!(event_types.last(), Some(&"result"));

    let last = lines.last().unwrap();
    assert_eq!(last["status"], "completed");
    assert_eq!(last["result"], "Read README.md successfully.");
    assert_eq!(last["exitCode"], 0);
    assert_eq!(last["sessionId"], payload2.session_id.as_str());

    // Run 2 wrote back its own new session too (#2860).
    let after_both = shared_sessions.get_all_sessions().await.unwrap();
    assert_eq!(after_both.len(), 3);
    assert!(after_both.iter().any(|s| s.id == payload2.session_id));
}

#[tokio::test]
async fn headless_default_mode_stops_with_exit_code_3_on_approval() {
    let workspace_dir = TempDir::new().unwrap();
    let mut args = default_headless_args("run dangerous shell command", workspace_dir.path());
    args.output_format = OutputFormat::Json;

    let runtime = EphemeralHeadlessRuntime::bootstrap(None, &args)
        .await
        .unwrap();
    // Default execution mode must be Normal (never skipping approvals by default)
    assert_eq!(runtime.resolved.execution_mode, ExecutionMode::Normal);

    let (_interrupt_tx, interrupt_rx) = watch::channel(None);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let payload = runtime
        .run_with_driver(
            &args,
            interrupt_rx,
            |session_id, _repo, active_sessions, _resolved| async move {
                let (tx, rx) = oneshot::channel();
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                    session.pending_approvals.write().await.insert(
                        "call_shell_1".to_string(),
                        PendingApprovalData {
                            sender: tx,
                            tool_name: "workspace__runShell".to_string(),
                            arguments: r#"{"command":"git clean -fd"}"#.to_string(),
                            approval_kind: PendingApprovalKind::Standard,
                            request_id: None,
                            description: Some("Run shell command".to_string()),
                            input_preview: Some("git clean -fd".to_string()),
                        },
                    );
                }
                let _ = rx.await;
                Ok(())
            },
            &mut stdout,
            &mut stderr,
        )
        .await;

    assert_eq!(payload.exit_code, exit_codes::APPROVAL_REQUIRED);
    assert_eq!(payload.exit_code, 3);
    assert_eq!(payload.status, HeadlessStatus::Blocked);
    assert_eq!(payload.pending_approvals.len(), 1);
    assert_eq!(payload.pending_approvals[0].tool_call_id, "call_shell_1");
    assert_eq!(
        payload.pending_approvals[0].tool_name,
        "workspace__runShell"
    );
    assert_eq!(
        payload.pending_approvals[0].command_preview,
        "git clean -fd"
    );
    assert_eq!(
        payload.pending_approvals[0].approval_kind,
        PendingApprovalKind::Standard
    );

    let parsed: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(parsed["type"], "result");
    assert_eq!(parsed["status"], "blocked");
    assert_eq!(parsed["reason"], "approval");
    assert_eq!(parsed["exitCode"], 3);
    assert_eq!(parsed["pendingApprovals"][0]["toolCallId"], "call_shell_1");
}

#[tokio::test]
async fn headless_stops_with_exit_code_4_on_user_question_or_secret() {
    let workspace_dir = TempDir::new().unwrap();
    let mut args = default_headless_args("deploy to staging", workspace_dir.path());
    args.output_format = OutputFormat::Json;

    let runtime = EphemeralHeadlessRuntime::bootstrap(None, &args)
        .await
        .unwrap();
    let (_interrupt_tx, interrupt_rx) = watch::channel(None);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let payload = runtime
        .run_with_driver(
            &args,
            interrupt_rx,
            |session_id, _repo, active_sessions, _resolved| async move {
                let slot = {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                    session.pending_user_prompt.clone()
                };
                let (_req_id, rx) = admit_user_prompt(
                    &slot,
                    UserPromptPayload::Question {
                        questions: vec![UserQuestion {
                            id: "env".to_string(),
                            question: "Which staging cluster should we target?".to_string(),
                            description: None,
                            options: None,
                            multi_select: false,
                            allow_custom: true,
                        }],
                    },
                )
                .await
                .unwrap();
                let _ = rx.await;
                Ok(())
            },
            &mut stdout,
            &mut stderr,
        )
        .await;

    assert_eq!(payload.exit_code, exit_codes::QUESTION_REQUIRED);
    assert_eq!(payload.exit_code, 4);
    assert_eq!(payload.status, HeadlessStatus::Blocked);

    let parsed: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(parsed["type"], "result");
    assert_eq!(parsed["status"], "blocked");
    assert_eq!(parsed["reason"], "question");
    assert_eq!(parsed["exitCode"], 4);
}

#[tokio::test]
async fn headless_timeout_and_interrupt_exit_codes() {
    let workspace_dir = TempDir::new().unwrap();

    // 1. Timeout -> exit code 124
    let mut timeout_args = default_headless_args("long running task", workspace_dir.path());
    timeout_args.output_format = OutputFormat::Json;
    timeout_args.timeout_secs = Some(1);

    let runtime = EphemeralHeadlessRuntime::bootstrap(None, &timeout_args)
        .await
        .unwrap();
    let (_interrupt_tx, interrupt_rx) = watch::channel(None);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let payload = runtime
        .run_with_driver(
            &timeout_args,
            interrupt_rx,
            |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(())
            },
            &mut stdout,
            &mut stderr,
        )
        .await;

    assert_eq!(payload.exit_code, exit_codes::TIMEOUT);
    assert_eq!(payload.exit_code, 124);
    assert_eq!(payload.status, HeadlessStatus::Timeout);

    // 2. SIGINT -> exit code 130
    let mut int_args = default_headless_args("interruptible task", workspace_dir.path());
    int_args.output_format = OutputFormat::Json;

    let runtime2 = EphemeralHeadlessRuntime::bootstrap(None, &int_args)
        .await
        .unwrap();
    let (interrupt_tx2, interrupt_rx2) = watch::channel(None);
    let mut stdout2 = Vec::new();
    let mut stderr2 = Vec::new();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        let _ = interrupt_tx2.send(Some(InterruptSignal::SigInt));
    });

    let payload2 = runtime2
        .run_with_driver(
            &int_args,
            interrupt_rx2,
            |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(())
            },
            &mut stdout2,
            &mut stderr2,
        )
        .await;

    assert_eq!(payload2.exit_code, exit_codes::INTERRUPTED_SIGINT);
    assert_eq!(payload2.exit_code, 130);
    assert_eq!(payload2.status, HeadlessStatus::Interrupted);
}

#[tokio::test]
async fn multiple_headless_runs_execute_concurrently_without_blocking() {
    let shared_dir = TempDir::new().unwrap();
    let workspace_dir = TempDir::new().unwrap();
    let shared_db_path = shared_dir.path().join("concurrent_profile.db");

    let shared_db = init_ephemeral_database(&shared_db_path).await.unwrap();
    let shared_settings = SqliteSettingsRepository::new(shared_db.clone());
    shared_settings
        .set(
            "preferredModel",
            json!({ "provider": "openai", "model": "gpt-4o-mini" }),
        )
        .await
        .unwrap();

    let mut handles = Vec::new();
    for idx in 0..4 {
        let db_path = shared_db_path.clone();
        let ws = workspace_dir.path().to_path_buf();
        handles.push(tokio::spawn(async move {
            let args = default_headless_args(&format!("task-{idx}"), &ws);
            let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&db_path), &args)
                .await
                .expect("concurrent bootstrap");
            let (_tx, rx) = watch::channel(None);
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let expected = format!("completed-run-{idx}");
            let expected_clone = expected.clone();

            let payload = runtime
                .run_with_driver(
                    &args,
                    rx,
                    move |session_id, _repo, active_sessions, _resolved| async move {
                        {
                            let mut active = active_sessions.write().await;
                            let session = active.get_mut(&session_id).unwrap();
                            session.metadata.status = SessionStatus::Busy;
                        }
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        {
                            let mut active = active_sessions.write().await;
                            let session = active.get_mut(&session_id).unwrap();
                            session.messages.write().await.push(make_assistant_message(
                                &session_id,
                                &format!("msg-{idx}"),
                                &expected_clone,
                                None,
                            ));
                            session.metadata.status = SessionStatus::Idle;
                        }
                        Ok(())
                    },
                    &mut stdout,
                    &mut stderr,
                )
                .await;

            assert_eq!(payload.exit_code, 0);
            assert_eq!(String::from_utf8(stdout).unwrap().trim(), expected.as_str());
        }));
    }

    let mut results = HashMap::new();
    for (idx, h) in handles.into_iter().enumerate() {
        h.await.expect("concurrent headless run should succeed");
        results.insert(idx, true);
    }
    assert_eq!(results.len(), 4);

    // #2860: each of the 4 concurrent runs wrote back exactly one session
    // row through its own short write transaction.
    let shared_sessions = SqliteSessionRepository::new(shared_db);
    let written_back = shared_sessions.get_all_sessions().await.unwrap();
    assert_eq!(written_back.len(), 4);
}

#[tokio::test]
async fn snapshot_scrubs_session_rows_from_raw_pages_not_just_rows() {
    // A `DELETE` leaves the freed pages in the file, so a copied profile
    // session's text would remain recoverable from the snapshot's raw bytes.
    // The snapshot must `VACUUM` after stripping so the sentinel is gone
    // physically, not just logically.
    let shared_dir = TempDir::new().unwrap();
    let workspace_dir = TempDir::new().unwrap();
    let shared_db_path = shared_dir.path().join("sentinel_profile.db");

    let shared_db = init_ephemeral_database(&shared_db_path).await.unwrap();
    let shared_sessions = SqliteSessionRepository::new(shared_db.clone());

    let sentinel = "SENTINEL-SESSION-9f3a7c2e-REDACT-ME";
    let session = SessionMetadata {
        id: "gui-sentinel-1".to_string(),
        name: Some(sentinel.to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some("gui-sentinel-1".to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 100,
        updated_at: 100,
        last_viewed_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        last_message_at: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    };
    shared_sessions.upsert_session(&session).await.unwrap();
    drop(shared_db);

    let args = default_headless_args("unused", workspace_dir.path());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&shared_db_path), &args)
        .await
        .unwrap();
    let snapshot_path = runtime.env.db_path.clone();

    // No row-level access: read the raw snapshot bytes and require the
    // sentinel name to be physically absent.
    let raw = std::fs::read(&snapshot_path).expect("snapshot file must exist");
    let raw_text = String::from_utf8_lossy(&raw);
    assert!(
        !raw_text.contains(sentinel),
        "ephemeral snapshot must physically scrub stripped profile session rows (VACUUM after DELETE)"
    );

    // Sanity: the snapshot itself is still usable and only holds the
    // bootstrap's own ephemeral session.
    let sessions = runtime.session_repo.get_all_sessions().await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, runtime.session_id);

    drop(runtime);
}

/// Insert plain user rows into a migrated database for snapshot tests.
fn insert_session_messages(db_path: &std::path::Path, session_id: &str, ids: &[(&str, i64)]) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    for (id, created) in ids {
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at, updated_at) \
             VALUES (?1, ?2, 'user', '[]', ?3, ?3)",
            rusqlite::params![id, session_id, created],
        )
        .unwrap();
    }
}

fn count_rows(db_path: &std::path::Path, table: &str) -> i64 {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

/// A persisted session with its own workspace, used by the `--session` tests.
async fn seed_continuable_session(db_path: &std::path::Path, workspace: &std::path::Path) {
    let db = init_ephemeral_database(db_path).await.unwrap();
    let repo = SqliteSessionRepository::new(db.clone());
    repo.upsert_session(&SessionMetadata {
        id: "s-cont".to_string(),
        name: Some("Titled".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some("s-cont".to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 100,
        updated_at: 100,
        last_viewed_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        last_message_at: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: Some(workspace.to_string_lossy().to_string()),
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    })
    .await
    .unwrap();
    repo.upsert_session(&SessionMetadata {
        id: "s-other".to_string(),
        created_at: 50,
        updated_at: 50,
        ..session_template()
    })
    .await
    .unwrap();
    drop(db);

    insert_session_messages(db_path, "s-cont", &[("m-1", 500), ("m-2", 600)]);
    insert_session_messages(db_path, "s-other", &[("m-other", 700)]);
    seed_per_session_state(db_path, "s-cont", "m-1", 1);
    seed_per_session_state(db_path, "s-other", "m-other", 2);
}

/// One row per session-owned state table for one session, so the snapshot
/// keep-filters can be verified by counting rows (#2859 G1.1). The
/// `pending_queue` row references a real message (`pending_queue.message_id`
/// has a foreign key to `messages.id`).
fn seed_per_session_state(
    db_path: &std::path::Path,
    session_id: &str,
    message_id: &str,
    queue_seq: i64,
) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute(
        "INSERT INTO compact_contexts (id, session_id, to_id, summary, created_at) \
         VALUES (?1, ?2, 'to-1', 'summary', 1000)",
        rusqlite::params![format!("cc-{session_id}"), session_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO planning_goals (session_id, goal_text, status, created_at) \
         VALUES (?1, 'goal', 'active', 1000)",
        [session_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO planning_todos (session_id, content, priority, is_checked, status, \
         created_at, updated_at) VALUES (?1, 'todo', 'medium', 0, 'pending', 1000, 1000)",
        [session_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO planning_scratchpad (session_id, content, created_at, updated_at) \
         VALUES (?1, 'note', 1000, 1000)",
        [session_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO pending_queue (message_id, session_id, created_at, queue_seq) \
         VALUES (?1, ?2, 1000, ?3)",
        rusqlite::params![message_id, session_id, queue_seq],
    )
    .unwrap();
}

fn session_template() -> SessionMetadata {
    SessionMetadata {
        id: "template".to_string(),
        name: None,
        status: SessionStatus::Idle,
        model: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: None,
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 0,
        updated_at: 0,
        last_viewed_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        last_message_at: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    }
}

#[tokio::test]
async fn snapshot_keep_session_retains_only_that_sessions_rows() {
    let src_dir = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    let workspace = TempDir::new().unwrap();
    seed_continuable_session(&src, workspace.path()).await;

    let dst = src_dir.path().join("snapshot.db");
    tauri_mcp_agent_lib::headless::ephemeral_db::snapshot_sqlite_db_stripping_sessions(
        &src,
        &dst,
        Some("s-cont"),
    )
    .unwrap();

    assert_eq!(count_rows(&dst, "sessions"), 1);
    assert_eq!(count_rows(&dst, "messages"), 2);
    // The continued session keeps its compact context and planning state;
    // the other session's rows in the same tables are stripped (#2859 G1.1).
    for table in [
        "compact_contexts",
        "planning_todos",
        "planning_goals",
        "planning_scratchpad",
    ] {
        assert_eq!(
            count_rows(&dst, table),
            1,
            "kept session's {table} row survives"
        );
    }
    // pending_queue belongs to a run that no longer exists: always emptied.
    assert_eq!(count_rows(&dst, "pending_queue"), 0);
    // The source is opened read-only: continuation must not touch it.
    assert_eq!(count_rows(&src, "sessions"), 2);
    assert_eq!(count_rows(&src, "messages"), 3);
}

#[tokio::test]
async fn bootstrap_session_continuation_keeps_identity_and_fork_base() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue the work", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    assert_eq!(runtime.session_id, "s-cont");
    assert!(runtime.resumed);
    assert_eq!(
        runtime.fork_base,
        tauri_mcp_agent_lib::headless::SessionForkBase {
            session_id: "s-cont".to_string(),
            message_count: 2,
            last_message_id: Some("m-2".to_string()),
        }
    );

    // The continued session keeps its own configuration and metadata; the run
    // executes under the session's workspace, not the invocation cwd.
    let meta = runtime
        .session_repo
        .get_session("s-cont")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta.name.as_deref(), Some("Titled"));
    assert_eq!(meta.created_at, 100);
    assert_eq!(runtime.resolved.model, "gpt-4o");
    assert_eq!(runtime.resolved.provider, "openai");
    assert_eq!(
        runtime.resolved.workspace_path,
        workspace.path().canonicalize().unwrap()
    );

    // The snapshot holds only this session's two history rows (plus nothing
    // of the other session), and the source profile DB is untouched.
    let snapshot_rows = count_rows(&runtime.env.db_path, "messages");
    assert_eq!(snapshot_rows, 2);
    assert_eq!(count_rows(&src, "messages"), 3);

    drop(runtime);
}

#[tokio::test]
async fn bootstrap_session_cli_flags_override_the_sessions_own_config() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    args.model = Some("flag-model".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session with --model");

    // `--model` wins; the un-flagged provider falls back to the session's own.
    assert_eq!(runtime.resolved.model, "flag-model");
    assert_eq!(runtime.resolved.provider, "openai");

    drop(runtime);
}

#[tokio::test]
async fn bootstrap_unknown_session_id_fails_with_clear_error() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("no-such-session".to_string());
    let err = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .err()
        .expect("unknown --session id must fail");
    assert!(err.contains("Session not found: no-such-session"), "{err}");

    // Failure happens after the read-only snapshot: the profile is intact.
    assert_eq!(count_rows(&src, "sessions"), 2);
}

fn list_sessions_args(
    dir: &std::path::Path,
) -> tauri_mcp_agent_lib::headless::cli::ListSessionsArgs {
    tauri_mcp_agent_lib::headless::cli::ListSessionsArgs {
        cd: Some(dir.to_path_buf()),
    }
}

/// A profile database holding sessions for two directories.
fn profile_db_with_sessions(db_path: &std::path::Path, here: &std::path::Path, other: &str) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE sessions (id TEXT, name TEXT, status TEXT, model TEXT, \
         updated_at INTEGER, workspace_override TEXT);",
    )
    .unwrap();
    let here = here.to_string_lossy().to_string();
    for (id, name, ws, updated) in [
        ("s-old", Some("Older"), here.as_str(), 100_i64),
        ("s-new", None, here.as_str(), 200_i64),
        ("s-other", Some("Elsewhere"), other, 300_i64),
    ] {
        conn.execute(
            "INSERT INTO sessions VALUES (?1, ?2, 'idle', 'gpt-x', ?3, ?4)",
            rusqlite::params![id, name, updated, ws],
        )
        .unwrap();
    }
}

#[test]
fn list_sessions_prints_this_directorys_sessions_newest_first() {
    use tauri_mcp_agent_lib::headless::run_list_sessions;
    let workspace = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("profile.db");
    profile_db_with_sessions(&db_path, workspace.path(), "/somewhere/else");

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_list_sessions(
        &list_sessions_args(workspace.path()),
        &db_path,
        &mut out,
        &mut err,
    );

    assert_eq!(code, exit_codes::SUCCESS);
    assert!(err.is_empty());
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "only this directory's sessions: {text}");
    assert_eq!(lines[0], "s-new\tidle\tgpt-x\t200\t(untitled)");
    assert_eq!(lines[1], "s-old\tidle\tgpt-x\t100\tOlder");
}

#[test]
fn list_sessions_matches_non_canonical_workspace_override() {
    use tauri_mcp_agent_lib::headless::run_list_sessions;
    let workspace = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("profile.db");

    // The app stores `workspace_override` exactly as the user gave it, so a
    // redundant segment like `<workspace>/sub/..` must still match the
    // canonicalized target directory. `sub` has to exist for canonicalization
    // to resolve the path (portable: no symlinks required).
    std::fs::create_dir(workspace.path().join("sub")).unwrap();
    let non_canonical = workspace.path().join("sub").join("..");
    assert_ne!(
        non_canonical.as_os_str(),
        workspace.path().as_os_str(),
        "the override must be a non-canonical spelling of the workspace dir"
    );

    profile_db_with_sessions(&db_path, &non_canonical, "/somewhere/else");

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_list_sessions(
        &list_sessions_args(workspace.path()),
        &db_path,
        &mut out,
        &mut err,
    );

    assert_eq!(code, exit_codes::SUCCESS);
    assert!(err.is_empty());
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "non-canonical override must still list this directory's sessions: {text}"
    );
    assert_eq!(lines[0], "s-new\tidle\tgpt-x\t200\t(untitled)");
    assert_eq!(lines[1], "s-old\tidle\tgpt-x\t100\tOlder");
}

#[test]
fn list_sessions_without_a_profile_database_is_an_empty_success() {
    use tauri_mcp_agent_lib::headless::run_list_sessions;
    let workspace = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_list_sessions(
        &list_sessions_args(workspace.path()),
        &db_dir.path().join("missing.db"),
        &mut out,
        &mut err,
    );

    assert_eq!(code, exit_codes::SUCCESS);
    assert!(out.is_empty() && err.is_empty());
}

#[test]
fn list_sessions_on_a_database_without_the_sessions_table_is_an_error() {
    use tauri_mcp_agent_lib::headless::run_list_sessions;
    let workspace = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("profile.db");
    rusqlite::Connection::open(&db_path)
        .unwrap()
        .execute_batch("CREATE TABLE unrelated (x INTEGER);")
        .unwrap();

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_list_sessions(
        &list_sessions_args(workspace.path()),
        &db_path,
        &mut out,
        &mut err,
    );

    // Not "no sessions": the schema is wrong, and the caller must be told.
    assert_eq!(code, exit_codes::ERROR);
    assert!(out.is_empty());
    assert!(String::from_utf8(err)
        .unwrap()
        .contains("failed to list sessions"));
}

#[test]
fn list_sessions_on_a_bad_row_is_an_error_not_a_shorter_list() {
    use tauri_mcp_agent_lib::headless::run_list_sessions;
    let workspace = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("profile.db");
    profile_db_with_sessions(&db_path, workspace.path(), "/somewhere/else");
    // A row whose `updated_at` is not an integer cannot be read.
    rusqlite::Connection::open(&db_path)
        .unwrap()
        .execute(
            "INSERT INTO sessions VALUES ('s-bad', 'Bad', 'idle', 'gpt-x', 'not-a-number', ?1)",
            [workspace.path().to_string_lossy().as_ref()],
        )
        .unwrap();

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_list_sessions(
        &list_sessions_args(workspace.path()),
        &db_path,
        &mut out,
        &mut err,
    );

    assert_eq!(code, exit_codes::ERROR);
    assert!(out.is_empty(), "no partial list on error");
    assert!(!err.is_empty());
}

#[test]
fn list_sessions_for_a_missing_directory_is_an_error() {
    use tauri_mcp_agent_lib::headless::run_list_sessions;
    let db_dir = TempDir::new().unwrap();

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_list_sessions(
        &list_sessions_args(&db_dir.path().join("no-such-dir")),
        &db_dir.path().join("profile.db"),
        &mut out,
        &mut err,
    );

    assert_eq!(code, exit_codes::ERROR);
    assert!(String::from_utf8(err).unwrap().contains("does not exist"));
}

// ── #2857: headless runs never start the scheduler ─────────────────────────

#[tokio::test]
async fn headless_run_never_starts_the_scheduler() {
    // The scheduler belongs to the GUI backend (#2857): a headless run must
    // never fire scheduled tasks, even when the profile carries a due
    // GLOBAL task whose assistant exists and whose cron window is open.
    let shared_dir = TempDir::new().unwrap();
    let workspace_dir = TempDir::new().unwrap();
    let shared_db_path = shared_dir.path().join("shared_profile.db");

    let shared_db = init_ephemeral_database(&shared_db_path).await.unwrap();
    let shared_assistants = SqliteAssistantRepository::new(shared_db.clone());
    shared_assistants
        .create_assistant(
            "asst-scheduler".to_string(),
            "Scheduler Assistant".to_string(),
            json!({
                "name": "Scheduler Assistant",
                "systemPrompt": "You keep the schedule."
            })
            .to_string(),
        )
        .await
        .unwrap();

    let due_at = chrono::Utc::now().timestamp_millis() - 60_000;
    let shared_tasks = SqliteScheduledTaskRepository::new(shared_db.clone());
    shared_tasks
        .create_scheduled_task(CreateScheduledTaskParams {
            id: "task-headless-guard".to_string(),
            name: "Headless guard".to_string(),
            task_category: TASK_CATEGORY_GLOBAL.to_string(),
            cron_expression: Some("0 * * * *".to_string()),
            schedule_timezone: "local".to_string(),
            assistant_id: "asst-scheduler".to_string(),
            message: "fire only if a scheduler is running".to_string(),
            execution_mode: ExecutionMode::Normal,
            created_by_session_id: None,
            session_id: None,
            workspace_override: None,
            reset_planning_state: false,
            next_run_at: Some(due_at),
        })
        .await
        .unwrap();

    let args = default_headless_args("guard prompt", workspace_dir.path());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&shared_db_path), &args)
        .await
        .expect("bootstrap ephemeral headless runtime");

    // Any scheduler in this process would run its immediate startup check
    // and fire the due task well inside this window.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let guarded = shared_tasks
        .get_scheduled_task("task-headless-guard")
        .await
        .unwrap()
        .expect("the task must stay in the shared profile DB");
    assert_eq!(
        guarded.last_run_at, None,
        "a headless run must never execute scheduled tasks"
    );
    assert_eq!(
        guarded.next_run_at,
        Some(due_at),
        "a headless run must not rewrite the schedule"
    );
    assert_eq!(
        guarded.run_claim_run, None,
        "a headless run must not claim tasks for execution"
    );

    let ephemeral_tasks = SqliteScheduledTaskRepository::new(runtime.db.clone())
        .list_scheduled_tasks(None)
        .await
        .unwrap();
    assert!(
        ephemeral_tasks.is_empty(),
        "the ephemeral runtime must not carry profile tasks"
    );
}

// ---------------------------------------------------------------------------
// S8 (#2860): write-back of run history into the profile database.
//
// These drive `write_back_run` directly on a bootstrapped runtime: rows in
// the ephemeral snapshot are exactly what the driver would have persisted, so
// they are the write-back's input.
// ---------------------------------------------------------------------------

fn query_i64(db_path: &std::path::Path, sql: &str) -> i64 {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn query_strings(db_path: &std::path::Path, sql: &str) -> Vec<String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .flatten()
        .collect()
}

fn seed_ownership(db_path: &std::path::Path, session_id: &str, heartbeat_at: i64) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute(
        "INSERT INTO session_ownership (session_id, owner_run, owner_pid, owner_started_marker, heartbeat_at) \
         VALUES (?1, 'gui-run', 1, 0, ?2)",
        rusqlite::params![session_id, heartbeat_at],
    )
    .unwrap();
}

#[tokio::test]
async fn write_back_fresh_run_inserts_exactly_one_new_session() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let args = default_headless_args("fresh run", workspace.path());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("fresh bootstrap");
    assert!(!runtime.resumed);
    insert_session_messages(
        &runtime.env.db_path,
        &runtime.session_id,
        &[("f-m-1", 900), ("f-m-2", 901)],
    );

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.session_id, runtime.session_id);
    assert!(outcome.session_id.starts_with("headless-"));
    assert_eq!(outcome.forked_from, None);

    // The two seeded sessions plus exactly one new headless session (#2860).
    assert_eq!(count_rows(&src, "sessions"), 3);
    assert_eq!(
        query_i64(
            &src,
            "SELECT COUNT(*) FROM sessions WHERE forked_from IS NOT NULL"
        ),
        0
    );
    // Only the run's two messages were added; seeded history is untouched.
    assert_eq!(count_rows(&src, "messages"), 5);
    let run_messages = query_strings(
        &src,
        &format!(
            "SELECT id FROM messages WHERE session_id = '{}' ORDER BY created_at",
            runtime.session_id
        ),
    );
    assert_eq!(run_messages, vec!["f-m-1".to_string(), "f-m-2".to_string()]);

    drop(runtime);
}

#[tokio::test]
async fn write_back_session_run_merges_into_original_session() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");
    assert!(runtime.resumed);
    insert_session_messages(&runtime.env.db_path, "s-cont", &[("m-3", 900)]);

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.session_id, "s-cont");
    assert_eq!(outcome.forked_from, None);

    // Merge: no new session, the run's message appended to the original.
    assert_eq!(count_rows(&src, "sessions"), 2);
    assert_eq!(count_rows(&src, "messages"), 4);
    assert_eq!(
        query_strings(
            &src,
            "SELECT id FROM messages WHERE session_id = 's-cont' ORDER BY created_at"
        ),
        vec!["m-1".to_string(), "m-2".to_string(), "m-3".to_string()]
    );
    // The merge surfaced the run in the session list.
    assert!(query_i64(&src, "SELECT updated_at FROM sessions WHERE id = 's-cont'") > 100);

    drop(runtime);
}

#[tokio::test]
async fn write_back_session_run_forks_when_profile_changed_after_snapshot() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    // The GUI appended its own message after the snapshot was taken.
    insert_session_messages(&src, "s-cont", &[("m-9", 800)]);
    insert_session_messages(&runtime.env.db_path, "s-cont", &[("m-3", 900)]);

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.forked_from.as_deref(), Some("s-cont"));
    assert_ne!(outcome.session_id, "s-cont");
    assert!(outcome.session_id.starts_with("headless-"));

    assert_eq!(count_rows(&src, "sessions"), 3);
    assert_eq!(
        query_strings(
            &src,
            &format!(
                "SELECT forked_from FROM sessions WHERE id = '{}'",
                outcome.session_id
            )
        ),
        vec!["s-cont".to_string()]
    );
    // The fork carries the run's full history (fresh message ids); the GUI's
    // own rows keep theirs.
    assert_eq!(
        query_i64(
            &src,
            &format!(
                "SELECT COUNT(*) FROM messages WHERE session_id = '{}'",
                outcome.session_id
            )
        ),
        3
    );
    assert_eq!(
        query_strings(
            &src,
            "SELECT id FROM messages WHERE session_id = 's-cont' ORDER BY created_at"
        ),
        vec!["m-1".to_string(), "m-2".to_string(), "m-9".to_string()]
    );

    drop(runtime);
}

#[tokio::test]
async fn write_back_session_run_forks_while_gui_owns_the_session() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    seed_ownership(&src, "s-cont", chrono::Utc::now().timestamp_millis());

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.forked_from.as_deref(), Some("s-cont"));
    assert_eq!(count_rows(&src, "sessions"), 3);

    drop(runtime);
}

#[tokio::test]
async fn write_back_merges_despite_a_stale_ownership_row() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    // A dead GUI (lapsed heartbeat) must not block the merge.
    seed_ownership(
        &src,
        "s-cont",
        chrono::Utc::now().timestamp_millis() - 60_000,
    );
    insert_session_messages(&runtime.env.db_path, "s-cont", &[("m-3", 900)]);

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.session_id, "s-cont");
    assert_eq!(outcome.forked_from, None);
    assert_eq!(count_rows(&src, "sessions"), 2);

    drop(runtime);
}

#[tokio::test]
async fn write_back_redacts_profile_secrets_from_written_rows() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    // A provider API key lives in the profile's settings and flows into the
    // snapshot exactly like a real user profile's would.
    let secret = "sk-REDACTME-0123456789abcdef";
    {
        let conn = rusqlite::Connection::open(&src).unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, created_at, updated_at) \
             VALUES ('serviceConfigs', ?1, 0, 0)",
            [format!(r#"{{"openai":{{"apiKey":"{secret}"}}}}"#)],
        )
        .unwrap();
    }

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    {
        let conn = rusqlite::Connection::open(&runtime.env.db_path).unwrap();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at, updated_at) \
             VALUES ('r-m-1', 's-cont', 'assistant', ?1, 900, 900)",
            [format!(
                r#"[{{"type":"text","text":"configured with {secret}"}}]"#
            )],
        )
        .unwrap();
    }

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.session_id, "s-cont");

    let stored = query_strings(&src, "SELECT content FROM messages WHERE id = 'r-m-1'");
    assert_eq!(stored.len(), 1);
    assert!(
        !stored[0].contains(secret),
        "the API key must never reach the profile DB in clear text: {}",
        stored[0]
    );
    assert!(stored[0].contains("***"));

    drop(runtime);
}

#[tokio::test]
async fn write_back_failed_transaction_leaves_profile_unchanged() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let args = default_headless_args("fresh run", workspace.path());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("fresh bootstrap");
    insert_session_messages(&runtime.env.db_path, &runtime.session_id, &[("x-m-1", 900)]);

    // Pre-empt the run's session id so the transaction's first statement
    // fails on the primary key.
    {
        let conn = rusqlite::Connection::open(&src).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, status, model, provider, created_at, updated_at, \
             is_bookmarked, execution_mode, workspace_isolation) \
             VALUES (?1, 'idle', 'gpt-4o', 'openai', 1, 1, 0, 'normal', 'host')",
            [runtime.session_id.as_str()],
        )
        .unwrap();
    }

    assert!(write_back_run(&runtime).is_err());
    // Rolled back atomically: no partial session or message writes.
    assert_eq!(count_rows(&src, "sessions"), 3);
    assert_eq!(count_rows(&src, "messages"), 3);

    drop(runtime);
}

#[tokio::test]
async fn write_back_creates_the_profile_database_when_none_exists() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("first_profile.db"); // never created
    assert!(!src.exists());

    let args = default_headless_args("first ever run", workspace.path());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap without an existing profile DB");
    insert_session_messages(&runtime.env.db_path, &runtime.session_id, &[("n-m-1", 900)]);

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.session_id, runtime.session_id);
    assert!(
        src.exists(),
        "a first run persists itself into a new profile DB"
    );
    assert_eq!(count_rows(&src, "sessions"), 1);
    assert_eq!(count_rows(&src, "messages"), 1);

    drop(runtime);
}

// --- Gatekeeper round 1 fixes (#2876) ------------------------------------

#[tokio::test]
async fn write_back_fails_closed_when_the_secret_source_is_unreadable() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let args = default_headless_args("fresh run", workspace.path());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("fresh bootstrap");
    insert_session_messages(&runtime.env.db_path, &runtime.session_id, &[("u-m-1", 900)]);

    // Break the redaction key source: without `settings` the secret list
    // cannot be built, so the write-back must abort rather than persist
    // rows that might be unredacted.
    {
        let conn = rusqlite::Connection::open(&runtime.env.db_path).unwrap();
        conn.execute_batch("DROP TABLE settings;").unwrap();
    }

    let err = write_back_run(&runtime).expect_err("unreadable secret source must abort write-back");
    assert!(
        err.contains("settings"),
        "the error must name the failed source: {err}"
    );
    assert_eq!(
        count_rows(&src, "sessions"),
        2,
        "nothing may be written when redaction is impossible"
    );
    assert_eq!(count_rows(&src, "messages"), 3);

    drop(runtime);
}

#[tokio::test]
async fn write_back_leaves_short_values_under_secret_like_keys_intact() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    // `maxTokens` matches the secret key heuristic but its value is an
    // ordinary short number; masking "4096" would corrupt history.
    {
        let conn = rusqlite::Connection::open(&src).unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, created_at, updated_at) \
             VALUES ('maxTokens', '4096', 0, 0)",
            [],
        )
        .unwrap();
    }

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    {
        let conn = rusqlite::Connection::open(&runtime.env.db_path).unwrap();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at, updated_at) \
             VALUES ('sv-m-1', 's-cont', 'assistant', ?1, 900, 900)",
            [r#"[{"type":"text","text":"used 4096 tokens total"}]"#],
        )
        .unwrap();
    }

    let outcome = write_back_run(&runtime).unwrap();
    assert_eq!(outcome.session_id, "s-cont");

    let stored = query_strings(&src, "SELECT content FROM messages WHERE id = 'sv-m-1'");
    assert_eq!(stored.len(), 1);
    assert!(
        stored[0].contains("4096"),
        "a short value under a key-like name must not be masked: {}",
        stored[0]
    );
    assert!(
        !stored[0].contains("***"),
        "no masking expected: {}",
        stored[0]
    );

    drop(runtime);
}

/// #2860 acceptance: every terminal outcome writes back before cleanup —
/// through `run_with_driver`, not just by calling `write_back_run` directly.
#[tokio::test]
async fn write_back_persists_error_timeout_and_interrupted_runs() {
    // 1. Driver error -> the session row lands in the profile DB.
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("err_profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut err_args = default_headless_args("failing task", workspace.path());
    err_args.output_format = OutputFormat::Json;
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &err_args)
        .await
        .expect("bootstrap for error run");
    let (_tx, rx) = watch::channel(None);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let payload = runtime
        .run_with_driver(
            &err_args,
            rx,
            |_session_id, _repo, _active, _resolved| async move {
                Err("simulated driver failure".to_string())
            },
            &mut stdout,
            &mut stderr,
        )
        .await;
    assert_eq!(payload.status, HeadlessStatus::Error);
    assert_eq!(
        query_i64(
            &src,
            &format!(
                "SELECT COUNT(*) FROM sessions WHERE id = '{}'",
                payload.session_id
            )
        ),
        1,
        "an Error run must still write back its session row"
    );

    // 2. Timeout -> the session row lands in the profile DB.
    let src2_dir = TempDir::new().unwrap();
    let src2 = src2_dir.path().join("timeout_profile.db");
    seed_continuable_session(&src2, workspace.path()).await;
    let mut to_args = default_headless_args("long task", workspace.path());
    to_args.output_format = OutputFormat::Json;
    to_args.timeout_secs = Some(1);
    let runtime2 = EphemeralHeadlessRuntime::bootstrap(Some(&src2), &to_args)
        .await
        .expect("bootstrap for timeout run");
    let (_tx2, rx2) = watch::channel(None);
    let mut stdout2 = Vec::new();
    let mut stderr2 = Vec::new();
    let payload2 = runtime2
        .run_with_driver(
            &to_args,
            rx2,
            |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(())
            },
            &mut stdout2,
            &mut stderr2,
        )
        .await;
    assert_eq!(payload2.status, HeadlessStatus::Timeout);
    assert_eq!(
        query_i64(
            &src2,
            &format!(
                "SELECT COUNT(*) FROM sessions WHERE id = '{}'",
                payload2.session_id
            )
        ),
        1,
        "a Timeout run must still write back its session row"
    );

    // 3. Interrupt -> the session row lands in the profile DB.
    let src3_dir = TempDir::new().unwrap();
    let src3 = src3_dir.path().join("interrupt_profile.db");
    seed_continuable_session(&src3, workspace.path()).await;
    let mut int_args = default_headless_args("interruptible task", workspace.path());
    int_args.output_format = OutputFormat::Json;
    let runtime3 = EphemeralHeadlessRuntime::bootstrap(Some(&src3), &int_args)
        .await
        .expect("bootstrap for interrupt run");
    let (tx3, rx3) = watch::channel(None);
    let mut stdout3 = Vec::new();
    let mut stderr3 = Vec::new();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        let _ = tx3.send(Some(InterruptSignal::SigInt));
    });
    let payload3 = runtime3
        .run_with_driver(
            &int_args,
            rx3,
            |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(())
            },
            &mut stdout3,
            &mut stderr3,
        )
        .await;
    assert_eq!(payload3.status, HeadlessStatus::Interrupted);
    assert_eq!(
        query_i64(
            &src3,
            &format!(
                "SELECT COUNT(*) FROM sessions WHERE id = '{}'",
                payload3.session_id
            )
        ),
        1,
        "an Interrupted run must still write back its session row"
    );
}

/// The merge-or-fork decision must be taken inside the write-back's own
/// `BEGIN IMMEDIATE` transaction, on the state the concurrent writer has
/// committed — never on the pre-transaction snapshot.
#[tokio::test]
async fn write_back_decides_fork_on_concurrent_writer_commit() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");
    insert_session_messages(&runtime.env.db_path, "s-cont", &[("m-3", 900)]);

    // A second connection (a GUI backend stand-in) holds the write lock.
    // The write-back's BEGIN IMMEDIATE must then wait (busy_timeout) and
    // decide after this writer commits its own message.
    let writer = rusqlite::Connection::open(&src).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE;").unwrap();

    let outcome = std::thread::scope(|scope| {
        let handle = scope.spawn(|| write_back_run(&runtime));
        std::thread::sleep(Duration::from_millis(300));
        writer
            .execute(
                "INSERT INTO messages (id, session_id, role, content, created_at, updated_at) \
                 VALUES ('m-9', 's-cont', 'user', '[]', 800, 800)",
                [],
            )
            .unwrap();
        writer.execute_batch("COMMIT;").unwrap();
        handle.join().unwrap()
    })
    .expect("write-back must succeed under contention");

    // The writer's m-9 was committed before the transaction began, so the
    // profile no longer matches the snapshot: fork, not merge.
    assert_eq!(outcome.forked_from.as_deref(), Some("s-cont"));
    assert_ne!(outcome.session_id, "s-cont");
    assert_eq!(
        query_strings(
            &src,
            "SELECT id FROM messages WHERE session_id = 's-cont' ORDER BY created_at"
        ),
        vec!["m-1".to_string(), "m-2".to_string(), "m-9".to_string()]
    );

    drop(runtime);
}

/// A `--session` run that forks through `run_with_driver` must surface the
/// fork in the payload and the serialized JSON result (#2860).
#[tokio::test]
async fn run_with_driver_fork_reports_forked_from_in_payload_and_json() {
    let src_dir = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let src = src_dir.path().join("profile.db");
    seed_continuable_session(&src, workspace.path()).await;

    let mut args = default_headless_args("continue", workspace.path());
    args.session = Some("s-cont".to_string());
    args.output_format = OutputFormat::Json;
    let runtime = EphemeralHeadlessRuntime::bootstrap(Some(&src), &args)
        .await
        .expect("bootstrap --session");

    // The GUI appended its own message after the snapshot was taken (a
    // committed write through a second connection).
    insert_session_messages(&src, "s-cont", &[("m-9", 800)]);

    // The test driver persists its own message to the ephemeral snapshot
    // exactly like a real driver run does, so the fork must carry it.
    let ephemeral_db = runtime.env.db_path.clone();
    let (_tx, rx) = watch::channel(None);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let payload = runtime
        .run_with_driver(
            &args,
            rx,
            move |session_id, _repo, active_sessions, _resolved| async move {
                {
                    let mut active = active_sessions.write().await;
                    let session = active.get_mut(&session_id).unwrap();
                    session.metadata.status = SessionStatus::Busy;
                    session.messages.write().await.push(make_assistant_message(
                        &session_id,
                        "msg-final",
                        "finished the titled session.",
                        None,
                    ));
                    session.metadata.status = SessionStatus::Idle;
                }
                insert_session_messages(&ephemeral_db, &session_id, &[("msg-final", 900)]);
                Ok(())
            },
            &mut stdout,
            &mut stderr,
        )
        .await;

    assert_eq!(payload.status, HeadlessStatus::Completed);
    assert_eq!(payload.forked_from.as_deref(), Some("s-cont"));
    assert_ne!(payload.session_id, "s-cont");
    assert!(payload.session_id.starts_with("headless-"));

    let parsed: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(parsed["type"], "result");
    assert_eq!(parsed["status"], "completed");
    assert_eq!(parsed["sessionId"], payload.session_id.as_str());
    assert_eq!(parsed["forkedFrom"], "s-cont");

    // The fork row exists with forked_from set and carries the full history
    // (snapshot + the run's message) under fresh ids.
    assert_eq!(
        query_strings(
            &src,
            &format!(
                "SELECT forked_from FROM sessions WHERE id = '{}'",
                payload.session_id
            )
        ),
        vec!["s-cont".to_string()]
    );
    assert_eq!(
        query_i64(
            &src,
            &format!(
                "SELECT COUNT(*) FROM messages WHERE session_id = '{}'",
                payload.session_id
            )
        ),
        3
    );
}
