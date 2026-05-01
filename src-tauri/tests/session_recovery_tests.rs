pub mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::events::{AgentEvent, AgentEventDispatcher};
use tauri_mcp_agent_lib::agent::lifecycle::recover_sessions_with_dispatcher;
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::state::AgentSession;
use tauri_mcp_agent_lib::repositories::{
    InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMessageRepository,
};
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus, set_message_repository};
use tokio::sync::{OnceCell, RwLock};

#[derive(Default)]
struct RecordingDispatcher {
    events: Mutex<Vec<AgentEvent>>,
}

impl RecordingDispatcher {
    fn agent_events(&self) -> Vec<AgentEvent> {
        self.events
            .lock()
            .expect("agent event lock poisoned")
            .clone()
    }
}

impl AgentEventDispatcher for RecordingDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        self.events
            .lock()
            .expect("agent event lock poisoned")
            .push(event);
        Ok(())
    }

    fn emit_compact_state(
        &self,
        _event: tauri_mcp_agent_lib::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }
}

static TEST_DB: OnceCell<sea_orm::DatabaseConnection> = OnceCell::const_new();

async fn ensure_test_runtime_state() {
    let _ = TEST_DB
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            set_message_repository(SqliteMessageRepository::new(db.clone()));
            db
        })
        .await;

    init_session_bus(SessionBus::new());
    init_concurrency_gate(ConcurrencyGate::new(
        DEFAULT_MAX_ACTIVE_AGENTS,
        DEFAULT_MAX_SUSPENDED_AGENTS,
        DEFAULT_MAX_ACTIVE_PROCESSES,
        DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));
}

fn build_session_metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Recovery regression".to_string()),
        status,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: Some(r#"{"name":"Recovery regression"}"#.to_string()),
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
        workspace_override: None,
    }
}

#[tokio::test]
async fn recover_sessions_marks_crashed_busy_sessions_paused_without_auto_resuming() {
    ensure_test_runtime_state().await;

    let session_id = format!("recovery-{}", uuid::Uuid::new_v4());
    let repo = Arc::new(InMemorySessionRepository::new());
    let session_repo: Arc<dyn SessionRepository> = repo.clone();
    let active_sessions: Arc<RwLock<HashMap<String, AgentSession>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let dispatcher = RecordingDispatcher::default();

    repo.upsert_session(&build_session_metadata(&session_id, SessionStatus::Busy))
        .await
        .expect("session upsert should succeed");

    recover_sessions_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        Arc::new(ContextRegistry::new()),
    )
    .await
    .expect("session recovery should succeed");

    let persisted = repo
        .get_session(&session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should still exist");
    assert_eq!(persisted.status, SessionStatus::Paused);

    let active = active_sessions.read().await;
    let recovered = active
        .get(&session_id)
        .expect("recovered session should be loaded in memory");
    assert_eq!(recovered.metadata.status, SessionStatus::Paused);
    assert!(!recovered.is_running);

    assert!(
        dispatcher.agent_events().iter().any(|event| matches!(
            event,
            AgentEvent::StatusChanged {
                session_id: changed_session_id,
                status: SessionStatus::Paused,
            } if changed_session_id == &session_id
        )),
        "recovery should emit a paused status change for the recovered session"
    );
}
