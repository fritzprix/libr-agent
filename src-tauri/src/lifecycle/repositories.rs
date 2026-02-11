use crate::repositories::{
    SettingsRepository, SqliteAssistantRepository, SqliteContentStoreRepository,
    SqliteKnowledgeRepository, SqliteMCPServerRepository, SqliteMessageRepository,
    SqlitePlanningRepository, SqlitePlaybookRepository, SqliteSessionRepository,
    SqliteSettingsRepository,
};
use crate::state::{
    set_assistant_repository, set_content_store_repository, set_database_connection,
    set_knowledge_repository, set_mcp_server_repository, set_mcp_service_proxy_manager,
    set_message_repository, set_planning_repository, set_playbook_repository,
    set_session_repository, set_settings_repository,
};
use log::{error, info};
use sea_orm::DatabaseConnection;

pub async fn init_repositories(
    db: &DatabaseConnection,
    session_manager: &crate::session::SessionManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize repository instances
    let message_repo = SqliteMessageRepository::new(db.clone());
    info!("✅ Message repository initialized");

    let content_store_repo = SqliteContentStoreRepository::new(db.clone());
    info!("✅ Content store repository initialized");

    let session_repo = SqliteSessionRepository::new(db.clone());
    info!("✅ Session repository initialized");

    let mcp_server_repo = SqliteMCPServerRepository::new(db.clone());
    info!("✅ MCP server repository initialized");

    // Fetch System Settings from DB
    #[derive(serde::Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct SystemSettings {
        search_index_frequency_minutes: Option<u64>,
        web_action_timeout_seconds: Option<u64>,
        active_session_retention_hours: Option<u64>,
    }

    let system_settings: SystemSettings = {
        // Initialize settings repository first
        let settings_repo = SqliteSettingsRepository::new(db.clone());
        set_settings_repository(settings_repo.clone());
        info!("✅ Settings repository initialized");

        match settings_repo.get("systemSettings").await {
            Ok(Some(model)) => serde_json::from_str(&model.value).unwrap_or_default(),
            Ok(None) => SystemSettings::default(),
            Err(e) => {
                log::warn!("Failed to fetch system settings: {}, using defaults", e);
                SystemSettings::default()
            }
        }
    };

    let index_freq_mins = system_settings.search_index_frequency_minutes.unwrap_or(5);
    let web_timeout_secs = system_settings.web_action_timeout_seconds.unwrap_or(30);
    let retention_hours = system_settings.active_session_retention_hours.unwrap_or(24);

    info!(
        "⚙️ System Configuration: Index Frequency = {}m, Web Timeout = {}s, Retention = {}h",
        index_freq_mins, web_timeout_secs, retention_hours
    );

    // Perform session cleanup
    match session_manager
        .cleanup_old_sessions(retention_hours, 5)
        .await
    {
        Ok(count) => info!(
            "🧹 Session cleanup completed: removed {} old sessions",
            count
        ),
        Err(e) => log::error!("❌ Session cleanup failed: {}", e),
    }

    // Start background indexing worker
    let _indexing_worker =
        crate::search::IndexingWorker::new(std::time::Duration::from_secs(index_freq_mins * 60));
    info!("✅ Background message indexing worker started");

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
    if let Err(e) = crate::services::assistant_init::ensure_default_assistants().await {
        error!("❌ Failed to ensure default assistants: {}", e);
    } else {
        info!("✅ Default assistants verified");
    }

    // Initialize the MCP manager with database connection
    // NOTE: Global MCPServerManager is deprecated in favor of session-isolated management
    let session_manager_arc = std::sync::Arc::new(session_manager.clone());
    let _mcp_manager = crate::mcp::MCPServerManager::new_with_session_manager_and_db(
        session_manager_arc.clone(),
        db.clone(),
    )
    .await;

    // Global MCP manager is no longer set due to Session Isolation architecture
    // All external server management is now per-session through MCPServiceProxyManager

    info!("✅ Session-Isolated MCP architecture initialized");

    // Initialize the MCP Service Proxy Manager for session-aware builtin tools
    use crate::mcp::MCPServiceProxyManager;

    // For shared ownership, MCPServiceProxyManager needs Arc-wrapped dependencies
    // We'll modify the state management to use Arc storage pattern
    let proxy_manager = MCPServiceProxyManager::new_from_static_refs();

    set_mcp_service_proxy_manager(std::sync::Arc::new(proxy_manager));

    info!("✅ MCP Service Proxy Manager initialized");

    Ok(())
}
