pub mod app_setup;
pub mod database;
pub mod database_backup;
pub mod database_error;
pub mod migration_verifier;
pub mod repositories;
pub mod retry_utils;
pub mod schema_version; // Schema version tracking (matches migration #10 table layout)
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

        // Initialize Database (now returns Result).
        // Strategy: if migration fails on an existing DB (e.g. schema mismatch from
        // an untracked legacy DB), rename it aside and retry with a fresh DB so the
        // app always starts. The old DB is preserved as *.incompatible for recovery.
        let db = match database::init_database(&db_url).await {
            Ok(connection) => connection,
            Err(first_err) => {
                log::error!("❌ Database init failed (attempt 1): {}", first_err);

                // Extract the file path from the URL and try to quarantine the bad DB.
                // Re-use the same helper as init_database so there is no duplicated
                // URL-parsing logic scattered across the module.
                let db_file = database::extract_db_file_path(&db_url).unwrap_or("");

                let quarantine_path = format!("{}.incompatible", db_file);
                if !db_file.is_empty() && std::path::Path::new(db_file).exists() {
                    match std::fs::rename(db_file, &quarantine_path) {
                        Ok(_) => log::warn!(
                            "⚠️ Incompatible DB quarantined to: {}. Retrying with fresh DB.",
                            quarantine_path
                        ),
                        Err(e) => log::error!("❌ Could not quarantine DB file: {}", e),
                    }
                }

                // Retry with fresh DB
                match database::init_database(&db_url).await {
                    Ok(connection) => {
                        log::warn!(
                            "✅ Fresh DB created after quarantine. Previous data preserved at: {}",
                            quarantine_path
                        );
                        connection
                    }
                    Err(second_err) => {
                        // Even fresh DB failed — something is very wrong (permissions, disk full, etc.)
                        eprintln!("❌ Database init failed even with fresh DB: {}", second_err);
                        eprintln!("💡 Check disk space and file permissions at: {}", db_file);
                        std::process::exit(1);
                    }
                }
            }
        };

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

        // Global MCPServerManager initialization is intentionally skipped.
        // Session-isolated MCP architecture uses MCPServiceProxyManager initialized in repositories.
        let _ = db;
        info!("✅ Session-isolated MCP architecture initialized");
    });

    // Call the main run function
    crate::run();
}
