use crate::lifecycle::settings::SystemSettings;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{
    SettingsRepository, SqliteAssistantRepository, SqliteContentStoreRepository,
    SqliteKnowledgeRepository, SqliteMCPServerRepository, SqliteMessageRepository,
    SqlitePlanningRepository, SqlitePlaybookRepository, SqliteSessionRepository,
    SqliteSettingsRepository,
};
use crate::services;
use crate::state::{
    set_assistant_repository, set_content_store_repository, set_database_connection,
    set_knowledge_repository, set_mcp_server_repository, set_mcp_service_proxy_manager,
    set_message_repository, set_planning_repository, set_playbook_repository,
    set_session_repository, set_settings_repository,
};
use log::{error, info};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub async fn init_repositories(db: &DatabaseConnection) -> SystemSettings {
    let message_repo = SqliteMessageRepository::new(db.clone());
    info!("✅ Message repository initialized");

    let content_store_repo = SqliteContentStoreRepository::new(db.clone());
    info!("✅ Content store repository initialized");

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
    set_content_store_repository(content_store_repo);
    set_session_repository(session_repo);
    set_mcp_server_repository(mcp_server_repo);

    // Initialize Assistant, Playbook, and Knowledge repositories
    set_assistant_repository(SqliteAssistantRepository::new(db.clone()));
    set_playbook_repository(SqlitePlaybookRepository::new(db.clone()));
    set_knowledge_repository(SqliteKnowledgeRepository::new(db.clone()));
    set_planning_repository(SqlitePlanningRepository::new(db.clone()));

    info!("✅ Repository instances initialized");

    // Ensure default assistants exist (after repositories are initialized)
    if let Err(e) = services::assistant_init::ensure_default_assistants().await {
        error!("❌ Failed to ensure default assistants: {}", e);
    } else {
        info!("✅ Default assistants verified");
    }

    // Initialize the MCP Service Proxy Manager for session-aware builtin tools
    // For shared ownership, MCPServiceProxyManager needs Arc-wrapped dependencies
    // We'll modify the state management to use Arc storage pattern
    let proxy_manager = MCPServiceProxyManager::new_from_static_refs();

    set_mcp_service_proxy_manager(Arc::new(proxy_manager));

    info!("✅ MCP Service Proxy Manager initialized");

    system_settings
}
