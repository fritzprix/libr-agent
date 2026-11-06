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
    normalize_tool_arguments, CircuitBreakerAction, WaitPreventionSettings,
    TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE,
};
use tauri_mcp_agent_lib::agent::llm::natural_recovery::{
    build_loop_prevention_guidance, LoopPreventionKind, LoopPreventionShortCircuit,
};
use tauri_mcp_agent_lib::agent::llm::preprocess_assistant_tool_calls_for_testing;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::types::ToolCall;
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
        status_reason: None,
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
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        pending_user_prompt: tauri_mcp_agent_lib::agent::state::new_user_prompt_slot(),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::new(),
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

fn mixed_batch_history_three_turns() -> Vec<Message> {
    let mut history = mixed_batch_history();
    history.extend([
        test_message(
            "assistant-3",
            "assistant",
            Some(mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "")),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-3a",
            "tool",
            None,
            Some("tc-3a"),
            None,
            "ok a",
            Some(false),
        ),
        test_message(
            "tool-3b",
            "tool",
            None,
            Some("tc-3b"),
            None,
            "ok b",
            Some(false),
        ),
        test_message(
            "tool-3c",
            "tool",
            None,
            Some("tc-3c"),
            None,
            "ok c",
            Some(false),
        ),
    ]);
    history
}

#[test]
fn mixed_batch_escalates_before_hard_break_when_offset_allows_gap() {
    let messages = mixed_batch_history_three_turns();
    let current = mixed_batch(["tc-4a", "tc-4b", "tc-4c"], "");

    assert_eq!(
        evaluate_batch_circuit_breaker(
            &messages,
            &current,
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        Some(CircuitBreakerAction::RepeatedBatchSequenceEscalate {
            count: 4,
            tool_name: "workspace__readFile".to_string(),
            args: r#"{"path":"a.ts"}"#.to_string(),
        })
    );
}

#[test]
fn mixed_batch_repetition_is_detected_across_turns() {
    let messages = mixed_batch_history();
    let current = mixed_batch(["tc-3a", "tc-3b", "tc-3c"], "");

    assert_eq!(
        evaluate_batch_circuit_breaker(
            &messages,
            &current,
            3,
            1,
            &WaitPreventionSettings::default(),
        ),
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
        evaluate_batch_circuit_breaker(
            &messages,
            &current,
            3,
            1,
            &WaitPreventionSettings::default(),
        ),
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
        evaluate_batch_circuit_breaker(
            &messages,
            &current,
            3,
            1,
            &WaitPreventionSettings::default(),
        ),
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
        evaluate_batch_circuit_breaker(
            &messages,
            &current,
            3,
            1,
            &WaitPreventionSettings::default(),
        ),
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
            duplicate_of_tool_call_id: "tc-1".to_string(),
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
            duplicate_of_tool_call_id: "tc-1".to_string(),
        })
    );
}

#[test]
fn intra_batch_three_identical_calls_all_point_to_first() {
    let args = r#"{"description":"same todo"}"#;
    let tool_calls = vec![
        test_tool_call("tc-1", "planning__addTodo", args),
        test_tool_call("tc-2", "planning__addTodo", args),
        test_tool_call("tc-3", "planning__addTodo", args),
    ];

    let duplicates = find_intra_batch_duplicates(&tool_calls);
    assert!(!duplicates.contains_key("tc-1"));
    assert_eq!(
        duplicates.get("tc-2"),
        Some(&CircuitBreakerAction::DuplicateInBatch {
            tool_name: "planning__addTodo".to_string(),
            args: args.to_string(),
            duplicate_of_tool_call_id: "tc-1".to_string(),
        })
    );
    assert_eq!(
        duplicates.get("tc-3"),
        Some(&CircuitBreakerAction::DuplicateInBatch {
            tool_name: "planning__addTodo".to_string(),
            args: args.to_string(),
            duplicate_of_tool_call_id: "tc-1".to_string(),
        }),
        "third identical call must cite the first occurrence, not the second"
    );
}

