use log::{error, warn};

mod agent;
mod commands;
mod config;
mod db_schema_validator; // Schema validation for database integrity
pub mod entity; // SeaORM entity definitions
pub mod lifecycle; // New lifecycle module
mod logger; // Custom file logger
pub mod mcp; // Make public for integration tests
pub mod repositories; // Make public for integration tests
mod search;
pub mod server;
mod services;
pub mod session;
mod session_isolation;
mod state;
pub mod utils;

// Re-export migration for use in MCP modules
pub use migration;

use commands::agent_commands::{
    agent_call_builtin_tool, agent_cancel_workflow, agent_clear_all_sessions, agent_create_session,
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
use commands::file_commands::{
    read_dropped_file, register_dropped_files, workspace_write_file, write_file,
};
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
use commands::skill_commands::{
    get_aggregated_skills, get_default_skills_directory, open_skills_directory_in_explorer,
    scan_skills_directory,
};
use commands::skill_management::{
    copy_global_to_assistant, delete_assistant_skill, download_global_skills,
    import_assistant_skills, reset_assistant_skills,
};
use commands::url_commands::open_external_url;
use commands::workspace_commands::{
    cancel_workspace_override, get_app_data_dir, get_app_logs_dir, get_workspace_override, greet,
    list_workspace_files, open_workspace_file_with_default_app, open_workspace_in_explorer,
    open_workspace_in_terminal, set_workspace_override,
};

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
    lifecycle::run_with_sqlite_sync(db_url);
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
                register_dropped_files,
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
                agent_cancel_workflow,
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
                list_settings,
                scan_skills_directory,
                get_default_skills_directory,
                open_skills_directory_in_explorer,
                get_aggregated_skills,
                download_global_skills,
                copy_global_to_assistant,
                delete_assistant_skill,
                import_assistant_skills,
                reset_assistant_skills,
            ])
            .setup(|app| lifecycle::app_setup::setup_app(app))
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
