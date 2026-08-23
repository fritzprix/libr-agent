//! Windows-safe integration coverage for tool-loop resample policy.
//!
//! Settings-mutation cases (`legacy`, `max_retries=0`) run only on non-Windows
//! targets because the full DB/migration harness hits STATUS_ENTRYPOINT_NOT_FOUND
//! in this test binary on Windows.

use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc};

use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::llm::natural_recovery::LoopPreventionKind;
use tauri_mcp_agent_lib::agent::llm::{
    preprocess_assistant_tool_calls_for_testing, CircuitBreakerPreprocessResult,
};
use tauri_mcp_agent_lib::agent::state::{AgentSession, PendingEventManager};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::{init_active_sessions, try_get_active_sessions};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

static TEST_GUARD: Mutex<()> = Mutex::const_new(());

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
        session_id: "loop-prevention-resample-session".to_string(),
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
        name: Some("tool-loop-resample".to_string()),
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
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: tauri_mcp_agent_lib::agent::state::CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
        tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
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

async fn run_preprocess(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    history: Vec<Message>,
    current_tool_calls: Vec<ToolCall>,
) -> (CircuitBreakerPreprocessResult, Message) {
    {
        let mut sessions = active_sessions.write().await;
        sessions.insert(
            session_id.to_string(),
            build_active_session(session_id, history),
        );
    }

    let mut assistant_message = test_message(
        "assistant-current",
        "assistant",
        Some(current_tool_calls),
        None,
        None,
        "",
        None,
    );

    let preprocess_result = preprocess_assistant_tool_calls_for_testing(
        active_sessions,
        session_id,
        &mut assistant_message,
    )
    .await;

    (preprocess_result, assistant_message)
}

#[tokio::test]
async fn repeated_error_outcome_triggers_resample_with_default_settings() {
    let _guard = TEST_GUARD.lock().await;

    let session_id = "tool-loop-resample-error-session";
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_error = "Error: file not found";

    let history = vec![
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
    ];

    let active_sessions = ensure_active_sessions();
    let (preprocess_result, _) = run_preprocess(
        &active_sessions,
        session_id,
        history,
        vec![test_tool_call("tc-3", "workspace__readFile", repeated_args)],
    )
    .await;

    assert!(preprocess_result.forced_stop.is_none());
    let decision = preprocess_result
        .tool_loop_resample
        .as_ref()
        .expect("expected tool_loop_resample decision");
    assert_eq!(decision.count, 3);
    assert_eq!(decision.attempt_index, 0);
    assert!(matches!(
        decision.kind,
        LoopPreventionKind::RepeatedErrorOutcome | LoopPreventionKind::RepeatedErrorEscalate
    ));
}

#[tokio::test]
async fn repeated_success_outcome_triggers_resample_decision() {
    let _guard = TEST_GUARD.lock().await;

    let session_id = "tool-loop-resample-success-session";
    let repeated_args = r#"{"processId":"abc","timeout":0}"#;
    let status = "Process abc: running";

    let history = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__waitForProcess",
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
            status,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__waitForProcess",
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
            status,
            Some(false),
        ),
    ];

    let active_sessions = ensure_active_sessions();
    let (preprocess_result, _) = run_preprocess(
        &active_sessions,
        session_id,
        history,
        vec![test_tool_call(
            "tc-3",
            "workspace__waitForProcess",
            repeated_args,
        )],
    )
    .await;

    let decision = preprocess_result
        .tool_loop_resample
        .as_ref()
        .expect("expected resample for repeated success outcome");
    assert_eq!(decision.count, 3);
    assert!(matches!(
        decision.kind,
        LoopPreventionKind::RepeatedSuccessOutcome
    ));
}

#[tokio::test]
async fn repeated_batch_sequence_triggers_resample_decision() {
    let _guard = TEST_GUARD.lock().await;

    let session_id = "tool-loop-resample-batch-session";
    let read_args = r#"{"path":"a.ts"}"#;
    let list_args = r#"{"path":"."}"#;
    let search_args = r#"{"query":"foo"}"#;

    let batch = |ids: [&str; 3]| {
        vec![
            test_tool_call(ids[0], "workspace__readFile", read_args),
            test_tool_call(ids[1], "workspace__listDirectory", list_args),
            test_tool_call(ids[2], "knowledge__search", search_args),
        ]
    };

    let history = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(batch(["tc-1a", "tc-1b", "tc-1c"])),
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
            Some(batch(["tc-2a", "tc-2b", "tc-2c"])),
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
    ];

    let active_sessions = ensure_active_sessions();
    let (preprocess_result, _) = run_preprocess(
        &active_sessions,
        session_id,
        history,
        batch(["tc-3a", "tc-3b", "tc-3c"]),
    )
    .await;

    let decision = preprocess_result
        .tool_loop_resample
        .as_ref()
        .expect("expected resample for repeated batch sequence");
    assert_eq!(decision.count, 3);
    assert!(matches!(
        decision.kind,
        LoopPreventionKind::RepeatedBatchSequence
    ));
}