#[test]
fn duplicate_in_batch_guidance_cites_origin_and_truncates_long_args() {
    let with_id = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
        kind: LoopPreventionKind::DuplicateInBatch,
        tool_name: "planning__addTodo".to_string(),
        args: r#"{"description":"IMF PPP 추정"}"#.to_string(),
        count: 2,
        duplicate_of_tool_call_id: Some("earlier-call-id".to_string()),
    });
    assert!(with_id.contains("earlier tool_call_id 'earlier-call-id'"));
    assert!(with_id.contains("IMF PPP 추정"));
    assert!(with_id.contains("different arguments are not blocked"));

    let without_id = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
        kind: LoopPreventionKind::DuplicateInBatch,
        tool_name: "planning__addTodo".to_string(),
        args: r#"{"description":"x"}"#.to_string(),
        count: 2,
        duplicate_of_tool_call_id: None,
    });
    assert!(without_id.contains("duplicate of an earlier call"));
    assert!(!without_id.contains("tool_call_id 'an earlier call'"));

    let long_args: String = format!("{{\"description\":\"{}\"}}", "한".repeat(200));
    let truncated = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
        kind: LoopPreventionKind::DuplicateInBatch,
        tool_name: "planning__addTodo".to_string(),
        args: long_args,
        count: 2,
        duplicate_of_tool_call_id: Some("tc-1".to_string()),
    });
    let matched_line = truncated
        .lines()
        .find(|line| line.starts_with("Matched arguments:"))
        .expect("matched arguments line");
    assert!(
        matched_line.ends_with('…'),
        "long args preview must truncate with ellipsis: {matched_line}"
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

#[test]
fn normalize_tool_arguments_is_stable_for_truncated_and_redacted_args() {
    let truncated_1 = r#"{"path":"foo.txt","content":"abcdef"#;
    let truncated_2 = r#"{"path":"foo.txt","content":"abcdefghijklmnop"#;
    assert_eq!(
        normalize_tool_arguments(truncated_1),
        TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE
    );
    assert_eq!(
        normalize_tool_arguments(truncated_2),
        TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE
    );
    assert_eq!(
        normalize_tool_arguments(truncated_1),
        normalize_tool_arguments(truncated_2)
    );

    let redacted_1 = "<redacted: 100 bytes, truncated>";
    let redacted_2 = "<redacted: 5000 bytes, truncated>";
    assert_eq!(
        normalize_tool_arguments(redacted_1),
        TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE
    );
    assert_eq!(
        normalize_tool_arguments(redacted_2),
        TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE
    );
    assert_eq!(
        normalize_tool_arguments(redacted_1),
        normalize_tool_arguments(redacted_2)
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
    assert_eq!(
        dup.duplicate_of_tool_call_id.as_deref(),
        Some("tc-1"),
        "guidance must cite the earlier identical call in the batch"
    );
}

// ---- Wait-aware batch loop prevention (#2610) ----

/// `count` identical 2-call wait batches; the first result of each batch
/// recorded a slow duration (≥ wait threshold), which marks the batch a wait.
fn slow_wait_batch_history(count: usize) -> Vec<Message> {
    let mut messages = Vec::new();
    for i in 0..count {
        let id_a = format!("tc-{i}a");
        let id_b = format!("tc-{i}b");
        messages.push(test_message(
            &format!("assistant-{i}"),
            "assistant",
            Some(vec![
                test_tool_call(
                    &id_a,
                    "workspace__waitForProcess",
                    r#"{"processId":"proc-1","timeout":10}"#,
                ),
                test_tool_call(&id_b, "agent__checkSession", r#"{"sessionId":"s-1"}"#),
            ]),
            None,
            None,
            "",
            None,
        ));
        messages.push(test_message(
            &format!("tool-{i}a"),
            "tool",
            None,
            Some(&id_a),
            Some(serde_json::json!({ "executionDurationMs": 12_000 })),
            "Process proc-1: running",
            Some(false),
        ));
        messages.push(test_message(
            &format!("tool-{i}b"),
            "tool",
            None,
            Some(&id_b),
            None,
            "Session s-1: running",
            Some(false),
        ));
    }
    messages
}

fn current_wait_batch(suffix: &str) -> Vec<ToolCall> {
    vec![
        test_tool_call(
            &format!("tc-cur{suffix}a"),
            "workspace__waitForProcess",
            r#"{"processId":"proc-1","timeout":10}"#,
        ),
        test_tool_call(
            &format!("tc-cur{suffix}b"),
            "agent__checkSession",
            r#"{"sessionId":"s-1"}"#,
        ),
    ]
}

#[test]
fn identical_slow_wait_batches_skip_fast_rule_below_guide_count() {
    // 5 slow batches + current = 6th: the fast rule (threshold 3, hard at 5)
    // would have hard-broken; the wait ladder passes instead.
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &slow_wait_batch_history(5),
            &current_wait_batch(""),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        None,
        "identical slow wait batches must skip the fast-loop ladder"
    );
}

#[test]
fn identical_slow_wait_batches_guide_at_eleventh_and_break_at_twenty_first() {
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &slow_wait_batch_history(10),
            &current_wait_batch("11"),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        Some(CircuitBreakerAction::WaitGuidance {
            count: 11,
            tool_name: "workspace__waitForProcess".to_string(),
            args: r#"{"processId":"proc-1","timeout":10}"#.to_string(),
        }),
        "the 11th identical slow wait batch must guide to a scheduled callback"
    );

    assert_eq!(
        evaluate_batch_circuit_breaker(
            &slow_wait_batch_history(20),
            &current_wait_batch("21"),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        Some(CircuitBreakerAction::HardBreak {
            count: 21,
            tool_name: "workspace__waitForProcess".to_string(),
            args: r#"{"processId":"proc-1","timeout":10}"#.to_string(),
        }),
        "continued guided wait batches must hard-break at the 21st"
    );
}

#[test]
fn wait_batch_hard_break_reached_for_large_non_default_counts() {
    // S1.2: guide 40 + break 30 → hard break at the 71st identical wait batch.
    // With the old fixed 32-batch scan cap the streak under-counts to 33 and
    // the ladder never trips; the scanner must scan far enough back.
    let wait = WaitPreventionSettings {
        wait_duration_ms: 10_000,
        guide_count: 40,
        break_count: 30,
    };
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &slow_wait_batch_history(70),
            &current_wait_batch("71"),
            3,
            2,
            &wait,
        ),
        Some(CircuitBreakerAction::HardBreak {
            count: 71,
            tool_name: "workspace__waitForProcess".to_string(),
            args: r#"{"processId":"proc-1","timeout":10}"#.to_string(),
        }),
        "the batch scanner must reach the hard break for non-default guide/break counts"
    );
}

