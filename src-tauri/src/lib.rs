use log::{error, info, warn};
use tauri::Manager;

use crate::services::InteractiveBrowserServer;

pub mod agent; // pub for integration tests (cancel_logic.rs)
pub mod commands; // Make public for integration tests
mod config;
mod db_schema_validator; // Schema validation for database integrity
pub mod entity; // SeaORM entity definitions
pub mod lifecycle; // New lifecycle module
mod logger; // Custom file logger
pub mod mcp; // Make public for integration tests
pub mod models;
pub mod repositories; // Make public for integration tests
pub mod scheduled; // Cron-backed scheduled task background worker (public for integration tests)
mod search;
pub mod server;
pub mod services;
pub mod session;
pub mod session_isolation;
mod state;
pub mod utils;

// Re-export migration for use in MCP modules
pub use migration;

// Re-export SecureFileManager for integration tests
pub use services::SecureFileManager;

use commands::agent_commands::{
    agent_add_attachment, agent_call_builtin_tool, agent_cancel_workflow, agent_clear_all_sessions,
    agent_create_session, agent_create_session_with_initial_message, agent_delete_attachment,
    agent_delete_session, agent_delete_session_only, agent_execute_ui_tauri_action,
    agent_factory_reset, agent_get_all_sessions, agent_get_available_tools,
    agent_get_compact_context, agent_get_service_contexts, agent_get_session, agent_get_tools,
    agent_handle_compact_error, agent_handle_compact_response, agent_handle_llm_error,
    agent_handle_llm_response, agent_handle_tool_result, agent_init_session_with_messages,
    agent_inject_channel_message, agent_inject_channel_message_auto, agent_inject_messages,
    agent_mark_session_viewed, agent_pause_workflow, agent_respond_channel_permission,
    agent_respond_tool_approval, agent_resume_session, agent_resume_workflow,
    agent_save_compact_context, agent_send_message, agent_set_yolo_mode, agent_terminate_workflow,
    agent_toggle_session_bookmark, agent_update_session_config,
};
use commands::assistant_crud_commands::{
    batch_upsert_assistants, create_assistant, delete_assistant, get_assistant, list_assistants,
    search_assistants, update_assistant,
};
use commands::attachments_commands::delete_attachments;
use commands::browser_commands::*;
use commands::download_commands::{
    download_media_file, download_workspace_file, export_and_download_zip,
};
use commands::file_commands::{
    check_dropped_path_type, read_dropped_file, register_dropped_files, workspace_write_file,
    write_file,
};
use commands::knowledge_commands::{
    delete_global_knowledge, get_global_knowledge_detail, list_global_knowledge,
};
use commands::log_commands::{
    backup_current_log, clear_current_log, get_launch_log_level, list_log_files, log_batch,
    log_debug, log_error_from_frontend, log_info, log_trace, log_warn,
};
use commands::mcp_commands::{
    get_oauth_token, has_oauth_token, list_available_builtin_server_definitions,
    list_builtin_servers, list_builtin_servers_with_metadata, list_builtin_tools, probe_mcp_server,
    revoke_oauth_token, validate_tool_schema,
};
use commands::mcp_server_config_commands::{
    create_mcp_server_config, delete_mcp_server_config, list_mcp_server_configs,
    list_mcp_server_presets, update_mcp_server_config,
};
use commands::messages_commands::{
    messages_delete, messages_delete_all_for_session, messages_get_page, messages_search,
    messages_upsert, messages_upsert_many,
};
use commands::playbook_commands::{
    create_playbook, delete_playbook, get_playbook, list_playbooks, toggle_playbook_bookmark,
    update_playbook,
};
use commands::scheduled_task_commands::{
    create_scheduled_task, delete_scheduled_task, get_scheduled_task, list_scheduled_tasks,
    toggle_scheduled_task, update_scheduled_task,
};
use commands::session_commands::remove_session;
use commands::settings_commands::{
    delete_setting, get_setting, list_settings, set_setting, update_settings,
};
use commands::skill_commands::{
    get_aggregated_skills, get_default_skills_directory, get_managed_skills_overview,
    get_skill_content, list_workspace_file_paths, list_workspace_file_paths_for_path,
    open_skills_directory_in_explorer, scan_skills_directory,
};
use commands::skill_management::{
    copy_global_to_assistant, delete_assistant_skill, delete_user_skill, import_assistant_skills,
    import_user_skills, install_github_skills, preview_github_skill_install,
    preview_user_skill_import, reset_assistant_skills, reset_user_skills,
};
use commands::url_commands::open_external_url;
use commands::workspace_commands::{
    cancel_workspace_override, get_app_data_dir, get_app_logs_dir, get_update_install_capability,
    get_workspace_dir, get_workspace_override, greet, list_workspace_files,
    open_workspace_file_with_default_app, open_workspace_in_explorer, open_workspace_in_terminal,
    read_local_file_as_base64, restart_app, set_workspace_override,
};

