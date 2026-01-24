use log::{error, info, warn};
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};

mod agent;
mod commands;
mod config;
pub mod entity; // SeaORM entity definitions
mod logger; // Custom file logger
pub mod mcp; // Make public for integration tests
pub mod repositories; // Make public for integration tests
mod search;
mod services;
pub mod session;
mod session_isolation;
mod state;
pub mod utils;

// Re-export migration for use in MCP modules
pub use migration;

use commands::agent_commands::{
    agent_call_builtin_tool, agent_clear_all_sessions, agent_create_session,
    agent_create_session_with_initial_message, agent_delete_session, agent_factory_reset,
    agent_get_all_sessions, agent_get_available_tools, agent_get_service_contexts,
    agent_get_session, agent_get_tools, agent_handle_llm_error, agent_handle_llm_response,
    agent_handle_tool_result, agent_init_session_with_messages, agent_inject_messages,
    agent_pause_workflow, agent_resume_session, agent_resume_workflow, agent_send_message,
    agent_terminate_workflow, agent_update_session_config,
};
use commands::assistant_crud_commands::{
    create_assistant, delete_assistant, get_assistant, list_assistants, update_assistant,
};
use commands::browser_commands::*;
use commands::content_store_commands::delete_content_store;
use commands::download_commands::{download_workspace_file, export_and_download_zip};
use commands::file_commands::{read_dropped_file, read_file, workspace_write_file, write_file};
use commands::log_commands::{
    backup_current_log, clear_current_log, list_log_files, log_debug, log_error_from_frontend,
    log_info, log_trace, log_warn,
};
use commands::mcp_commands::{
    call_builtin_tool, call_mcp_tool, check_all_servers_status, check_server_status,
    complete_oauth_flow, get_connected_servers, get_oauth_token, get_service_context,
    get_validated_tools, has_oauth_token, list_all_tools, list_all_tools_unified,
    list_available_builtin_server_definitions, list_builtin_servers,
    list_builtin_servers_with_metadata, list_builtin_tools, list_mcp_tools, list_tools_from_config,
    revoke_oauth_token, sample_from_mcp_server, start_mcp_server, start_oauth_flow,
    stop_mcp_server, validate_tool_schema,
};
use commands::mcp_server_config_commands::{
    create_mcp_server_config, delete_mcp_server_config, list_mcp_server_configs,
    update_mcp_server_config,
};
use commands::messages_commands::{
    messages_delete, messages_delete_all_for_session, messages_get_page, messages_search,
    messages_upsert, messages_upsert_many,
};
use commands::playbook_commands::{
    create_playbook, delete_playbook, get_playbook, list_playbooks, toggle_playbook_bookmark,
    update_playbook,
};
use commands::session_commands::{remove_session, switch_session};
use commands::settings_commands::{delete_setting, get_setting, list_settings, set_setting};
use commands::url_commands::open_external_url;
use commands::workspace_commands::{
    cancel_workspace_override, get_app_data_dir, get_app_logs_dir, get_workspace_override, greet,
    list_workspace_files, open_workspace_file_with_default_app, open_workspace_in_explorer,
    open_workspace_in_terminal, set_workspace_override,
};
use mcp::MCPServerManager;
use services::{InteractiveBrowserServer, SecureFileManager};
use session::get_session_manager;

// Re-export state management functions
pub use state::{
    get_assistant_repository, get_content_store_repository, get_database_connection,
    get_knowledge_repository, get_mcp_server_repository, get_mcp_service_proxy_manager,
    get_message_repository, get_playbook_repository, get_session_repository, get_sqlite_db_url,
    set_assistant_repository, set_content_store_repository, set_database_connection,
    set_knowledge_repository, set_mcp_server_repository, set_mcp_service_proxy_manager,
    set_message_repository, set_playbook_repository, set_session_repository, set_sqlite_db_url,
};

