use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::llm::natural_recovery::LoopPreventionKind;
use tauri_mcp_agent_lib::agent::llm::preprocess_assistant_tool_calls_for_testing;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::{init_active_sessions, try_get_active_sessions};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

fn test_message(
    id: &str,
    role: &str,
    tool_calls: Option<Vec<ToolCall>>,
    tool_call_id: Option<&str>,
    metadata: Option<serde_json::Value>,
    text: &str,
    is_error: Option<bool>,
) -> Message {
    let metadata = match (metadata, is_error) {
        (Some(mut value), Some(true)) => {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("toolError".to_string(), serde_json::Value::Bool(true));
            }
            Some(value)
        }
        (None, Some(true)) => Some(serde_json::json!({ "toolError": true })),
        (other, _) => other,
    };

    Message {
        id: id.to_string(),
        session_id: "loop-prevention-session".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls,
        tool_call_id: tool_call_id.map(str::to_string),
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
        metadata,
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
        name: Some("loop-prevention".to_string()),
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
        bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        pending_reasoning_budget_nudge: Arc::new(RwLock::new(None)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
    }
}

fn repeated_success_history(repeated_args: &str, repeated_success: &str) -> Vec<Message> {
    vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            None,
            repeated_success,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            None,
            repeated_success,
            Some(false),
        ),
    ]
}

fn ensure_active_sessions() -> Arc<RwLock<HashMap<String, AgentSession>>> {
    if let Some(existing) = try_get_active_sessions() {
        return existing.clone();
    }

    let sessions = Arc::new(RwLock::new(HashMap::new()));
    init_active_sessions(sessions.clone());
    sessions
}

fn repeated_error_history(repeated_args: &str, repeated_error: &str) -> Vec<Message> {
    vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
    ]
}

#[tokio::test]
async fn natural_recovery_success_keeps_original_tool_call_and_registers_short_circuit() {
    let session_id = "loop-prevention-success-session";
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_success = "src/main.ts contents";
    let history = repeated_success_history(repeated_args, repeated_success);

    let active_sessions = ensure_active_sessions();
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, history),
        );
    }

    let mut assistant_message = test_message(
        "assistant-3",
        "assistant",
        Some(vec![test_tool_call(
            "tc-3",
            "workspace__readFile",
            repeated_args,
        )]),
        None,
        None,
        "",
        None,
    );

    let preprocess_result = preprocess_assistant_tool_calls_for_testing(
        &active_sessions,
        session_id,
        &mut assistant_message,
    )
    .await;

    let tool_calls = assistant_message
        .tool_calls
        .as_ref()
        .expect("tool calls preserved");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, "workspace__readFile");
    assert_eq!(tool_calls[0].id, "tc-3");

    let short_circuit = preprocess_result
        .loop_prevention_short_circuits
        .get("tc-3")
        .expect("short circuit registered");
    assert_eq!(
        short_circuit.kind,
        LoopPreventionKind::RepeatedSuccessOutcome
    );
    assert_eq!(short_circuit.count, 3);
    assert!(preprocess_result.forced_stop.is_none());
}

#[tokio::test]
async fn natural_recovery_error_keeps_original_tool_call_and_registers_short_circuit() {
    let session_id = "loop-prevention-error-session";
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_error = "Error: file not found";
    let history = repeated_error_history(repeated_args, repeated_error);

    let active_sessions = ensure_active_sessions();
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, history),
        );
    }

    let mut assistant_message = test_message(
        "assistant-3",
        "assistant",
        Some(vec![test_tool_call(
            "tc-3",
            "workspace__readFile",
            repeated_args,
        )]),
        None,
        None,
        "",
        None,
    );

    let preprocess_result = preprocess_assistant_tool_calls_for_testing(
        &active_sessions,
        session_id,
        &mut assistant_message,
    )
    .await;

    let tool_calls = assistant_message
        .tool_calls
        .as_ref()
        .expect("tool calls preserved");
    assert_eq!(tool_calls[0].function.name, "workspace__readFile");

    let short_circuit = preprocess_result
        .loop_prevention_short_circuits
        .get("tc-3")
        .expect("short circuit registered");
    assert_eq!(short_circuit.kind, LoopPreventionKind::RepeatedErrorOutcome);
    let guidance =
        tauri_mcp_agent_lib::agent::llm::natural_recovery::build_loop_prevention_guidance(
            short_circuit,
        );
    assert!(
        !guidance.contains("planning__reflect"),
        "soft error outcome guidance should not recommend reflect: {guidance}"
    );
    assert!(preprocess_result.forced_stop.is_none());
}

#[tokio::test]
async fn natural_recovery_preserves_original_tool() {
    let session_id = "loop-prevention-no-think-session";
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let history = repeated_success_history(repeated_args, "same content");

    let active_sessions = ensure_active_sessions();
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, history),
        );
    }

    let mut assistant_message = test_message(
        "assistant-3",
        "assistant",
        Some(vec![test_tool_call(
            "tc-3",
            "workspace__readFile",
            repeated_args,
        )]),
        None,
        None,
        "",
        None,
    );

    preprocess_assistant_tool_calls_for_testing(
        &active_sessions,
        session_id,
        &mut assistant_message,
    )
    .await;

    let tool_calls = assistant_message.tool_calls.as_ref().expect("tool calls");
    // Regression: natural recovery must keep the original tool (never substitute
    // scratchpad__think or any other recovery tool).
    assert_eq!(tool_calls[0].function.name, "workspace__readFile");
}
