use log::error;
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_log::{Target, TargetKind};

mod agent;
mod commands;
mod config;
pub mod mcp; // Make public for integration tests
pub mod repositories; // Make public for integration tests
mod search;
mod services;
mod session;
mod session_isolation;
mod state;

use commands::agent_commands::{
    agent_call_builtin_tool, agent_create_session, agent_delete_session, agent_get_all_sessions,
    agent_get_service_contexts, agent_get_session, agent_handle_llm_error,
    agent_handle_llm_response, agent_handle_tool_result, agent_pause_workflow,
    agent_resume_workflow, agent_send_message, agent_terminate_workflow,
};
use commands::browser_commands::*;
use commands::content_store_commands::delete_content_store;
use commands::download_commands::{download_workspace_file, export_and_download_zip};
use commands::file_commands::{read_dropped_file, read_file, workspace_write_file, write_file};
use commands::log_commands::{backup_current_log, clear_current_log, list_log_files};
use commands::mcp_commands::{
    call_builtin_tool, call_mcp_tool, check_all_servers_status, check_server_status,
    complete_oauth_flow, get_connected_servers, get_oauth_token, get_service_context,
    get_validated_tools, has_oauth_token, list_all_tools, list_all_tools_unified,
    list_builtin_servers, list_builtin_servers_with_metadata, list_builtin_tools, list_mcp_tools,
    list_tools_from_config, revoke_oauth_token, sample_from_mcp_server, start_mcp_server,
    start_oauth_flow, stop_mcp_server, switch_context, validate_tool_schema,
};
use commands::messages_commands::{
    messages_delete, messages_delete_all_for_session, messages_get_page, messages_search,
    messages_upsert, messages_upsert_many,
};
use commands::session_commands::{
    cleanup_sessions, create_session, fast_session_switch, get_current_session_info,
    get_current_session_legacy, get_isolation_capabilities, get_session_stats,
    get_session_workspace_dir, list_all_sessions, list_sessions_legacy, pre_allocate_sessions,
    remove_session, set_current_session, switch_session,
};
use commands::url_commands::open_external_url;
use commands::workspace_commands::{
    get_app_data_dir, get_app_logs_dir, greet, list_workspace_files,
};
use mcp::MCPServerManager;
use services::{InteractiveBrowserServer, SecureFileManager};
use session::get_session_manager;