#[tokio::test]
async fn preprocess_mixed_wait_batch_owned_by_batch_ladder_across_turns() {
    // G1.2: a mixed batch (slow `waitForProcess` + fast `checkSession`) repeated
    // across turns must be owned entirely by the batch wait ladder. The fast
    // member must not be fast-blocked per-tool on turns 3..=10 (the old
    // per-tool/batch disagreement), and the 11th batch guides both members.
    let session_id = "mixed-wait-batch-ladder-session";
    let active_sessions = ensure_active_sessions();
    let mut history: Vec<Message> = Vec::new();

    for turn in 0..=10 {
        {
            let mut sessions = active_sessions.write().await;
            sessions.insert(
                session_id.to_string(),
                build_active_session(session_id, history.clone()),
            );
        }

        let current = current_wait_batch(&format!("-{turn}"));
        let mut assistant_message = test_message(
            &format!("assistant-{turn}"),
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

        if turn < 10 {
            assert!(
                preprocess_result.loop_prevention_short_circuits.is_empty(),
                "turn {}: repeated wait-batch members must not be fast-blocked per-tool",
                turn + 1,
            );

            // Feed this turn's results back: one slow wait + one fast identical poll.
            history.push(assistant_message);
            history.push(test_message(
                &format!("tool-{turn}a"),
                "tool",
                None,
                Some(&format!("tc-cur-{turn}a")),
                Some(serde_json::json!({ "executionDurationMs": 12_000 })),
                "Process proc-1: running",
                Some(false),
            ));
            history.push(test_message(
                &format!("tool-{turn}b"),
                "tool",
                None,
                Some(&format!("tc-cur-{turn}b")),
                None,
                "Session s-1: running",
                Some(false),
            ));
        } else {
            assert_eq!(
                preprocess_result.loop_prevention_short_circuits.len(),
                2,
                "the 11th identical wait batch must short-circuit both members",
            );
            // Batch guidance cites the batch's first call as representative
            // for every member (same as RepeatedBatchSequence guidance).
            for tool_call in &current {
                let short_circuit = preprocess_result
                    .loop_prevention_short_circuits
                    .get(&tool_call.id)
                    .unwrap_or_else(|| panic!("{} must be short-circuited", tool_call.id));
                assert_eq!(short_circuit.kind, LoopPreventionKind::WaitGuidance);
                assert_eq!(short_circuit.tool_name, "workspace__waitForProcess");
                assert_eq!(short_circuit.count, 11);
            }
        }
    }
}

// ---- Duration-mix overall cap for batches (#2610 G3.1) ----

/// `count` identical 2-call batches whose first (wait) member alternates
/// duration: batch `i` (oldest first) is slow when `slow(i)`.
fn duration_mix_batch_history(count: usize, slow: impl Fn(usize) -> bool) -> Vec<Message> {
    let mut messages = Vec::new();
    for i in 0..count {
        let id_a = format!("tc-mix-{i}a");
        let id_b = format!("tc-mix-{i}b");
        let duration = if slow(i) { 12_000 } else { 50 };
        messages.push(test_message(
            &format!("assistant-mix-{i}"),
            "assistant",
            Some(vec![
                test_tool_call(
                    &id_a,
                    "workspace__waitForProcess",
                    r#"{"processId":"proc-1","timeout":10}"#,
                ),
                test_tool_call(&id_b, "agent__checkSession", r#"{"sessionId":"s-1"}"#),
            ]),
            None,
            None,
            "",
            None,
        ));
        messages.push(test_message(
            &format!("tool-mix-{i}a"),
            "tool",
            None,
            Some(&id_a),
            Some(serde_json::json!({ "executionDurationMs": duration })),
            "Process proc-1: running",
            Some(false),
        ));
        messages.push(test_message(
            &format!("tool-mix-{i}b"),
            "tool",
            None,
            Some(&id_b),
            None,
            "Session s-1: running",
            Some(false),
        ));
    }
    messages
}

#[test]
fn alternating_slow_fast_identical_batches_are_bounded_by_overall_cap() {
    // G3.1 for batches: only durations alternate — fingerprint and outcome are
    // identical across every batch, so each ladder only ever sees a 1-batch
    // trailing run. The overall streak must still be bounded.
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &duration_mix_batch_history(19, |i| i % 2 == 0),
            &current_wait_batch("20"),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        None,
        "the 20th identical batch (guide 10 + break 10) must not trip yet"
    );
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &duration_mix_batch_history(20, |i| i % 2 == 0),
            &current_wait_batch("21"),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        Some(CircuitBreakerAction::HardBreak {
            count: 21,
            tool_name: "workspace__waitForProcess".to_string(),
            args: r#"{"processId":"proc-1","timeout":10}"#.to_string(),
        }),
        "the 21st identical batch must hard-break regardless of duration mix"
    );
}

