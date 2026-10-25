use super::*;
use crate::entity::{
    assistant, knowledge, mcp_server, planning_goal, planning_scratchpad, planning_todo, playbook,
    session,
};
use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema, Set};
use serde_json::json;
use std::sync::Arc;

mod builtin_lifecycle;
mod concurrency;
mod readiness;
mod scenarios;

pub(super) struct TestHarness {
    pub(super) _guard: tokio::sync::MutexGuard<'static, ()>,
    pub(super) manager: Arc<MCPServiceProxyManager>,
}

pub(super) fn test_session_active_model(id: impl Into<String>) -> session::ActiveModel {
    session::ActiveModel {
        id: Set(id.into()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(0),
        status: Set("idle".to_string()),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        execution_mode: Set("normal".to_string()),
        workspace_isolation: Set("host".to_string()),
        is_bookmarked: Set(false),
        ..Default::default()
    }
}

pub(super) async fn insert_test_session(manager: &MCPServiceProxyManager, session_id: &str) {
    let assistant_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    assistant::Entity::insert(assistant::ActiveModel {
        id: Set(assistant_id.clone()),
        name: Set(format!("Test Assistant for {session_id}")),
        config: Set("{}".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    })
    .exec(&*manager.db)
    .await
    .expect("failed to insert test assistant");

    let mut model = test_session_active_model(session_id);
    model.assistant_id = Set(Some(assistant_id));

    session::Entity::insert(model)
        .exec(&*manager.db)
        .await
        .expect("failed to insert test session");
}

pub(super) async fn create_test_harness() -> TestHarness {
    let guard = crate::state::lock_test_global_state().await;
    crate::state::reset_state();

    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    let schema = Schema::new(db.get_database_backend());

    for create in [
        schema.create_table_from_entity(session::Entity),
        schema.create_table_from_entity(playbook::Entity),
        schema.create_table_from_entity(assistant::Entity),
        schema.create_table_from_entity(knowledge::Entity),
        schema.create_table_from_entity(planning_goal::Entity),
        schema.create_table_from_entity(planning_todo::Entity),
        schema.create_table_from_entity(planning_scratchpad::Entity),
        schema.create_table_from_entity(crate::entity::settings::Entity),
        schema.create_table_from_entity(mcp_server::Entity),
    ] {
        db.execute(db.get_database_backend().build(&create))
            .await
            .expect("Failed to create test table");
    }

    crate::lifecycle::repositories::init_repositories(&db).await;
    crate::state::init_session_bus(crate::agent::session_bus::SessionBus::new());
    crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    // Create a minimal SessionManager
    let session_manager = Arc::new(crate::session::SessionManager::new().unwrap());

    TestHarness {
        _guard: guard,
        manager: Arc::new(MCPServiceProxyManager::new(Arc::new(db), session_manager)),
    }
}
