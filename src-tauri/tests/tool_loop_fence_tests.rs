mod common;

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::llm::{
    initialize_pending_execution_for_testing, preprocess_assistant_tool_calls_for_testing,
    CompactionParentRequest,
};
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionStatus, SettingsRepository, SqliteSettingsRepository,
};
use tauri_mcp_agent_lib::{
    init_active_sessions, set_settings_repository, try_get_active_sessions,
    try_get_settings_repository,
};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    let agent_config = tauri_mcp_agent_lib::agent::AgentConfig {
        max_tokens: Some(1024),
        ..Default::default()
    };

    SessionMetadata {
        id: session_id.to_string(),
        name: Some("tool-loop-fence".to_string()),
        status: SessionStatus::Idle,
        model: "gemini-2.5-pro".to_string(),
        provider: "google".to_string(),
        agent_config: Some(agent_config.to_json().expect("agent config json")),
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
        yolo_mode: false,
        unsafe_mode: true,
        workspace_override: None,
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
        yolo_mode: Arc::new(AtomicBool::new(false)),
        unsafe_mode: Arc::new(AtomicBool::new(true)),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(messages)),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(Some(CompactionParentRequest {
            model: "gemini-2.5-pro".to_string(),
            provider: "google".to_string(),
            system_prompt: Some(
                "System prompt ".repeat(80) + "Keep responses concise and tool-aware.",
            ),
            session_context: Some(
                "Session context ".repeat(40) + "Current workspace contains multiple files.",
            ),
            available_tools: None,
        }))),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
    }
}

fn build_history(session_id: &str) -> Vec<Message> {
    let user_text = "User request context ".repeat(220);
    let user_message = Message::new_user_message(
        session_id.to_string(),
        user_text,
        Some(MessageSource::Ui),
        None,
    );

    let now = chrono::Utc::now().timestamp_millis();
    let assistant_message = Message {
        id: "assistant-anchor".to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![MCPContent::Text {
            text: "Acknowledged. Preparing the next batch of workspace reads.".to_string(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: Some(serde_json::json!({
            "promptTokens": 1700,
            "completionTokens": 220,
            "totalTokens": 1920
        })),
        prompt_tokens: Some(1700),
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::Assistant),
        error: None,
        metadata: None,
    };

    vec![user_message, assistant_message]
}

fn build_tool_call(index: usize) -> ToolCall {
    ToolCall {
        id: format!("call-{index}"),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__readFile".to_string(),
            arguments: serde_json::json!({
                "path": format!("src/features/feature-{index}/component.tsx"),
                "offset": 0,
                "limit": 400
            })
            .to_string(),
        },
    }
}

fn build_assistant_message(session_id: &str, tool_call_count: usize) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    Message {
        id: "assistant-current".to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![MCPContent::Text {
            text: "I will inspect the relevant files in small batches.".to_string(),
            is_error: None,
        }],
        tool_calls: Some((0..tool_call_count).map(build_tool_call).collect()),
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: Some("Need to inspect all related files before editing.".to_string()),
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: Some(serde_json::json!({
            "promptTokens": 1850,
            "completionTokens": 260,
            "totalTokens": 2110
        })),
        prompt_tokens: Some(1850),
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::Assistant),
        error: None,
        metadata: None,
    }
}

async fn ensure_settings_repo() -> SqliteSettingsRepository {
    if let Some(repo) = try_get_settings_repository() {
        return repo.clone();
    }

    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSettingsRepository::new(db);
    set_settings_repository(repo.clone());
    repo
}

async fn upsert_tool_loop_settings() {
    let repo = ensure_settings_repo().await;
    repo.set("contextStrategy", serde_json::json!("compact"))
        .await
        .expect("set context strategy");
    repo.set("maxInputContext", serde_json::json!(2_900))
        .await
        .expect("set max input context");
    repo.set(
        "advancedSettings",
        serde_json::json!({
            "toolResultInlineLimitBytes": 2048
        }),
    )
    .await
    .expect("set advanced settings");
}

async fn register_session(session_id: &str) -> Arc<RwLock<HashMap<String, AgentSession>>> {
    let active_sessions = if let Some(existing) = try_get_active_sessions() {
        existing.clone()
    } else {
        let sessions = Arc::new(RwLock::new(HashMap::new()));
        init_active_sessions(sessions.clone());
        sessions
    };

    active_sessions.write().await.insert(
        session_id.to_string(),
        build_active_session(session_id, build_history(session_id)),
    );

    active_sessions
}

#[tokio::test]
async fn tool_loop_fence_redacts_before_pending_execution_initialization() {
    upsert_tool_loop_settings().await;
    let session_id = "tool-loop-fence-session";
    let active_sessions = register_session(session_id).await;
    let mut assistant_message = build_assistant_message(session_id, 6);
    let original_tool_call_count = assistant_message
        .tool_calls
        .as_ref()
        .expect("tool calls present")
        .len();

    preprocess_assistant_tool_calls_for_testing(
        &active_sessions,
        session_id,
        &mut assistant_message,
    )
    .await;

    let kept_tool_calls = assistant_message
        .tool_calls
        .clone()
        .expect("tool calls should remain after redaction");
    let kept_count = kept_tool_calls.len();

    assert!(
        kept_count < original_tool_call_count,
        "tool-loop fence should redact oversized tool batches: kept={kept_count}, original={original_tool_call_count}"
    );
    assert!(
        kept_count >= 1,
        "tool-loop fence should preserve at least one tool call"
    );

    initialize_pending_execution_for_testing(
        &active_sessions,
        session_id,
        &assistant_message.id,
        &kept_tool_calls,
    )
    .await;

    let active = active_sessions.read().await;
    let session = active.get(session_id).expect("session should exist");
    let pending_execution = session
        .pending_execution
        .as_ref()
        .expect("pending execution should be initialized");

    assert_eq!(
        pending_execution.total_expected, kept_count,
        "pending execution must track only the redacted tool-call prefix"
    );
    assert_eq!(
        pending_execution.expected_tool_call_ids.len(),
        kept_count,
        "expected tool-call ids must match the redacted prefix"
    );
    assert!(
        pending_execution
            .expected_tool_call_ids
            .iter()
            .all(|tool_call_id| kept_tool_calls
                .iter()
                .any(|tool_call| &tool_call.id == tool_call_id)),
        "pending execution should only contain kept tool-call ids"
    );
    assert!(
        !session.cancel_pending.load(Ordering::Relaxed),
        "tool-loop fence should redact, not mark the session as cancelled"
    );
}