// Re-export state management functions
pub use state::{
    get_content_store_repository, get_mcp_manager, get_mcp_service_proxy_manager,
    get_message_repository, get_session_repository, get_sqlite_db_url, get_sqlite_pool,
    set_content_store_repository, set_mcp_manager, set_mcp_service_proxy_manager,
    set_message_repository, set_session_repository, set_sqlite_db_url, set_sqlite_pool,
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
    println!("🔄 Initializing LibrAgent with SQLite support: {db_url}");

    // Create a Tokio runtime for async initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let session_manager = get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = std::sync::Arc::new(session_manager.clone());

        // Initialize the SQLite connection pool. If the database file was
        // removed (for example during testing), try to create the file and
        // retry the connection once before failing the startup.
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
        use std::str::FromStr;
        use std::time::Duration;

        let options = SqliteConnectOptions::from_str(&db_url)
            .expect("Invalid database URL")
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        let pool = match SqlitePoolOptions::new().connect_with(options.clone()).await {
            Ok(p) => p,
            Err(e) => {
                // If this looks like a file-backed sqlite URL, try to create the file
                if let Some(path) = db_url.strip_prefix("sqlite://") {
                    println!("⚙️ SQLite connect failed, attempting to create DB file: {path}");
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        if let Err(err) = std::fs::create_dir_all(parent) {
                            eprintln!("Failed to create parent directory for DB: {err}");
                        }
                    }

                    match std::fs::File::create(path) {
                        Ok(_) => println!("✅ Created new SQLite DB file: {path}"),
                        Err(err) => eprintln!("Failed to create SQLite DB file: {err}"),
                    }

                    // Retry connection once
                    SqlitePoolOptions::new()
                        .connect_with(options)
                        .await
                        .unwrap_or_else(|err| {
                            panic!(
                                "Failed to connect to SQLite database after creating file: {err}"
                            )
                        })
                } else {
                    panic!("Failed to connect to SQLite database: {e}");
                }
            }
        };

        // Initialize repository instances
        use repositories::{
            MessageRepository, SessionRepository, SqliteContentStoreRepository,
            SqliteMessageRepository, SqliteSessionRepository,
        };

        let message_repo = SqliteMessageRepository::new(pool.clone());
        message_repo
            .create_table()
            .await
            .expect("Failed to create messages table");
        println!("✅ Messages table initialized");

        let content_store_repo = SqliteContentStoreRepository::new(pool.clone());
        let session_repo = SqliteSessionRepository::new(pool.clone());
        session_repo
            .create_table()
            .await
            .expect("Failed to create sessions table");
        println!("✅ Sessions table initialized");

        // Start background indexing worker (checks every 5 minutes)
        let _indexing_worker = search::IndexingWorker::new(std::time::Duration::from_secs(300));
        println!("✅ Background message indexing worker started");

        // Set the global SQLite pool
        set_sqlite_pool(pool);
        println!("✅ SQLite connection pool initialized");

        // Set the global repository instances
        set_message_repository(message_repo);
        set_content_store_repository(content_store_repo);
        set_session_repository(session_repo);
        println!("✅ Repository instances initialized");

        // Initialize the MCP manager asynchronously
        let mcp_manager = MCPServerManager::new_with_session_manager_and_sqlite(
            session_manager_arc.clone(),
            db_url.clone(),
        )
        .await;

        // Set the global MCP manager
        set_mcp_manager(mcp_manager);

        println!("✅ SQLite-backed MCP Manager initialized");

        // Initialize the MCP Service Proxy Manager for session-aware builtin tools
        use mcp::MCPServiceProxyManager;

        // For shared ownership, MCPServiceProxyManager needs Arc-wrapped dependencies
        // We'll modify the state management to use Arc storage pattern
        let proxy_manager = MCPServiceProxyManager::new_from_static_refs();

        set_mcp_service_proxy_manager(proxy_manager);

        println!("✅ MCP Service Proxy Manager initialized");
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
            .plugin(
                tauri_plugin_log::Builder::default()
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::LogDir {
                            file_name: Some("libragent".to_string()),
                        }),
                        Target::new(TargetKind::Webview),
                    ])
                    .level(log::LevelFilter::Trace)
                    .build(),
            )
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
                call_builtin_tool,
                list_all_tools_unified,
                list_all_tools_unified,
                // Download commands
                download_workspace_file,
                export_and_download_zip,
                // Session management commands (legacy)
                set_current_session,
                get_current_session_legacy,
                get_session_workspace_dir,
                list_sessions_legacy,
                // Enhanced session management commands
                switch_session,
                create_session,
                get_current_session_info,
                list_all_sessions,
                get_session_stats,
                pre_allocate_sessions,
                cleanup_sessions,
                remove_session,
                delete_content_store,
                get_isolation_capabilities,
                fast_session_switch,
                get_app_data_dir,
                get_app_logs_dir,
                backup_current_log,
                clear_current_log,
                list_log_files,
                read_file,
                read_dropped_file,
                write_file,
                workspace_write_file,
                open_external_url,
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
                switch_context,
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
                agent_send_message,
                agent_handle_llm_response,
                agent_handle_llm_error,
                agent_handle_tool_result,
                agent_get_session,
                agent_get_all_sessions,
                agent_delete_session,
                agent_pause_workflow,
                agent_resume_workflow,
                agent_terminate_workflow,
                agent_call_builtin_tool,
                agent_get_service_contexts
            ])
            .setup(|app| {
                println!("🚀 LibrAgent initializing...");

                // Initialize SecureFileManager and add to managed state
                let file_manager = SecureFileManager::new();
                app.manage(file_manager);
                println!("✅ SecureFileManager initialized");

                // Initialize Interactive Browser Server and add to managed state
                let browser_server = InteractiveBrowserServer::new(app.handle().clone());
                app.manage(browser_server);
                println!("✅ Interactive Browser Server initialized");

                // Initialize Agent Runtime State
                println!("✅ Interactive Browser Server initialized");

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

                let agent_session_manager =
                    agent::AgentSessionManager::new(app.handle().clone(), proxy_manager_arc);

                // Recover sessions on app startup
                let manager_for_recovery = agent_session_manager.clone_for_task();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = manager_for_recovery.recover_sessions().await {
                        log::error!("Failed to recover sessions on startup: {}", e);
                    }
                });

                app.manage(agent_session_manager);
                println!("✅ Agent Session Manager initialized with proxy manager");
                println!("🔄 Session recovery initiated in background");

                // Built-in servers are now automatically initialized with SessionManager support
                // via the get_mcp_manager() function when first called.
                println!("✅ Builtin servers initialized with SessionManager support");

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
                println!("✅ OAuth deep link handler registered");

                // Linux-specific checks (environment variables are now set in main.rs)
                #[cfg(target_os = "linux")]
                {
                    println!(
                        "🐧 Linux detected - WebKit compatibility flags already set in main.rs"
                    );

                    // Check if running in a container or other limited graphics environment
                    if std::env::var("container").is_ok() || std::env::var("DISPLAY").is_err() {
                        eprintln!("⚠️  Warning: Running in limited graphics environment");
                    }
                }

                println!("✅ LibrAgent setup completed successfully");
                Ok(())
            })
            .run(tauri::generate_context!())
    });

    // Handle the result of the application run, exiting with an error code on panic
    match result {
        Ok(app_result) => {
            if let Err(e) = app_result {
                eprintln!("❌ Tauri application error: {e}");
                std::process::exit(1);
            }
        }
        Err(panic_payload) => {
            eprintln!("❌ Application panicked during startup");
            if let Some(panic_str) = panic_payload.downcast_ref::<&str>() {
                eprintln!("   Panic message: {panic_str}");
            } else if let Some(panic_string) = panic_payload.downcast_ref::<String>() {
                eprintln!("   Panic message: {panic_string}");
            }

            eprintln!("💡 Troubleshooting suggestions:");
            eprintln!(
                "   1. Check WebKit/GTK installation: sudo apt install libwebkit2gtk-4.1-dev"
            );
            eprintln!("   2. Update graphics drivers");
            eprintln!("   3. Set WEBKIT_DISABLE_COMPOSITING_MODE=1");
            eprintln!("   4. Run in a desktop environment with proper display");

            std::process::exit(1);
        }
    }
}
