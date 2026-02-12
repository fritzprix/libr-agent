pub mod database;
pub mod repositories;
pub mod app_setup;
pub mod settings;

use log::{info};
use crate::state::set_sqlite_db_url;
use crate::session::get_session_manager;
use crate::search;

/// A synchronous wrapper to initialize and run the application with SQLite support.
pub fn run_with_sqlite_sync(db_url: String) {
    // Set the SQLite URL
    set_sqlite_db_url(db_url.clone());
    info!("🔄 Initializing LibrAgent with SQLite support: {db_url}");

    // Create a Tokio runtime for async initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let session_manager = get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = std::sync::Arc::new(session_manager.clone());

        // Initialize Database
        let db = database::init_database(&db_url).await;

        // Initialize Repositories and get System Settings
        let system_settings = repositories::init_repositories(&db).await;

        let index_freq_mins = system_settings.search_index_frequency_minutes.unwrap_or(5);
        let retention_hours = system_settings.active_session_retention_hours.unwrap_or(24);

        info!(
            "⚙️ System Configuration: Index Frequency = {}m, Retention = {}h",
            index_freq_mins, retention_hours
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
            search::IndexingWorker::new(std::time::Duration::from_secs(index_freq_mins * 60));
        info!("✅ Background message indexing worker started");

        // Initialize the MCP manager with database connection
        // NOTE: Global MCPServerManager is deprecated in favor of session-isolated management
        use crate::mcp::MCPServerManager;
        let _mcp_manager = MCPServerManager::new_with_session_manager_and_db(
            session_manager_arc.clone(),
            db.clone(),
        )
        .await;

        info!("✅ Session-Isolated MCP architecture initialized");
    });

    // Call the main run function
    crate::run();
}