// ---- Overall cap never overrides the fast batch ladder (#2610 G4.1) ----

const WAIT_BATCH_TOOL: &str = "workspace__waitForProcess";
const WAIT_BATCH_ARGS: &str = r#"{"processId":"proc-1","timeout":10}"#;

fn fast_batch_action(
    history: usize,
    threshold: usize,
    hard_break_offset: usize,
    wait: &WaitPreventionSettings,
) -> Option<CircuitBreakerAction> {
    evaluate_batch_circuit_breaker(
        &duration_mix_batch_history(history, |_| false),
        &current_wait_batch(&(history + 1).to_string()),
        threshold,
        hard_break_offset,
        wait,
    )
}

fn batch_sequence(count: usize) -> Option<CircuitBreakerAction> {
    Some(CircuitBreakerAction::RepeatedBatchSequence {
        count,
        tool_name: WAIT_BATCH_TOOL.to_string(),
        args: WAIT_BATCH_ARGS.to_string(),
    })
}

fn batch_escalate(count: usize) -> Option<CircuitBreakerAction> {
    Some(CircuitBreakerAction::RepeatedBatchSequenceEscalate {
        count,
        tool_name: WAIT_BATCH_TOOL.to_string(),
        args: WAIT_BATCH_ARGS.to_string(),
    })
}

