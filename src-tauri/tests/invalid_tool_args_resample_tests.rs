//! Windows-safe coverage for the invalid tool-call arguments policy.
//!
//! A reply whose tool-call arguments are not a JSON object (for example, cut
//! off at the output-token limit) must not go into history. It is resampled up
//! to `INVALID_TOOL_ARGS_MAX_RESAMPLES` times; then the circuit breaker stops
//! the turn with a `ui__circuitBreak` call that has valid JSON arguments.

use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc};

use sea_orm::Database;
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::llm::{
    apply_invalid_tool_args_policy_for_testing, handle_llm_response, InvalidToolArgsOutcome,
    INVALID_TOOL_ARGS_MAX_RESAMPLES,
};
use tauri_mcp_agent_lib::agent::state::{AgentSession, PendingEventManager};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

fn message(id: &str, role: &str, tool_calls: Option<Vec<ToolCall>>, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "invalid-tool-args-session".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata: None,
        usage: None,
        prompt_tokens: None,
    }
}

fn test_tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: name.to_string(),
            arguments: arguments.to_string(),
        },
    }
}

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("invalid-tool-args-resample".to_string()),
        status: SessionStatus::Idle,
        model: "gemini-2.5-pro".to_string(),
        provider: "google".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Unsafe,
        workspace_override: None,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    }
}

fn build_active_session(session_id: &str, messages: Vec<Message>) -> AgentSession {
    AgentSession {
        metadata: build_session_metadata(session_id),
        is_running: false,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(messages)),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
        invalid_tool_args_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        pending_user_prompt: tauri_mcp_agent_lib::agent::state::new_user_prompt_slot(),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: tauri_mcp_agent_lib::agent::state::CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        last_session_context_snapshot: Arc::new(RwLock::new(None)),
        session_context_turns_since_force_fresh: Arc::new(RwLock::new(0)),
        tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
        tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
        max_input_context_override: None,
    }
}

/// The tool-call arguments of trace step 175: a one-line test command that
/// degenerate generation made run to the output-token limit.
fn truncated_arguments() -> String {
    format!(
        "{{\"command\":\"pnpm rust:test --lib plugin_options{}",
        " --lib plugin_options".repeat(2000)
    )
}

fn reply(id: &str, arguments: &str) -> Message {
    message(
        id,
        "assistant",
        Some(vec![test_tool_call(
            &format!("{id}-call"),
            "workspace__spawnProcess",
            arguments,
        )]),
        "",
    )
}

async fn session_with_history(session_id: &str) -> Arc<RwLock<HashMap<String, AgentSession>>> {
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    sessions.write().await.insert(
        session_id.to_string(),
        build_active_session(
            session_id,
            vec![message("user-1", "user", None, "Re-run the test.")],
        ),
    );
    sessions
}

