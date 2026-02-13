pub mod app_setup;
pub mod database;
pub mod repositories;
pub mod settings;

use crate::search;
use crate::session::get_session_manager;
use crate::state::set_sqlite_db_url;
use log::info;

/// A synchronous wrapper to initialize and run the application with SQLite support.
pub fn run_with_sqlite_sync(db_url: String) {
    // Set the SQLite URL
    set_sqlite_db_url(db_url.clone());
    info!("🔄 Initializing LibrAgent with SQLite support: {db_url}");

    // Run async initialization on Tauri's global async runtime
    tauri::async_runtime::block_on(async {
        let session_manager = get_session_manager().expect("SessionManager not initialized");

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
        let indexing_worker =
            search::IndexingWorker::new(std::time::Duration::from_secs(index_freq_mins * 60));
        std::mem::forget(indexing_worker);
        info!("✅ Background message indexing worker started");

        // Global MCPServerManager initialization is intentionally skipped.
        // Session-isolated MCP architecture uses MCPServiceProxyManager initialized in repositories.
        let _ = db;
        info!("✅ Session-isolated MCP architecture initialized");
    });

    // Call the main run function
    crate::run();
}
