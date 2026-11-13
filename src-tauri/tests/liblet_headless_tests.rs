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
use tauri_mcp_agent_lib::headless::{init_ephemeral_database, EphemeralHeadlessRuntime};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    AssistantRepository, SessionMetadata, SessionRepository, SessionStatus, SettingsRepository,
    SqliteAssistantRepository, SqliteSessionRepository, SqliteSettingsRepository,
};
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

    // Shared GUI session list must remain untouched (only gui-session-1)
    let remaining_shared = shared_sessions.get_all_sessions().await.unwrap();
    assert_eq!(remaining_shared.len(), 1);
    assert_eq!(remaining_shared[0].id, "gui-session-1");

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

    // Shared DB still has zero sessions after 4 concurrent headless runs
    let shared_sessions = SqliteSessionRepository::new(shared_db);
    assert!(shared_sessions.get_all_sessions().await.unwrap().is_empty());
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