async fn history(
    sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Vec<Message> {
    let sessions = sessions.read().await;
    let session = sessions.get(session_id).expect("session");
    let messages = session.messages.read().await.clone();
    messages
}

async fn retry_count(
    sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> u32 {
    let sessions = sessions.read().await;
    let session = sessions.get(session_id).expect("session");
    let count = *session.invalid_tool_args_retry_count.read().await;
    count
}

fn has_invalid_tool_args(messages: &[Message]) -> bool {
    messages
        .iter()
        .flat_map(|m| m.tool_calls.iter().flatten())
        .any(|call| {
            !serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                .is_ok_and(|value| value.is_object())
        })
}

#[tokio::test]
async fn truncated_tool_args_are_resampled_and_a_valid_retry_leaves_history_clean() {
    let session_id = "invalid-tool-args-retry-ok";
    let sessions = session_with_history(session_id).await;
    let before = history(&sessions, session_id).await;

    let mut truncated = reply("assistant-truncated", &truncated_arguments());
    let outcome =
        apply_invalid_tool_args_policy_for_testing(&sessions, session_id, &mut truncated).await;
    assert_eq!(
        outcome,
        InvalidToolArgsOutcome::Resample {
            tool_name: "workspace__spawnProcess".to_string(),
            attempt: 1,
            max_resamples: INVALID_TOOL_ARGS_MAX_RESAMPLES,
        }
    );
    // The discarded reply is not put into history and no placeholder appears.
    assert_eq!(history(&sessions, session_id).await.len(), before.len());
    assert_eq!(retry_count(&sessions, session_id).await, 1);

    let mut valid = reply(
        "assistant-valid",
        r#"{"command":"pnpm rust:test --lib plugin_options"}"#,
    );
    let outcome =
        apply_invalid_tool_args_policy_for_testing(&sessions, session_id, &mut valid).await;
    assert_eq!(outcome, InvalidToolArgsOutcome::Valid);
    assert_eq!(
        valid.tool_calls.as_ref().unwrap()[0].function.arguments,
        r#"{"command":"pnpm rust:test --lib plugin_options"}"#
    );
    assert_eq!(retry_count(&sessions, session_id).await, 0);

    // The response layer then caches the valid reply. History holds only
    // executable, JSON-object arguments.
    sessions
        .read()
        .await
        .get(session_id)
        .unwrap()
        .messages
        .write()
        .await
        .push(valid);
    let after = history(&sessions, session_id).await;
    assert_eq!(after.len(), before.len() + 1);
    assert!(!has_invalid_tool_args(&after));
    assert!(!serde_json::to_string(&after).unwrap().contains("<redacted"));
}

#[tokio::test]
async fn malformed_complete_tool_args_are_resampled_too() {
    let session_id = "invalid-tool-args-malformed";
    let sessions = session_with_history(session_id).await;

    for arguments in [
        "[1, 2]",
        r#"{"command": "ls",}"#,
        "<redacted: 10 bytes, truncated>",
    ] {
        *sessions
            .read()
            .await
            .get(session_id)
            .unwrap()
            .invalid_tool_args_retry_count
            .write()
            .await = 0;
        let mut malformed = reply("assistant-malformed", arguments);
        let outcome =
            apply_invalid_tool_args_policy_for_testing(&sessions, session_id, &mut malformed).await;
        assert!(
            matches!(outcome, InvalidToolArgsOutcome::Resample { attempt: 1, .. }),
            "{arguments}: {outcome:?}"
        );
    }
}

#[tokio::test]
async fn invalid_tool_args_after_all_resamples_trip_the_circuit_breaker() {
    let session_id = "invalid-tool-args-circuit-break";
    let sessions = session_with_history(session_id).await;
    let arguments = truncated_arguments();

    for attempt in 1..=INVALID_TOOL_ARGS_MAX_RESAMPLES {
        let mut truncated = reply(&format!("assistant-{attempt}"), &arguments);
        let outcome =
            apply_invalid_tool_args_policy_for_testing(&sessions, session_id, &mut truncated).await;
        assert!(
            matches!(outcome, InvalidToolArgsOutcome::Resample { attempt: a, .. } if a == attempt),
            "attempt {attempt}: {outcome:?}"
        );
    }

    let mut last = reply("assistant-last", &arguments);
    let outcome =
        apply_invalid_tool_args_policy_for_testing(&sessions, session_id, &mut last).await;
    assert_eq!(
        outcome,
        InvalidToolArgsOutcome::CircuitBreak {
            tool_name: "workspace__spawnProcess".to_string(),
            attempts: INVALID_TOOL_ARGS_MAX_RESAMPLES + 1,
        }
    );

    // The invalid call is replaced by the interactive circuit break, whose
    // arguments are a JSON object that the provider accepts.
    let tool_calls = last.tool_calls.as_ref().expect("circuit break call");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, "ui__circuitBreak");
    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("JSON arguments");
    assert_eq!(args["toolName"], "workspace__spawnProcess");
    assert_eq!(
        args["repetitionCount"],
        serde_json::json!(INVALID_TOOL_ARGS_MAX_RESAMPLES + 1)
    );
    assert_eq!(
        args["args"],
        format!("<redacted: {} bytes, truncated>", arguments.len())
    );
    assert!(args["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("invalid arguments")));
    assert!(!has_invalid_tool_args(std::slice::from_ref(&last)));

    // The budget starts again for the next turn.
    assert_eq!(retry_count(&sessions, session_id).await, 0);
}

/// A cancelled session must not be resampled. `handle_llm_response` runs
/// response admission first, and the guard rejects a reply whose session has
/// a cancelled cancellation token (`WORKFLOW_CANCELLED_ERROR`) before the
/// invalid tool-call-args policy runs. So an invalid reply on a cancelled
/// session is dropped: history is unchanged, the resample counter is not
/// consumed, and no new completion is requested (the early `Err` return).
#[tokio::test]
async fn cancelled_session_with_invalid_tool_args_is_not_resampled() {
    let session_id = "invalid-tool-args-cancelled";
    let sessions = session_with_history(session_id).await;
    let before = history(&sessions, session_id).await;

    let token = sessions
        .read()
        .await
        .get(session_id)
        .expect("session")
        .cancellation_token
        .clone();
    token.cancel();

    // These exist only to satisfy the signature. Admission rejects before any
    // of them is used, so the session is never persisted nor a completion sent.
    let session_repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let session_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new_with_base_dir(
            temp_dir.path().join("session-root"),
        )
        .expect("session manager"),
    );
    let proxy_manager = Arc::new(MCPServiceProxyManager::new(Arc::new(db), session_manager));
    let mock_app = tauri::test::mock_app();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_app.handle() as *const tauri::AppHandle<tauri::test::MockRuntime>
            as *const tauri::AppHandle)
    };

    let result = handle_llm_response(
        &session_repo,
        &sessions,
        &proxy_manager,
        app_handle,
        session_id.to_string(),
        reply("assistant-cancelled", &truncated_arguments()),
    )
    .await;

    assert_eq!(result, Err("Workflow was cancelled".to_string()));
    // The invalid reply never enters history…
    assert_eq!(history(&sessions, session_id).await.len(), before.len());
    // …and the resample counter stays untouched.
    assert_eq!(retry_count(&sessions, session_id).await, 0);
}
