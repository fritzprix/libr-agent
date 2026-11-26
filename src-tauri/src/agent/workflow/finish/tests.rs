use super::messages::{
    user_facing_persist_session_missing_error, user_facing_persist_storage_error,
    user_facing_workflow_event_emit_error,
};
use super::persist::{latest_assistant_message_from_cache, mark_error_after_persist_failure};
use super::stop_hook::{
    evaluate_stop_hook_outcome, settle_stub_event, subagent_matcher_value,
    subagent_matcher_value_with_names, StopHookTransition,
};
use super::*;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::state::{CompactionRuntimeState, PendingEvent, PendingEventManager};
use crate::repositories::{
    InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct WorkflowEventFailingDispatcher {
    fail_workflow_events: bool,
    events: Mutex<Vec<AgentEvent>>,
}

impl AgentEventDispatcher for WorkflowEventFailingDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        if self.fail_workflow_events
            && matches!(
                event,
                AgentEvent::WorkflowCompleted { .. } | AgentEvent::WorkflowError { .. }
            )
        {
            return Err("event emission failed".to_string());
        }
        self.events.lock().expect("event lock").push(event);
        Ok(())
    }

    fn emit_compact_state(
        &self,
        _event: crate::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }

    fn emit_completion_cancel(
        &self,
        _request: crate::agent::llm::types::CompletionCancelRequest,
    ) -> Result<(), String> {
        Ok(())
    }
}

async fn seed_in_memory_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    status: SessionStatus,
) {
    let metadata = SessionMetadata {
        id: session_id.to_string(),
        name: None,
        status: status.clone(),
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: crate::execution_mode::ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    };
    session_repo
        .upsert_session(&metadata)
        .await
        .expect("session upsert");
    active_sessions
        .write()
        .await
        .insert(session_id.to_string(), build_session(session_id));
}

#[derive(Default)]
struct RecordingDispatcher {
    events: Mutex<Vec<AgentEvent>>,
}

impl RecordingDispatcher {
    fn agent_events(&self) -> Vec<AgentEvent> {
        self.events.lock().expect("event lock").clone()
    }
}

impl AgentEventDispatcher for RecordingDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        self.events.lock().expect("event lock").push(event);
        Ok(())
    }

    fn emit_compact_state(
        &self,
        _event: crate::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }

    fn emit_completion_cancel(
        &self,
        _request: crate::agent::llm::types::CompletionCancelRequest,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn user_facing_persist_errors_hide_internal_details() {
    assert_eq!(
        user_facing_persist_session_missing_error(),
        "The session could not be found while saving the final response."
    );
    assert!(!user_facing_persist_storage_error().contains("session="));
    assert!(!user_facing_workflow_event_emit_error("WorkflowCompleted").contains("emit"));
}

#[tokio::test]
async fn mark_error_after_persist_failure_emits_workflow_error() {
    crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
    crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    let session_repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    seed_in_memory_session(
        &session_repo,
        &active_sessions,
        "sess-persist-mark-error",
        SessionStatus::Busy,
    )
    .await;

    let dispatcher = RecordingDispatcher::default();
    let user_message = user_facing_persist_storage_error();
    mark_error_after_persist_failure(
        &session_repo,
        &active_sessions,
        &dispatcher,
        "sess-persist-mark-error",
        &user_message,
    )
    .await
    .expect("mark error succeeds");

    assert_eq!(
        session_repo
            .get_session("sess-persist-mark-error")
            .await
            .expect("session load")
            .expect("session exists")
            .status,
        SessionStatus::Error
    );
    assert!(dispatcher.agent_events().iter().any(|event| {
        matches!(
            event,
            AgentEvent::WorkflowError { session_id, error }
                if session_id == "sess-persist-mark-error"
                    && error.display_message == user_message
        )
    }));
}

#[tokio::test]
async fn settle_session_and_finalize_error_with_dispatcher_propagates_emit_failure() {
    crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
    crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    let session_repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    seed_in_memory_session(
        &session_repo,
        &active_sessions,
        "sess-emit-failure",
        SessionStatus::Busy,
    )
    .await;

    let dispatcher = WorkflowEventFailingDispatcher {
        fail_workflow_events: true,
        ..Default::default()
    };
    let runtime_error = AgentRuntimeError::new(
        AgentRuntimeErrorType::AiServiceError,
        "Provider request failed",
    );

    let result = settle_session_and_finalize_error_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        "sess-emit-failure",
        None,
        runtime_error,
    )
    .await;

    assert_eq!(
        result,
        Err(user_facing_workflow_event_emit_error("WorkflowError"))
    );
    assert_eq!(
        session_repo
            .get_session("sess-emit-failure")
            .await
            .expect("session load")
            .expect("session exists")
            .status,
        SessionStatus::Error
    );
}