fn batch_hard_break(count: usize) -> Option<CircuitBreakerAction> {
    Some(CircuitBreakerAction::HardBreak {
        count,
        tool_name: WAIT_BATCH_TOOL.to_string(),
        args: WAIT_BATCH_ARGS.to_string(),
    })
}

#[test]
fn pure_fast_batches_keep_configured_hard_break_above_wait_cap() {
    // G4.1: threshold 20 + offset 20 configures the batch hard break at 40;
    // the overall cap (guide 10 + break 10) must not cut it to 21.
    let wait = WaitPreventionSettings::default();
    assert_eq!(fast_batch_action(20, 20, 20, &wait), batch_sequence(21));
    assert_eq!(fast_batch_action(38, 20, 20, &wait), batch_escalate(39));
    assert_eq!(fast_batch_action(39, 20, 20, &wait), batch_hard_break(40));
}

#[test]
fn pure_fast_batches_ignore_small_wait_counts() {
    // G4.1: guide 1 + break 1 must not change fast batch behavior.
    let wait = WaitPreventionSettings {
        guide_count: 1,
        break_count: 1,
        ..WaitPreventionSettings::default()
    };
    assert_eq!(fast_batch_action(2, 3, 2, &wait), batch_sequence(3));
    assert_eq!(fast_batch_action(3, 3, 2, &wait), batch_escalate(4));
    assert_eq!(fast_batch_action(4, 3, 2, &wait), batch_hard_break(5));
}

#[test]
fn nine_slow_one_fast_identical_batches_are_bounded_by_overall_cap() {
    // G3.1: a 9-slow/1-fast batch cycle restarts the wait ladder every 10
    // batches while the newest batch is fast.
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &duration_mix_batch_history(10, |i| i % 10 != 9),
            &current_wait_batch("11"),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        None,
        "the 11th identical batch stays below the overall cap"
    );
    assert_eq!(
        evaluate_batch_circuit_breaker(
            &duration_mix_batch_history(20, |i| i % 10 != 9),
            &current_wait_batch("21"),
            3,
            2,
            &WaitPreventionSettings::default(),
        ),
        Some(CircuitBreakerAction::HardBreak {
            count: 21,
            tool_name: "workspace__waitForProcess".to_string(),
            args: r#"{"processId":"proc-1","timeout":10}"#.to_string(),
        }),
        "the 21st identical batch must hard-break even with a repeating duration mix"
    );
}