/// A synchronous wrapper to initialize and run the application with SQLite support.
///
/// This function sets up a Tokio runtime to perform async initialization of the
/// `MCPServerManager` with a SQLite database, then calls the main `run` function.
///
/// # Arguments
/// * `db_url` - The connection URL for the SQLite database.
pub fn run_with_sqlite_sync(db_url: String) {
    // Set the SQLite URL
    set_sqlite_db_url(db_url.clone());
    info!("🔄 Initializing LibrAgent with SQLite support: {db_url}");

    // Create a Tokio runtime for async initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let session_manager = get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = std::sync::Arc::new(session_manager.clone());

        // Connect to database using SeaORM
        let db = sea_orm::Database::connect(&db_url)
            .await
            .unwrap_or_else(|e| {
                // If this looks like a file-backed sqlite URL, try to create the file
                if let Some(path) = db_url.strip_prefix("sqlite://") {
                    info!("⚙️ Database connect failed, attempting to create DB file: {path}");
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        if let Err(err) = std::fs::create_dir_all(parent) {
                            error!("Failed to create parent directory for DB: {err}");
                        }
                    }

                    if let Err(err) = std::fs::File::create(path) {
                        error!("Failed to create SQLite DB file: {err}");
                    } else {
                        info!("✅ Created new SQLite DB file: {path}");
                    }

                    // Retry connection once
                    futures::executor::block_on(async {
                        sea_orm::Database::connect(&db_url)
                            .await
                            .unwrap_or_else(|err| {
                                panic!("Failed to connect to database after creating file: {err}")
                            })
                    })
                } else {
                    panic!("Failed to connect to database: {e}");
                }
            });
        info!("✅ Database connected: {db_url}");

        // Run migrations
        use migration::{Migrator, MigratorTrait};
        Migrator::up(&db, None)
            .await
            .expect("Failed to run database migrations");
        info!("✅ Database migrations applied");

        // Initialize repository instances
        use repositories::{
            SqliteContentStoreRepository, SqliteMCPServerRepository, SqliteMessageRepository,
            SqliteSessionRepository,
        };

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
            use crate::repositories::settings_repository::SettingsRepository;
            use crate::repositories::SqliteSettingsRepository;
            use crate::state::set_settings_repository;

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
            search::IndexingWorker::new(std::time::Duration::from_secs(index_freq_mins * 60));
        info!("✅ Background message indexing worker started");

        // Set the global database connection
        set_database_connection(db.clone());
        info!("✅ Database connection initialized");

        // Set the global repository instances
        set_message_repository(message_repo);
        set_content_store_repository(content_store_repo);
        set_session_repository(session_repo);
        use crate::state::set_mcp_server_repository;
        set_mcp_server_repository(mcp_server_repo);

        // Initialize Assistant, Playbook, and Knowledge repositories
        use crate::repositories::{
            SqliteAssistantRepository, SqliteKnowledgeRepository, SqlitePlanningRepository,
            SqlitePlaybookRepository,
        };
        use crate::state::{
            set_assistant_repository, set_knowledge_repository, set_planning_repository,
            set_playbook_repository,
        };
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

        // Initialize the MCP manager with database connection
        // NOTE: Global MCPServerManager is deprecated in favor of session-isolated management
        let _mcp_manager = MCPServerManager::new_with_session_manager_and_db(
            session_manager_arc.clone(),
            db.clone(),
        )
        .await;

        // Global MCP manager is no longer set due to Session Isolation architecture
        // All external server management is now per-session through MCPServiceProxyManager

        info!("✅ Session-Isolated MCP architecture initialized");

        // Initialize the MCP Service Proxy Manager for session-aware builtin tools
        use mcp::MCPServiceProxyManager;

        // For shared ownership, MCPServiceProxyManager needs Arc-wrapped dependencies
        // We'll modify the state management to use Arc storage pattern
        let proxy_manager = MCPServiceProxyManager::new_from_static_refs();

        set_mcp_service_proxy_manager(proxy_manager);

        info!("✅ MCP Service Proxy Manager initialized");
    });

    // Call the main run function
    run();
}