fn build_session(session_id: &str) -> AgentSession {
    let now = chrono::Utc::now().timestamp_millis();
    AgentSession {
        metadata: SessionMetadata {
            id: session_id.to_string(),
            name: None,
            status: SessionStatus::Busy,
            model: "gpt-5.4".to_string(),
            provider: "openai".to_string(),
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
            execution_mode: crate::execution_mode::ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
            status_reason: None,
            forked_from: None,
        },
        is_running: true,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(Some(SystemTime::now()))),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
        invalid_tool_args_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        pending_user_prompt: crate::agent::state::new_user_prompt_slot(),
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

#[tokio::test]
async fn session_has_pending_events_is_false_when_queue_empty() {
    let sessions = Arc::new(RwLock::new(HashMap::from([(
        "sess-1".to_string(),
        build_session("sess-1"),
    )])));

    assert!(!session_has_pending_events(&sessions, "sess-1").await);
}

#[tokio::test]
async fn session_has_pending_events_is_true_when_queue_has_messages() {
    let session = build_session("sess-2");
    session
        .pending_events
        .write()
        .await
        .add(PendingEvent::Message("msg-1".to_string()));

    let sessions = Arc::new(RwLock::new(HashMap::from([(
        "sess-2".to_string(),
        session,
    )])));

    assert!(session_has_pending_events(&sessions, "sess-2").await);
}

#[test]
fn latest_assistant_message_from_cache_returns_newest_assistant() {
    use crate::mcp::types::MCPContent;
    use crate::models::chat::Message;

    fn assistant_message(id: &str, text: &str) -> Message {
        Message {
            id: id.to_string(),
            session_id: "sess".to_string(),
            role: "assistant".to_string(),
            content: vec![MCPContent::Text {
                text: text.to_string(),
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: 1,
            updated_at: 1,
            source: None,
            error: None,
            metadata: None,
        }
    }

    let messages = vec![
        assistant_message("asst-1", "first"),
        Message {
            id: "tool-1".to_string(),
            session_id: "sess".to_string(),
            role: "tool".to_string(),
            content: vec![MCPContent::Text {
                text: "tool output".to_string(),
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: 2,
            updated_at: 2,
            source: None,
            error: None,
            metadata: None,
        },
        assistant_message("asst-2", "final answer"),
    ];

    let latest = latest_assistant_message_from_cache(&messages).expect("assistant");
    assert_eq!(latest.id, "asst-2");
    let MCPContent::Text { text, .. } = &latest.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(text, "final answer");
}

#[test]
fn settle_stub_event_picks_stop_for_top_level_session() {
    assert_eq!(
        settle_stub_event(None),
        crate::services::hooks::HookEvent::Stop
    );
}

#[test]
fn settle_stub_event_picks_subagent_stop_when_parent_set() {
    assert_eq!(
        settle_stub_event(Some("parent-session")),
        crate::services::hooks::HookEvent::SubagentStop
    );
}

#[test]
fn evaluate_stop_hook_outcome_continues_up_to_cap_then_settles() {
    use crate::services::hooks::{
        clear_hook_context, take_turn_hook_context, HookDecision, HookDispatchResult, HookEvent,
        MAX_STOP_HOOK_CONTINUATIONS,
    };

    let sid = "finish-stop-hook-cap-test";
    clear_hook_context(sid);

    for i in 1..=MAX_STOP_HOOK_CONTINUATIONS {
        let cont = evaluate_stop_hook_outcome(
            sid,
            HookEvent::Stop,
            HookDispatchResult {
                additional_context: vec![],
                decision: Some(HookDecision::Block),
                reason: Some(format!("keep going #{i}")),
                updated_input: None,
                stop_reason: None,
                stops_processing: false,
            },
        );
        assert_eq!(
            cont,
            StopHookTransition::Continue,
            "continuation #{i} must be allowed"
        );
        let ctx = take_turn_hook_context(sid).expect("feedback queued");
        assert!(ctx.contains(&format!("keep going #{i}")));
    }

    // Next block exceeds MAX_STOP_HOOK_CONTINUATIONS -> must settle to Idle.
    let over_cap = evaluate_stop_hook_outcome(
        sid,
        HookEvent::Stop,
        HookDispatchResult {
            additional_context: vec![],
            decision: Some(HookDecision::Block),
            reason: Some("one too many".to_string()),
            updated_input: None,
            stop_reason: None,
            stops_processing: false,
        },
    );
    assert_eq!(
        over_cap,
        StopHookTransition::Settle,
        "must not continue past MAX_STOP_HOOK_CONTINUATIONS"
    );
    assert!(take_turn_hook_context(sid).is_none());
    clear_hook_context(sid);
}

#[test]
fn stop_hook_continue_false_settles_with_user_visible_reason_even_with_block() {
    use crate::services::hooks::{
        clear_hook_context, take_turn_hook_context, HookDecision, HookDispatchResult, HookEvent,
    };

    let sid = "finish-stop-hook-stop-test";
    clear_hook_context(sid);
    let transition = evaluate_stop_hook_outcome(
        sid,
        HookEvent::Stop,
        HookDispatchResult {
            additional_context: vec![],
            decision: Some(HookDecision::Block),
            reason: Some("keep going".to_string()),
            updated_input: None,
            stop_reason: Some("budget exhausted".to_string()),
            stops_processing: true,
        },
    );
    assert_eq!(
        transition,
        StopHookTransition::StoppedByHook("budget exhausted".to_string()),
        "continue:false must settle the turn and surface stopReason"
    );
    assert!(
        take_turn_hook_context(sid).is_none(),
        "a stopped turn must not queue continuation feedback"
    );
    assert_eq!(
        crate::services::hooks::stop_hook_continuation_count(sid),
        0,
        "stopping must not consume the continuation budget"
    );
    clear_hook_context(sid);
}

#[test]
fn subagent_matcher_value_uses_agent_type_for_children_only() {
    // Ids exactly as the app builds them: plugin source ids are
    // `plugin-<16 hex>` (skill_service::importing::plugin_source_id), so
    // a plugin agent row is `plugin:plugin-<16 hex>:<agent>` (#3003 G2.3).
    assert_eq!(
        subagent_matcher_value_with_names(None, Some("Explore"), Some("child"), None, None),
        ""
    );
    // Plugin agents resolve to the plugin manifest name for the scope.
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
            Some("my-plugin"),
            None,
        ),
        "my-plugin:reviewer"
    );
    // A plugin whose manifest name cannot be read falls back to the
    // source id (still better than the session name).
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
            None,
            None,
        ),
        "plugin-1a2b3c4d5e6f7a8b:reviewer"
    );
    // User agents resolve to the assistant configuration's name.
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
            Some("child-session-name"),
            None,
            Some("reviewer"),
        ),
        "reviewer"
    );
    // Without an assistant row, the session name remains the fallback.
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
            Some("reviewer"),
            None,
            None,
        ),
        "reviewer"
    );
    assert_eq!(
        subagent_matcher_value_with_names(Some("parent-1"), None, Some("reviewer"), None, None),
        "reviewer"
    );
    assert_eq!(
        subagent_matcher_value_with_names(Some("parent-1"), Some("  "), None, None, None),
        ""
    );
}

