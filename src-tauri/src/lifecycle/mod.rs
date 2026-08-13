pub mod alias_migration;
pub mod app_setup;
pub mod database;
pub mod database_backup;
pub mod database_error;
pub mod frontend_ready;
pub mod migration_verifier;
pub mod repositories;
pub mod retry_utils;
pub mod schema_version; // Schema version tracking (matches migration #10 table layout)
pub mod settings;
pub mod windows_taskbar;

use crate::session::get_session_manager;
use crate::state::{set_sqlite_db_url, start_startup_timer};
use database_error::DatabaseError;
use log::info;

pub fn should_quarantine_on_init_failure(error: &DatabaseError) -> bool {
    matches!(
        error,
        DatabaseError::MigrationFailed { .. }
            | DatabaseError::MigrationModified { .. }
            | DatabaseError::CorruptedDatabase { .. }
    )
}

/// A synchronous wrapper to initialize and run the application with SQLite support.
pub fn run_with_sqlite_sync(db_url: String) {
    start_startup_timer();
    // Set the SQLite URL
    set_sqlite_db_url(db_url.clone());
    info!("🔄 Initializing LibrAgent with SQLite support: {db_url}");

    // Run async initialization on Tauri's global async runtime
    tauri::async_runtime::block_on(async {
        let backend_init_start = std::time::Instant::now();
        let session_manager = get_session_manager().expect("SessionManager not initialized");

        let quarantine_start = std::time::Instant::now();
        if let Err(err) = database::maybe_restore_quarantined_database(&db_url).await {
            log::warn!(
                "⚠️ Failed to inspect quarantined DB recovery candidates: {}",
                err
            );
        }
        crate::state::log_startup_phase(
            "quarantine_inspect",
            Some(quarantine_start.elapsed().as_millis()),
        );

        // Initialize Database (now returns Result).
        // Strategy: if migration fails on an existing DB (e.g. schema mismatch from
        // an untracked legacy DB), rename it aside and retry with a fresh DB so the
        // app always starts. The old DB is preserved as *.incompatible for recovery.
        let init_db_start = std::time::Instant::now();
        let db = match database::init_database(&db_url).await {
            Ok(connection) => connection,
            Err(first_err) => {
                log::error!("❌ Database init failed (attempt 1): {}", first_err);

                if !should_quarantine_on_init_failure(&first_err) {
                    eprintln!("❌ Database init failed: {}", first_err);
                    eprintln!(
                        "💡 Refusing to quarantine the existing database for a non-migration failure."
                    );
                    std::process::exit(1);
                }

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
        crate::state::log_startup_phase("init_database", Some(init_db_start.elapsed().as_millis()));
        // Initialize repositories required by app setup and command handlers.
        let repos_start = std::time::Instant::now();
        repositories::init_repositories(&db).await;
        crate::state::log_startup_phase(
            "init_repositories",
            Some(repos_start.elapsed().as_millis()),
        );

        // Global MCPServerManager initialization is intentionally skipped.
        // Session-isolated MCP architecture uses MCPServiceProxyManager initialized in repositories.
        let _ = db;
        let _ = session_manager;
        info!("✅ Session-isolated MCP architecture initialized");
        crate::state::log_startup_phase(
            "backend_block_on_complete",
            Some(backend_init_start.elapsed().as_millis()),
        );
    });
    // Call the main run function
    if let Some(elapsed_ms) = crate::state::startup_elapsed_ms() {
        info!(
            "⏱️ Startup metric: calling crate::run (window create) after {}ms",
            elapsed_ms
        );
    }
    crate::run();
}