/// Configures and runs the main Tauri application.
///
/// This function is the entry point for the application GUI. It sets up:
/// - A custom panic handler for robust error logging.
/// - The Tauri application builder with all necessary plugins (dialog, logging, opener).
/// - The full list of invoke handlers (Tauri commands) available to the frontend.
/// - A setup hook to initialize managed state like `SecureFileManager` and `InteractiveBrowserServer`.
/// - Linux-specific environment variables and checks for WebKit compatibility.
/// - Graceful error handling for panics that may occur during application startup.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up custom panic handler for better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        error!("🚨 PANIC: {panic_info}");
        if let Some(location) = panic_info.location() {
            error!(
                "  Location: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }

        // Attempt graceful shutdown
        error!("🔄 Attempting graceful shutdown...");
    }));

    // Configure Tauri builder with error handling
    let result = std::panic::catch_unwind(|| {
        tauri::Builder::default()
            .plugin(tauri_plugin_http::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_opener::init())
            .invoke_handler(tauri::generate_handler![
                greet,
                list_workspace_files,
                start_mcp_server,
                stop_mcp_server,
                call_mcp_tool,
                sample_from_mcp_server,
                list_mcp_tools,
                list_tools_from_config,
                get_connected_servers,
                check_server_status,
                check_all_servers_status,
                list_all_tools,
                get_validated_tools,
                validate_tool_schema,
                list_builtin_servers,
                list_builtin_tools,
                list_builtin_servers_with_metadata,
                list_available_builtin_server_definitions,
                call_builtin_tool,
                list_all_tools_unified,
                list_all_tools_unified,
                // Download commands
                download_workspace_file,
                export_and_download_zip,
                // Session management commands (still needed for workspace isolation)
                switch_session,
                remove_session,
                delete_content_store,
                get_app_data_dir,
                get_app_logs_dir,
                backup_current_log,
                clear_current_log,
                list_log_files,
                log_trace,
                log_debug,
                log_info,
                log_warn,
                log_error_from_frontend,
                read_file,
                read_dropped_file,
                write_file,
                workspace_write_file,
                open_external_url,
                open_workspace_file_with_default_app,
                open_workspace_in_explorer,
                open_workspace_in_terminal,
                get_workspace_override,
                set_workspace_override,
                cancel_workspace_override,
                // Interactive Browser commands
                create_browser_session,
                close_browser_session,
                list_browser_sessions,
                navigate_to_url,
                browser_script_result,
                browser_page_loaded,
                execute_script,
                navigate_back,
                navigate_forward,
                get_service_context,
                // OAuth 2.1 Authentication commands
                start_oauth_flow,
                complete_oauth_flow,
                has_oauth_token,
                get_oauth_token,
                revoke_oauth_token,
                // Message management commands
                messages_get_page,
                messages_upsert_many,
                messages_upsert,
                messages_delete,
                messages_delete_all_for_session,
                messages_search,
                // Agent workflow commands
                agent_create_session,
                agent_resume_session,
                agent_init_session_with_messages,
                agent_send_message,
                agent_handle_llm_response,
                agent_handle_llm_error,
                agent_handle_tool_result,
                agent_get_session,
                agent_get_tools,
                agent_get_all_sessions,
                agent_delete_session,
                agent_get_available_tools,
                agent_pause_workflow,
                agent_resume_workflow,
                agent_terminate_workflow,
                agent_call_builtin_tool,
                agent_call_builtin_tool,
                agent_get_service_contexts,
                agent_inject_messages,
                agent_clear_all_sessions,
                agent_factory_reset,
                agent_update_session_config,
                agent_create_session_with_initial_message,
                // CRUD Commands
                create_assistant,
                update_assistant,
                delete_assistant,
                list_assistants,
                get_assistant,
                create_mcp_server_config,
                update_mcp_server_config,
                delete_mcp_server_config,
                list_mcp_server_configs,
                create_playbook,
                update_playbook,
                delete_playbook,
                get_playbook,
                list_playbooks,
                toggle_playbook_bookmark,
                set_setting,
                get_setting,
                delete_setting,
                get_setting,
                delete_setting,
                list_settings,
            ])
            .setup(|app| {
                // Setup custom file logger FIRST (before any log calls)
                let log_dir = app.path().app_log_dir()?;
                logger::setup_file_logger(log_dir)?;

                // Test if Rust logger is properly initialized
                log::info!("🔥 Logger initialized - testing Rust log to file");
                info!("🚀 LibrAgent initializing...");

                // Initialize SecureFileManager and add to managed state
                // Use a dedicated global directory for the global instance to avoid legacy session dependency
                let global_file_dir = app.path().app_data_dir()?.join("global_shared");
                let file_manager = SecureFileManager::new_with_base_dir(global_file_dir);
                app.manage(file_manager);
                info!("✅ SecureFileManager initialized");

                // Initialize Interactive Browser Server and add to managed state
                // We need to re-fetch settings or pass them down.
                // Since setup closure doesn't have access to the vars from run_with_sqlite_sync directly easily without passing via state or app handle,
                // BUT run_with_sqlite_sync calls run(), which sets up the builder.
                // The `SystemSettings` fetch happened in the async block BEFORE run().
                // To fetch it here synchronously or async is tricky in setup which is sync-ish or returns Result.
                // Actually setup is Sync.
                // We should re-fetch from DB inside setup? Or pass via App Handle state?
                // `run_with_sqlite_sync` spawns a runtime, waits for it, then calls `run`.
                // The variables `index_freq_mins` etc are lost.

                // Better approach: Store SystemSettings in a State<SystemSettings> during init?
                // Or just fetch from DB again in setup. Since we have connection.
                // But `get_database_connection` relies on global state which IS set.

                let web_action_timeout = {
                    // We can try to fetch from DB using the global connection which should be set by now
                    // But strictly speaking, `setup` runs on main thread.
                    // DB connection is async. We can't easily block on async DB call in sync setup unless we use a runtime.
                    // The runtime used in `run_with_sqlite_sync` is dropped!
                    // But we can create a temporary runtime to fetch config.

                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(async {
                        #[derive(serde::Deserialize, Default)]
                        #[serde(rename_all = "camelCase")]
                        struct SystemSettings {
                            web_action_timeout_seconds: Option<u64>,
                        }

                        use crate::repositories::settings_repository::SettingsRepository;
                        use crate::state::get_settings_repository;

                        let settings_repo = get_settings_repository();
                        match settings_repo.get("systemSettings").await {
                            Ok(Some(model)) => {
                                let s: SystemSettings =
                                    serde_json::from_str(&model.value).unwrap_or_default();
                                std::time::Duration::from_secs(
                                    s.web_action_timeout_seconds.unwrap_or(30),
                                )
                            }
                            _ => std::time::Duration::from_secs(30),
                        }
                    })
                };

                let browser_server =
                    InteractiveBrowserServer::new(app.handle().clone(), web_action_timeout);
                app.manage(browser_server);
                info!(
                    "✅ Interactive Browser Server initialized with timeout: {:?}",
                    web_action_timeout
                );

                // Initialize Agent Runtime State
                // Removed duplicate logging

                // Initialize Agent Session Manager with proxy manager
                // Get static reference and wrap in Arc using the same unsafe pattern
                let proxy_manager_arc = unsafe {
                    let ptr = get_mcp_service_proxy_manager() as *const mcp::MCPServiceProxyManager;
                    let arc: std::sync::Arc<mcp::MCPServiceProxyManager> =
                        std::sync::Arc::from_raw(ptr);
                    let cloned = arc.clone();
                    std::mem::forget(arc);
                    cloned
                };

                // Get session repository as Arc<dyn SessionRepository> for dependency injection
                let session_repo_arc: Arc<dyn repositories::SessionRepository> =
                    Arc::new(state::get_session_repository().clone());

                let agent_session_manager = agent::AgentSessionManager::new(
                    app.handle().clone(),
                    proxy_manager_arc,
                    session_repo_arc,
                );
                app.manage(agent_session_manager);

                // Spawn session recovery in background
                let recovery_manager = app
                    .state::<agent::AgentSessionManager>()
                    .inner()
                    .clone_for_task();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = recovery_manager.recover_sessions().await {
                        log::error!("❌ Session recovery failed: {}", e);
                    }
                });

                info!("🔄 Session recovery initiated in background");

                // Built-in servers are now automatically initialized with SessionManager support
                // via the get_mcp_manager() function when first called.
                info!("✅ Builtin servers initialized with SessionManager support");

                // Setup OAuth deep link handler for libr-agent://oauth/callback
                let app_handle = app.handle().clone();
                app.listen("deep-link://new-url", move |event| {
                    let url_str = event.payload();
                    log::info!("Deep link received: {url_str}");

                    // Parse the deep link URL to extract OAuth callback parameters
                    if url_str.starts_with("libr-agent://oauth/callback") {
                        log::info!("OAuth callback detected: {url_str}");

                        // Emit an event to the frontend with the OAuth callback URL
                        if let Err(e) = app_handle.emit("oauth-callback", url_str) {
                            log::error!("Failed to emit oauth-callback event: {e}");
                        }
                    }
                });
                info!("✅ OAuth deep link handler registered");

                // Linux-specific checks (environment variables are now set in main.rs)
                #[cfg(target_os = "linux")]
                {
                    info!("🐧 Linux detected - WebKit compatibility flags already set in main.rs");

                    // Check if running in a container or other limited graphics environment
                    if std::env::var("container").is_ok() || std::env::var("DISPLAY").is_err() {
                        warn!("⚠️  Warning: Running in limited graphics environment");
                    }
                }

                info!("✅ LibrAgent setup completed successfully");
                Ok(())
            })
            .run(tauri::generate_context!())
    });

    // Handle the result of the application run, exiting with an error code on panic
    match result {
        Ok(app_result) => {
            if let Err(e) = app_result {
                error!("❌ Tauri application error: {e}");
                std::process::exit(1);
            }
        }
        Err(panic_payload) => {
            error!("❌ Application panicked during startup");
            if let Some(panic_str) = panic_payload.downcast_ref::<&str>() {
                error!("   Panic message: {panic_str}");
            } else if let Some(panic_string) = panic_payload.downcast_ref::<String>() {
                error!("   Panic message: {panic_string}");
            }

            warn!("💡 Troubleshooting suggestions:");
            warn!("   1. Check WebKit/GTK installation: sudo apt install libwebkit2gtk-4.1-dev");
            warn!("   2. Update graphics drivers");
            warn!("   3. Set WEBKIT_DISABLE_COMPOSITING_MODE=1");
            warn!("   4. Run in a desktop environment with proper display");

            std::process::exit(1);
        }
    }
}
