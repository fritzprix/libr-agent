use crate::lifecycle::settings::SystemSettings;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{
    SettingsRepository, SqliteAssistantRepository, SqliteAttachmentsRepository,
    SqliteCompactContextRepository, SqliteKnowledgeRepository, SqliteKnowledgeV2Repository,
    SqliteMCPServerRepository, SqliteMessageRepository, SqlitePendingQueueRepository,
    SqlitePlanningRepository, SqlitePlaybookRepository, SqliteScheduledTaskRepository,
    SqliteSessionRepository, SqliteSettingsRepository,
};
use crate::state::{
    set_assistant_repository, set_attachments_repository, set_compact_context_repository,
    set_database_connection, set_knowledge_repository, set_knowledge_v2_repository,
    set_mcp_server_repository, set_mcp_service_proxy_manager, set_message_repository,
    set_pending_queue_repository, set_planning_repository, set_playbook_repository,
    set_scheduled_task_repository, set_session_repository, set_settings_repository,
};
use log::info;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub async fn init_repositories(db: &DatabaseConnection) -> SystemSettings {
    let message_repo = SqliteMessageRepository::new(db.clone());
    info!("✅ Message repository initialized");

    let attachments_repo = SqliteAttachmentsRepository::new(db.clone());
    info!("✅ Attachments repository initialized");

    let session_repo = SqliteSessionRepository::new(db.clone());
    info!("✅ Session repository initialized");

    let mcp_server_repo = SqliteMCPServerRepository::new(db.clone());
    info!("✅ MCP server repository initialized");

    // Initialize settings repository first
    let settings_repo = SqliteSettingsRepository::new(db.clone());
    set_settings_repository(settings_repo.clone());
    info!("✅ Settings repository initialized");

    let system_settings: SystemSettings = match settings_repo.get("systemSettings").await {
        Ok(Some(model)) => serde_json::from_str(&model.value).unwrap_or_default(),
        Ok(None) => SystemSettings::default(),
        Err(e) => {
            log::warn!("Failed to fetch system settings: {}, using defaults", e);
            SystemSettings::default()
        }
    };

    // Set the global database connection
    set_database_connection(db.clone());
    info!("✅ Database connection initialized");

    // Set the global repository instances
    set_message_repository(message_repo);
    set_pending_queue_repository(SqlitePendingQueueRepository::new(db.clone()));
    set_attachments_repository(attachments_repo);
    set_session_repository(session_repo);
    set_mcp_server_repository(mcp_server_repo);

    // Initialize Assistant, Playbook, and Knowledge repositories
    set_assistant_repository(SqliteAssistantRepository::new(db.clone()));
    set_playbook_repository(SqlitePlaybookRepository::new(db.clone()));
    set_knowledge_repository(SqliteKnowledgeRepository::new(db.clone()));
    set_knowledge_v2_repository(SqliteKnowledgeV2Repository::new(db.clone()));
    set_planning_repository(SqlitePlanningRepository::new(db.clone()));
    set_scheduled_task_repository(SqliteScheduledTaskRepository::new(db.clone()));
    set_compact_context_repository(SqliteCompactContextRepository::new(db.clone()));

    info!("✅ Repository instances initialized");

    // Run alias migrations to clean up legacy data
    let alias_start = std::time::Instant::now();
    crate::lifecycle::alias_migration::run_alias_migrations(db).await;
    crate::state::log_startup_phase(
        "alias_migrations",
        Some(alias_start.elapsed().as_millis()),
    );

    // Initialize the MCP Service Proxy Manager for session-aware builtin tools
    // For shared ownership, MCPServiceProxyManager needs Arc-wrapped dependencies
    // We'll modify the state management to use Arc storage pattern
    let proxy_start = std::time::Instant::now();
    let proxy_manager = MCPServiceProxyManager::new_from_static_refs();

    set_mcp_service_proxy_manager(Arc::new(proxy_manager));

    info!("✅ MCP Service Proxy Manager initialized");
    crate::state::log_startup_phase(
        "mcp_proxy_manager_init",
        Some(proxy_start.elapsed().as_millis()),
    );

    system_settings
}