#[tokio::test]
async fn preprocess_changing_slow_batch_still_blocks_its_fast_member() {
    // G3.2: the slow member's result progresses every turn, so the batch never
    // repeats an identical outcome (previous = 1) and never forms a wait streak.
    // The batch ladder must therefore NOT own the members: the byte-identical
    // `readFile` error in the same batch has to reach its own per-tool soft
    // ladder at count 3. Before the fix the gate treated any single slow batch
    // as a wait streak and skipped the per-tool scan, which looped forever.
    let session_id = "changing-slow-batch-session";
    let active_sessions = ensure_active_sessions();
    let mut history: Vec<Message> = Vec::new();
    let wait_args = r#"{"processId":"proc-1","timeout":10}"#;
    let read_args = r#"{"path":"src/main.ts"}"#;

    for turn in 0..3 {
        {
            let mut sessions = active_sessions.write().await;
            sessions.insert(
                session_id.to_string(),
                build_active_session(session_id, history.clone()),
            );
        }

        let wait_id = format!("tc-{turn}a");
        let read_id = format!("tc-{turn}b");
        let current = vec![
            test_tool_call(&wait_id, "workspace__waitForProcess", wait_args),
            test_tool_call(&read_id, "workspace__readFile", read_args),
        ];
        let mut assistant_message = test_message(
            &format!("assistant-{turn}"),
            "assistant",
            Some(current),
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

        assert!(
            preprocess_result.forced_stop.is_none(),
            "turn {}: a changing slow batch must never hard-break",
            turn + 1
        );

        if turn < 2 {
            assert!(
                preprocess_result.loop_prevention_short_circuits.is_empty(),
                "turn {}: identical readFile errors must stay below the soft threshold",
                turn + 1
            );
        } else {
            assert_eq!(
                preprocess_result.loop_prevention_short_circuits.len(),
                1,
                "only the repeatedly failing fast member is short-circuited"
            );
            assert!(
                !preprocess_result
                    .loop_prevention_short_circuits
                    .contains_key(&wait_id),
                "the slow member whose result progresses must keep running"
            );
            let short_circuit = preprocess_result
                .loop_prevention_short_circuits
                .get(&read_id)
                .expect("the 3rd identical readFile error is soft-blocked");
            assert_eq!(short_circuit.kind, LoopPreventionKind::RepeatedErrorOutcome);
            assert_eq!(short_circuit.count, 3);
        }

        history.push(assistant_message);
        history.push(test_message(
            &format!("tool-{turn}a"),
            "tool",
            None,
            Some(&wait_id),
            Some(serde_json::json!({ "executionDurationMs": 12_000 })),
            &format!("Process proc-1: running ({}s elapsed)", turn),
            Some(false),
        ));
        history.push(test_message(
            &format!("tool-{turn}b"),
            "tool",
            None,
            Some(&read_id),
            None,
            "Error: file not found",
            Some(true),
        ));
    }
}

#[tokio::test]
async fn preprocess_fast_tail_wait_batch_does_not_inherit_owned_turns() {
    // G3.3: 8 identical slow wait batches (all owned by the batch ladder, and
    // guided once past the guide count) followed by 1 identical FAST batch.
    // The newest batch being fast puts the batch wait ladder out of play
    // (slow_suffix = 0), so the per-tool scan runs again — it must not inherit
    // the owned batches as fast attempts for the fast member. Before the fix
    // the fast member saw 9 fast attempts and hard-broke with no warning; now
    // every owned turn is skipped and nothing fires.
    let session_id = "fast-tail-wait-batch-session";
    let active_sessions = ensure_active_sessions();
    let history = duration_mix_batch_history(9, |i| i < 8);
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, history),
        );
    }

    let current = current_wait_batch("-tail");
    let mut assistant_message = test_message(
        "assistant-tail",
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

    assert!(
        preprocess_result.forced_stop.is_none(),
        "owned wait batches must not be re-counted as per-tool fast attempts"
    );
    assert!(
        !preprocess_result
            .loop_prevention_short_circuits
            .contains_key(&current[0].id),
        "the slow member keeps its own wait ladder (count 3, below the guide count)"
    );
    let short_circuit = preprocess_result
        .loop_prevention_short_circuits
        .get(&current[1].id)
        .expect("the fast member is soft-blocked, not hard-broken");
    assert_eq!(
        short_circuit.kind,
        LoopPreventionKind::RepeatedSuccessOutcome
    );
    assert_eq!(
        short_circuit.count, 3,
        "only the unowned batches 1 and 0 plus the current one count as fast attempts"
    );
}