/// Drives the production async wrapper (not just the pure core) with an
/// app-built plugin id. The plugin is not installed in the unit-test
/// environment, so the namespace lookup misses and the scope falls back
/// to the source id — proving the wrapper resolves through the
/// session+global store path without panicking (#3003 G2.3).
#[tokio::test]
async fn subagent_matcher_value_resolves_plugin_scope_async() {
    assert_eq!(
        subagent_matcher_value(
            Some("parent-1"),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
        )
        .await,
        "plugin-1a2b3c4d5e6f7a8b:reviewer"
    );
    assert_eq!(
        subagent_matcher_value(
            None,
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child")
        )
        .await,
        ""
    );
    assert_eq!(
        subagent_matcher_value(Some("parent-1"), None, Some("reviewer")).await,
        "reviewer"
    );
}

/// #3003 G2.3: session-scoped plugin agents live in the **parent**
/// session's plugin store, so `subagent_matcher_value` must resolve the
/// plugin namespace from the parent session's store and return
/// `<manifest-name>:<agent>`.
#[tokio::test]
async fn subagent_matcher_value_resolves_parent_session_plugin_namespace_async() {
    use crate::services::hooks::registry::{
        get_session_plugins_directory, save_source_in, upsert_session_store_entry, PluginMetadata,
        PluginSourceManifest, SessionPluginStoreEntry,
    };
    let parent_id = format!("test-parent-{}", uuid::Uuid::new_v4());
    let source_id = "plugin-1a2b3c4d5e6f7a8b";
    let plugins_dir = get_session_plugins_directory(&parent_id).expect("session plugins dir");
    std::fs::create_dir_all(plugins_dir.join(source_id)).expect("create plugin dir");

    let manifest = PluginSourceManifest {
        source_url: "https://github.com/example/my-plugin".to_string(),
        source_id: source_id.to_string(),
        skill_names: Vec::new(),
        hooks: None,
        hooks_approved: false,
        approval_hashes: std::collections::BTreeMap::new(),
        created_at: 0,
        plugin: Some(PluginMetadata {
            name: "my-plugin".to_string(),
            version: None,
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            keywords: Vec::new(),
        }),
        hook_files: Vec::new(),
        inline_hooks: Vec::new(),
        mcp_servers: Vec::new(),
        mcp_approved: false,
        enabled: true,
        installed_commit: None,
    };
    save_source_in(&plugins_dir, manifest).expect("save plugin manifest");
    upsert_session_store_entry(
        &plugins_dir,
        SessionPluginStoreEntry {
            source_id: source_id.to_string(),
            source_url: "https://github.com/example/my-plugin".to_string(),
            installed_at: 0,
        },
    )
    .expect("upsert session store entry");

    let resolved = subagent_matcher_value(
        Some(&parent_id),
        Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
        Some("child-session-name"),
    )
    .await;

    // Clean up the test parent's default workspace directory.
    if let Ok(manager) = crate::session::get_session_manager() {
        let _ = std::fs::remove_dir_all(
            manager.get_session_default_workspace_dir_unverified(&parent_id),
        );
    }

    assert_eq!(resolved, "my-plugin:reviewer");
}

/// #3003 G2.3: a user agent's matcher value is the **assistant
/// configuration's name** read from the repository — a real lookup, not
/// an injected string.
#[tokio::test]
async fn subagent_matcher_value_resolves_user_assistant_name_async() {
    use sea_orm_migration::MigratorTrait;
    let _guard = crate::state::lock_test_global_state().await;
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    crate::state::set_assistant_repository(
        crate::repositories::assistant_repository::SqliteAssistantRepository::new(db),
    );
    use crate::repositories::AssistantRepository;
    crate::state::get_assistant_repository()
        .create_assistant(
            "b15fdf3e-6192-453d-9dc2-7b8ae6dab321".to_string(),
            "Code Reviewer".to_string(),
            "{}".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(
        subagent_matcher_value(
            Some("parent-1"),
            Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
            Some("child-session-name"),
        )
        .await,
        "Code Reviewer"
    );
    // Unknown assistant id → session name fallback (with a warning).
    assert_eq!(
        subagent_matcher_value(
            Some("parent-1"),
            Some("00000000-0000-0000-0000-000000000000"),
            Some("child-session-name"),
        )
        .await,
        "child-session-name"
    );
}