// Re-export state management functions
pub use state::{
    get_assistant_repository, get_attachments_repository, get_compact_context_repository,
    get_database_connection, get_knowledge_repository, get_mcp_server_repository,
    get_mcp_service_proxy_manager, get_message_repository, get_planning_repository,
    get_playbook_repository, get_session_repository, get_sqlite_db_url, init_concurrency_gate,
    init_session_bus, set_assistant_repository, set_attachments_repository,
    set_compact_context_repository, set_database_connection, set_knowledge_repository,
    set_mcp_server_repository, set_mcp_service_proxy_manager, set_message_repository,
    set_planning_repository, set_playbook_repository, set_session_repository, set_sqlite_db_url,
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
            .plugin(tauri_plugin_mcp_bridge::init())
            .plugin(tauri_plugin_http::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .invoke_handler(tauri::generate_handler![
                greet,
                restart_app,
                list_workspace_files,
                probe_mcp_server,
                validate_tool_schema,
                list_builtin_servers,
                list_builtin_tools,
                list_builtin_servers_with_metadata,
                list_available_builtin_server_definitions,
                // Download commands
                download_media_file,
                download_workspace_file,
                export_and_download_zip,
                // Session management commands (still needed for workspace isolation)
                remove_session,
                delete_attachments,
                get_app_data_dir,
                get_app_logs_dir,
                get_update_install_capability,
                backup_current_log,
                clear_current_log,
                list_log_files,
                get_launch_log_level,
                log_trace,
                log_debug,
                log_info,
                log_warn,
                log_error_from_frontend,
                log_batch,
                register_dropped_files,
                check_dropped_path_type,
                read_dropped_file,
                write_file,
                workspace_write_file,
                list_global_knowledge,
                get_global_knowledge_detail,
                delete_global_knowledge,
                open_external_url,
                open_workspace_file_with_default_app,
                open_workspace_in_explorer,
                open_workspace_in_terminal,
                get_workspace_override,
                set_workspace_override,
                cancel_workspace_override,
                get_workspace_dir,
                read_local_file_as_base64,
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
                // OAuth 2.1 Authentication commands
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
                agent_execute_ui_tauri_action,
                agent_handle_llm_response,
                agent_handle_llm_error,
                agent_handle_tool_result,
                agent_get_session,
                agent_get_tools,
                agent_get_all_sessions,
                agent_delete_session,
                agent_delete_session_only,
                agent_get_available_tools,
                agent_pause_workflow,
                agent_resume_workflow,
                agent_terminate_workflow,
                agent_cancel_workflow,
                agent_call_builtin_tool,
                agent_add_attachment,
                agent_delete_attachment,
                agent_get_service_contexts,
                agent_inject_messages,
                agent_inject_channel_message,
                agent_inject_channel_message_auto,
                agent_respond_channel_permission,
                agent_clear_all_sessions,
                agent_factory_reset,
                agent_update_session_config,
                agent_create_session_with_initial_message,
                agent_toggle_session_bookmark,
                agent_mark_session_viewed,
                agent_set_yolo_mode,
                agent_respond_tool_approval,
                agent_get_compact_context,
                agent_save_compact_context,
                agent_handle_compact_response,
                agent_handle_compact_error,
                // CRUD Commands
                create_assistant,
                update_assistant,
                delete_assistant,
                list_assistants,
                get_assistant,
                search_assistants,
                batch_upsert_assistants,
                create_mcp_server_config,
                update_mcp_server_config,
                delete_mcp_server_config,
                list_mcp_server_configs,
                list_mcp_server_presets,
                create_playbook,
                update_playbook,
                delete_playbook,
                get_playbook,
                list_playbooks,
                toggle_playbook_bookmark,
                create_scheduled_task,
                list_scheduled_tasks,
                get_scheduled_task,
                update_scheduled_task,
                toggle_scheduled_task,
                delete_scheduled_task,
                set_setting,
                update_settings,
                get_setting,
                delete_setting,
                list_settings,
                scan_skills_directory,
                get_default_skills_directory,
                open_skills_directory_in_explorer,
                get_aggregated_skills,
                get_managed_skills_overview,
                get_skill_content,
                list_workspace_file_paths,
                list_workspace_file_paths_for_path,
                copy_global_to_assistant,
                delete_assistant_skill,
                import_assistant_skills,
                preview_user_skill_import,
                import_user_skills,
                preview_github_skill_install,
                install_github_skills,
                delete_user_skill,
                reset_user_skills,
                reset_assistant_skills,
            ])
            .setup(|app| lifecycle::app_setup::setup_app(app))
            .build(tauri::generate_context!())
            .expect("error while building tauri application")
            .run(|app_handle, event| {
                if let tauri::RunEvent::Exit = event {
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::block_on(async move {
                        if let Some(browser_server) =
                            app_handle_clone.try_state::<InteractiveBrowserServer>()
                        {
                            info!("🚀 App exit detected - initiating explicit browser session cleanup...");
                            if let Err(e) = browser_server.close_all_sessions().await {
                                error!("❌ Failed to cleanup browser sessions on exit: {e}");
                            } else {
                                info!("✅ All browser sessions cleaned up successfully");
                            }
                        }
                    });
                }
            })
    });

    // Handle the result of the application run, exiting with an error code on panic
    match result {
        Ok(_) => {
            info!("✅ Application terminated normally");
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
            warn!("   3. Retry with LIBRAGENT_LINUX_COMPATIBILITY_MODE=1");
            warn!("   4. Run in a desktop environment with proper display");

            std::process::exit(1);
        }
    }
}