#[tokio::test]
async fn duplicate_in_batch_does_not_trigger_resample() {
    let _guard = TEST_GUARD.lock().await;

    let session_id = "tool-loop-resample-duplicate-session";
    let read_args = r#"{"path":"a.ts"}"#;

    let active_sessions = ensure_active_sessions();
    let (preprocess_result, _) = run_preprocess(
        &active_sessions,
        session_id,
        Vec::new(),
        vec![
            test_tool_call("tc-1", "workspace__readFile", read_args),
            test_tool_call("tc-2", "workspace__listDirectory", r#"{"path":"."}"#),
            test_tool_call("tc-3", "workspace__readFile", read_args),
        ],
    )
    .await;

    assert!(
        preprocess_result.tool_loop_resample.is_none(),
        "duplicate in batch uses legacy short-circuit only"
    );
    let dup = preprocess_result
        .loop_prevention_short_circuits
        .get("tc-3")
        .expect("duplicate call short-circuited");
    assert_eq!(dup.kind, LoopPreventionKind::DuplicateInBatch);
}

#[tokio::test]
async fn default_resample_budget_exhaustion_promotes_to_hard_break() {
    let _guard = TEST_GUARD.lock().await;

    let session_id = "tool-loop-resample-hard-break-session";
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_error = "Error: file not found";

    let history_with_4 = vec![
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
        test_message(
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
        ),
        test_message(
            "tool-3",
            "tool",
            None,
            Some("tc-3"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
        test_message(
            "assistant-4",
            "assistant",
            Some(vec![test_tool_call(
                "tc-4",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-4",
            "tool",
            None,
            Some("tc-4"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
    ];

    let active_sessions = ensure_active_sessions();
    let (preprocess_result, _) = run_preprocess(
        &active_sessions,
        session_id,
        history_with_4,
        vec![test_tool_call("tc-5", "workspace__readFile", repeated_args)],
    )
    .await;

    assert!(
        preprocess_result.forced_stop.is_some(),
        "expected hard break once resample budget is exhausted"
    );
    assert!(preprocess_result.tool_loop_resample.is_none());
}

#[tokio::test]
async fn session_resample_budget_blocks_resample_when_history_count_stays_at_threshold() {
    let _guard = TEST_GUARD.lock().await;

    let session_id = "tool-loop-resample-session-budget-session";
    let repeated_args = r#"{"processId":"abc","timeout":0}"#;
    let repeated_status = "Process abc: running";

    let history = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__waitForProcess",
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
            repeated_status,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__waitForProcess",
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
            repeated_status,
            Some(false),
        ),
    ];

    let budget_key = format!(
        "workspace__waitForProcess:{}",
        tauri_mcp_agent_lib::agent::llm::circuit_breaker::normalize_tool_arguments(repeated_args)
    );

    let active_sessions = ensure_active_sessions();
    {
        let mut sessions = active_sessions.write().await;
        let session = build_active_session(session_id, history.clone());
        session
            .tool_loop_resample_attempts
            .write()
            .await
            .insert(budget_key, 2);
        sessions.insert(session_id.to_string(), session);
    }

    let mut assistant_message = test_message(
        "assistant-current",
        "assistant",
        Some(vec![test_tool_call(
            "tc-3",
            "workspace__waitForProcess",
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

    assert!(
        preprocess_result.tool_loop_resample.is_none(),
        "session resample budget must block resample even when history count stays at threshold"
    );
    assert!(
        !preprocess_result.loop_prevention_short_circuits.is_empty(),
        "expected loop-prevention short-circuit once resample budget is exhausted"
    );
}

#[cfg(not(windows))]
mod settings_mutation_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
    use sea_orm_migration::MigratorTrait;
    use tauri_mcp_agent_lib::migration::Migrator;
    use tauri_mcp_agent_lib::repositories::{SettingsRepository, SqliteSettingsRepository};
    use tauri_mcp_agent_lib::{reset_state, set_settings_repository};

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Fresh migrated DB per call.
    ///
    /// Do not reuse a OnceCell-backed in-memory pool across tests: sqlx may close the
    /// idle connection and SQLite then destroys the shared-memory database, leaving
    /// later tests with `no such table: settings`.
    async fn install_experimental_settings(experimental: serde_json::Value) {
        reset_state();
        tauri_mcp_agent_lib::lifecycle::database::register_sqlite_vec();

        let db_id = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let database_url =
            format!("sqlite::file:tool_loop_resample_{db_id}?mode=memory&cache=shared");
        let mut opt = sea_orm::ConnectOptions::new(database_url);
        opt.max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(opt)
            .await
            .expect("Failed to create in-memory database");
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = ON".to_string(),
        ))
        .await
        .expect("Failed to enable SQLite foreign_keys");
        Migrator::up(&db, None)
            .await
            .expect("Migrations should run");

        let repo = SqliteSettingsRepository::new(db);
        set_settings_repository(repo.clone());
        repo.set("experimentalSettings", experimental)
            .await
            .expect("set experimentalSettings");
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
    async fn legacy_guidance_enabled_skips_resample_decision() {
        let _guard = TEST_GUARD.lock().await;
        install_experimental_settings(serde_json::json!({
            "toolLoopRecoveryPolicy": "legacyGuidance",
            "toolLoopMaxResampleRetries": 2,
            "inlineAudioAttachment": true
        }))
        .await;

        let session_id = "tool-loop-legacy-guidance-session";
        let repeated_args = r#"{"path":"src/main.ts"}"#;
        let repeated_error = "Error: file not found";

        let active_sessions = ensure_active_sessions();
        let (preprocess_result, _) = run_preprocess(
            &active_sessions,
            session_id,
            repeated_error_history(repeated_args, repeated_error),
            vec![test_tool_call("tc-3", "workspace__readFile", repeated_args)],
        )
        .await;

        assert!(preprocess_result.tool_loop_resample.is_none());
        assert!(!preprocess_result.loop_prevention_short_circuits.is_empty());

        // Drop global settings so later default-policy tests are not poisoned.
        reset_state();
    }

    #[tokio::test]
    async fn legacy_boolean_key_still_enables_legacy_guidance() {
        let _guard = TEST_GUARD.lock().await;
        // Read-compat for pre-canonicalize DB blobs.
        install_experimental_settings(serde_json::json!({
            "toolLoopLegacyGuidanceEnabled": true,
            "toolLoopMaxResampleRetries": 2,
            "inlineAudioAttachment": true
        }))
        .await;

        let session_id = "tool-loop-legacy-boolean-compat-session";
        let repeated_args = r#"{"path":"src/main.ts"}"#;
        let repeated_error = "Error: file not found";

        let active_sessions = ensure_active_sessions();
        let (preprocess_result, _) = run_preprocess(
            &active_sessions,
            session_id,
            repeated_error_history(repeated_args, repeated_error),
            vec![test_tool_call("tc-3", "workspace__readFile", repeated_args)],
        )
        .await;

        assert!(preprocess_result.tool_loop_resample.is_none());
        assert!(!preprocess_result.loop_prevention_short_circuits.is_empty());

        reset_state();
    }

    #[tokio::test]
    async fn zero_resample_retries_promotes_to_hard_break_at_threshold() {
        let _guard = TEST_GUARD.lock().await;
        install_experimental_settings(serde_json::json!({
            "toolLoopRecoveryPolicy": "resampleThenBreak",
            "toolLoopMaxResampleRetries": 0,
            "inlineAudioAttachment": true
        }))
        .await;

        let session_id = "tool-loop-zero-resample-session";
        let repeated_args = r#"{"path":"src/main.ts"}"#;
        let repeated_error = "Error: file not found";

        let active_sessions = ensure_active_sessions();
        let (preprocess_result, _) = run_preprocess(
            &active_sessions,
            session_id,
            repeated_error_history(repeated_args, repeated_error),
            vec![test_tool_call("tc-3", "workspace__readFile", repeated_args)],
        )
        .await;

        assert!(preprocess_result.tool_loop_resample.is_none());
        assert!(preprocess_result.forced_stop.is_some());

        reset_state();
    }
}
