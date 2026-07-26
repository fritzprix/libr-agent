//! Windows-safe circuit-breaker batch-loop coverage (including outcome-aware
//! batch streaks).
//!
//! The consolidated `integration_tests` binary is `#![cfg(not(windows))]` because
//! it links the full Tauri/WebView path. These cases stay runnable on Windows.

#[path = "common/circuit_breaker_fixtures.rs"]
mod circuit_breaker_fixtures;

use circuit_breaker_fixtures::{mixed_batch, test_message, test_tool_call};
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::llm::circuit_breaker::{
    batch_fingerprint, evaluate_batch_circuit_breaker, find_intra_batch_duplicates,
    normalize_tool_arguments, CircuitBreakerAction,
};
use tauri_mcp_agent_lib::agent::llm::natural_recovery::LoopPreventionKind;
use tauri_mcp_agent_lib::agent::llm::preprocess_assistant_tool_calls_for_testing;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::{init_active_sessions, try_get_active_sessions};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("batch-loop".to_string()),
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
        yolo_mode: Arc::new(AtomicBool::new(false)),
        unsafe_mode: Arc::new(AtomicBool::new(true)),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(messages)),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
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

fn ensure_active_sessions() -> Arc<RwLock<HashMap<String, AgentSession>>> {
    if let Some(existing) = try_get_active_sessions() {
        return existing.clone();
    }

    let sessions = Arc::new(RwLock::new(HashMap::new()));
    init_active_sessions(sessions.clone());
    sessions
}

fn mixed_batch_history() -> Vec<Message> {
    vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(mixed_batch(["tc-1a", "tc-1b", "tc-1c"], "")),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1a",
            "tool",
            None,
            Some("tc-1a"),
            None,
            "ok a",
            Some(false),
        ),
        test_message(
            "tool-1b",
            "tool",
            None,
            Some("tc-1b"),
            None,
            "ok b",
            Some(false),
        ),
        test_message(
            "tool-1c",
            "tool",
            None,
            Some("tc-1c"),
            None,
            "ok c",
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(mixed_batch(["tc-2a", "tc-2b", "tc-2c"], "")),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2a",
            "tool",
            None,
            Some("tc-2a"),
            None,
            "ok a",
            Some(false),
        ),
        test_message(
            "tool-2b",
            "tool",
            None,
            Some("tc-2b"),
            None,
            "ok b",
            Some(false),
        ),
        test_message(
            "tool-2c",
            "tool",
            None,
            Some("tc-2c"),
            None,
            "ok c",
            Some(false),
        ),
    ]
}

#[test]
fn mixed_batch_repetition_is_detected_across_turns() {
    let messages = mixed_batch_history();
    let current = mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "");

    assert_eq!(
        evaluate_batch_circuit_breaker(&messages, &current, 3, 1),
        Some(CircuitBreakerAction::RepeatedBatchSequence {
            count: 3,
            tool_name: "workspace__readFile".to_string(),
            args: r#"{"path":"a.ts"}"#.to_string(),
        })
    );
}

#[test]
fn mixed_batch_different_outcomes_do_not_accumulate_toward_threshold() {
    // Same batch fingerprint with progressing tool results only counts the
    // trailing identical-outcome segment.
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(mixed_batch(["tc-1a", "tc-1b", "tc-1c"], "")),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1a",
            "tool",
            None,
            Some("tc-1a"),
            None,
            "running a",
            Some(false),
        ),
        test_message(
            "tool-1b",
            "tool",
            None,
            Some("tc-1b"),
            None,
            "running b",
            Some(false),
        ),
        test_message(
            "tool-1c",
            "tool",
            None,
            Some("tc-1c"),
            None,
            "running c",
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(mixed_batch(["tc-2a", "tc-2b", "tc-2c"], "")),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2a",
            "tool",
            None,
            Some("tc-2a"),
            None,
            "done a",
            Some(false),
        ),
        test_message(
            "tool-2b",
            "tool",
            None,
            Some("tc-2b"),
            None,
            "done b",
            Some(false),
        ),
        test_message(
            "tool-2c",
            "tool",
            None,
            Some("tc-2c"),
            None,
            "done c",
            Some(false),
        ),
    ];
    let current = mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "");

    assert_eq!(
        evaluate_batch_circuit_breaker(&messages, &current, 3, 1),
        None,
        "changing batch outcomes must not Soft-block the next identical batch"
    );
}

#[test]
fn mixed_batch_hard_breaks_after_threshold_plus_offset() {
    let mut messages = mixed_batch_history();
    messages.push(test_message(
        "assistant-3",
        "assistant",
        Some(mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "")),
        None,
        None,
        "",
        None,
    ));
    messages.push(test_message(
        "tool-3a",
        "tool",
        None,
        Some("tc-3a"),
        None,
        "ok",
        Some(false),
    ));
    let current = mixed_batch(["tc-4a", "tc-4b", "tc-4c"], "");

    assert_eq!(
        evaluate_batch_circuit_breaker(&messages, &current, 3, 1),
        Some(CircuitBreakerAction::HardBreak {
            count: 4,
            tool_name: "workspace__readFile".to_string(),
            args: r#"{"path":"a.ts"}"#.to_string(),
        })
    );
}

