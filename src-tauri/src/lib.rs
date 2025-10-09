use log::error;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

mod commands;
mod mcp;
mod search;
mod services;
mod session;
mod session_isolation;
mod state;

use commands::browser_commands::*;
use commands::content_store_commands::delete_content_store;
use commands::download_commands::{download_workspace_file, export_and_download_zip};
use commands::file_commands::{read_dropped_file, read_file, workspace_write_file, write_file};
use commands::log_commands::{backup_current_log, clear_current_log, list_log_files};
use commands::mcp_commands::{
    call_builtin_tool, call_mcp_tool, call_tool_unified, check_all_servers_status,
    check_server_status, get_connected_servers, get_service_context, get_validated_tools,
    list_all_tools, list_all_tools_unified, list_builtin_servers, list_builtin_tools,
    list_mcp_tools, list_tools_from_config, sample_from_mcp_server, start_mcp_server,
    stop_mcp_server, switch_context, validate_tool_schema,
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
    get_mcp_manager, get_sqlite_db_url, get_sqlite_pool, set_mcp_manager, set_sqlite_db_url,
    set_sqlite_pool,
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
    println!("🔄 Initializing SynapticFlow with SQLite support: {db_url}");

    // Create a Tokio runtime for async initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let session_manager = get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = std::sync::Arc::new(session_manager.clone());

        // Initialize the SQLite connection pool
        let pool = sqlx::sqlite::SqlitePool::connect(&db_url)
            .await
            .expect("Failed to connect to SQLite database");

        // Initialize messages table
        commands::messages_commands::db::create_messages_table(&pool)
            .await
            .expect("Failed to create messages table");
        println!("✅ Messages table initialized");

        // Start background indexing worker (checks every 5 minutes)
        let _indexing_worker = search::IndexingWorker::new(std::time::Duration::from_secs(300));
        println!("✅ Background message indexing worker started");

        // Set the global SQLite pool
        set_sqlite_pool(pool);
        println!("✅ SQLite connection pool initialized");

        // Initialize the MCP manager asynchronously
        let mcp_manager =
            MCPServerManager::new_with_session_manager_and_sqlite(session_manager_arc, db_url)
                .await;

        // Set the global MCP manager
        set_mcp_manager(mcp_manager);

        println!("✅ SQLite-backed MCP Manager initialized");
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
            .plugin(tauri_plugin_dialog::init())
            .plugin(
                tauri_plugin_log::Builder::default()
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::LogDir {
                            file_name: Some("synaptic-flow".to_string()),
                        }),
                        Target::new(TargetKind::Webview),
                    ])
                    .level(log::LevelFilter::Info)
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
                call_builtin_tool,
                list_all_tools_unified,
                call_tool_unified,
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
                click_element,
                input_text,
                scroll_page,
                get_current_url,
                get_page_title,
                element_exists,
                list_browser_sessions,
                navigate_to_url,
                get_page_content,
                take_screenshot,
                browser_script_result,
                execute_script,
                poll_script_result,
                navigate_back,
                navigate_forward,
                get_element_text,
                get_element_attribute,
                find_element,
                get_service_context,
                switch_context,
                // Message management commands
                messages_get_page,
                messages_upsert_many,
                messages_upsert,
                messages_delete,
                messages_delete_all_for_session,
                messages_search
            ])
            .setup(|app| {
                println!("🚀 SynapticFlow initializing...");

                // Initialize SecureFileManager and add to managed state
                let file_manager = SecureFileManager::new();
                app.manage(file_manager);
                println!("✅ SecureFileManager initialized");

                // Initialize Interactive Browser Server and add to managed state
                let browser_server = InteractiveBrowserServer::new(app.handle().clone());
                app.manage(browser_server);
                println!("✅ Interactive Browser Server initialized");

                // Built-in servers are now automatically initialized with SessionManager support
                // via the get_mcp_manager() function when first called.
                println!("✅ Builtin servers initialized with SessionManager support");

                // Perform safety checks for WebView creation on Linux
                #[cfg(target_os = "linux")]
                {
                    println!("🐧 Linux detected - checking WebKit compatibility...");

                    // Set environment variables for better WebKit compatibility on some systems
                    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
                    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

                    // Check if running in a container or other limited graphics environment
                    if std::env::var("container").is_ok() || std::env::var("DISPLAY").is_err() {
                        eprintln!("⚠️  Warning: Running in limited graphics environment");
                    }
                }

                println!("✅ SynapticFlow setup completed successfully");
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