#[test]
fn user_message_resets_batch_streak() {
    let mut messages = mixed_batch_history();
    messages.push(test_message(
        "user-1",
        "user",
        None,
        None,
        None,
        "please continue",
        None,
    ));
    messages.push(test_message(
        "assistant-3",
        "assistant",
        Some(mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "")),
        None,
        None,
        "",
        None,
    ));
    messages.push(test_message(
        "tool-3a",
        "tool",
        None,
        Some("tc-3a"),
        None,
        "ok",
        Some(false),
    ));

    let current = mixed_batch(["tc-4a", "tc-4b", "tc-4c"], "");
    // After a user turn, only one prior matching batch remains consecutive.
    assert_eq!(
        evaluate_batch_circuit_breaker(&messages, &current, 3, 1),
        None
    );
}

#[test]
fn intra_batch_duplicate_signatures_are_detected() {
    let tool_calls = vec![
        test_tool_call("tc-1", "workspace__readFile", r#"{"path":"a.ts"}"#),
        test_tool_call("tc-2", "workspace__listDirectory", r#"{"path":"."}"#),
        test_tool_call("tc-3", "workspace__readFile", r#"{"path":"a.ts"}"#),
    ];

    let duplicates = find_intra_batch_duplicates(&tool_calls);
    assert!(!duplicates.contains_key("tc-1"));
    assert!(!duplicates.contains_key("tc-2"));
    assert_eq!(
        duplicates.get("tc-3"),
        Some(&CircuitBreakerAction::DuplicateInBatch {
            tool_name: "workspace__readFile".to_string(),
            args: r#"{"path":"a.ts"}"#.to_string(),
        })
    );
}

#[test]
fn intra_batch_duplicate_ignores_json_key_order() {
    let tool_calls = vec![
        test_tool_call(
            "tc-1",
            "workspace__readFile",
            r#"{"path":"a.ts","offset":1}"#,
        ),
        test_tool_call(
            "tc-2",
            "workspace__readFile",
            r#"{"offset":1,"path":"a.ts"}"#,
        ),
    ];

    let duplicates = find_intra_batch_duplicates(&tool_calls);
    assert_eq!(
        duplicates.get("tc-2"),
        Some(&CircuitBreakerAction::DuplicateInBatch {
            tool_name: "workspace__readFile".to_string(),
            args: r#"{"offset":1,"path":"a.ts"}"#.to_string(),
        })
    );
}

#[tokio::test]
async fn preprocess_short_circuits_whole_repeated_batch() {
    let session_id = "batch-loop-preprocess-session";
    let history = mixed_batch_history();
    let active_sessions = ensure_active_sessions();
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, history),
        );
    }

    let current = mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "");
    let mut assistant_message = test_message(
        "assistant-3",
        "assistant",
        Some(current.clone()),
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

    assert!(preprocess_result.forced_stop.is_none());
    assert_eq!(preprocess_result.loop_prevention_short_circuits.len(), 3);
    for tool_call in &current {
        let short_circuit = preprocess_result
            .loop_prevention_short_circuits
            .get(&tool_call.id)
            .expect("each identical-batch call is short-circuited");
        assert_eq!(
            short_circuit.kind,
            LoopPreventionKind::RepeatedBatchSequence
        );
        assert_eq!(short_circuit.count, 3);
    }

    // Tool call list is preserved (execution layer applies short-circuits).
    assert_eq!(
        assistant_message
            .tool_calls
            .as_ref()
            .map(|calls| calls.len()),
        Some(3)
    );
}

#[test]
fn batch_fingerprint_hashes_when_over_size_cap() {
    // Build a batch whose joined signatures exceed MAX_BATCH_FINGERPRINT_BYTES.
    let huge_args = format!(r#"{{"blob":"{}"}}"#, "x".repeat(40_000));
    let tool_calls: Vec<_> = (0..3)
        .map(|i| test_tool_call(&format!("tc-{i}"), "workspace__readFile", &huge_args))
        .collect();

    let fingerprint = batch_fingerprint(&tool_calls);
    assert!(
        fingerprint.starts_with("hashed:"),
        "oversized fingerprints should collapse to a hash: {fingerprint}"
    );
    assert_eq!(
        batch_fingerprint(&tool_calls),
        fingerprint,
        "identical oversized batches must still fingerprint-equal"
    );
}

#[test]
fn normalize_tool_arguments_is_stable_for_nested_objects() {
    let left = r#"{"a":{"z":1,"y":2},"b":[3,4]}"#;
    let right = r#"{"b":[3,4],"a":{"y":2,"z":1}}"#;
    assert_eq!(
        normalize_tool_arguments(left),
        normalize_tool_arguments(right)
    );
}

#[tokio::test]
async fn preprocess_only_short_circuits_intra_batch_duplicates() {
    let session_id = "intra-batch-preprocess-session";
    let active_sessions = ensure_active_sessions();
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, Vec::new()),
        );
    }

    let mut assistant_message = test_message(
        "assistant-1",
        "assistant",
        Some(vec![
            test_tool_call("tc-1", "workspace__readFile", r#"{"path":"a.ts"}"#),
            test_tool_call("tc-2", "workspace__listDirectory", r#"{"path":"."}"#),
            test_tool_call("tc-3", "workspace__readFile", r#"{"path":"a.ts"}"#),
        ]),
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

    assert!(preprocess_result.forced_stop.is_none());
    assert!(
        !preprocess_result
            .loop_prevention_short_circuits
            .contains_key("tc-1"),
        "first unique signature must still execute"
    );
    assert!(
        !preprocess_result
            .loop_prevention_short_circuits
            .contains_key("tc-2"),
        "non-duplicate sibling must still execute"
    );
    let dup = preprocess_result
        .loop_prevention_short_circuits
        .get("tc-3")
        .expect("duplicate signature is short-circuited");
    assert_eq!(dup.kind, LoopPreventionKind::DuplicateInBatch);
}
