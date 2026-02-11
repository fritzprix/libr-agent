use crate::agent;
use crate::logger;
use crate::repositories;
use crate::services::{InteractiveBrowserServer, SecureFileManager};
use crate::state::{self, get_mcp_service_proxy_manager};
use log::{info, warn};
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};

pub fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
    let web_action_timeout = {
        // We can try to fetch from DB using the global connection which should be set by now
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
                    let s: SystemSettings = serde_json::from_str(&model.value).unwrap_or_default();
                    std::time::Duration::from_secs(s.web_action_timeout_seconds.unwrap_or(30))
                }
                _ => std::time::Duration::from_secs(30),
            }
        })
    };

    let browser_server = InteractiveBrowserServer::new(app.handle().clone(), web_action_timeout);
    app.manage(browser_server);
    info!(
        "✅ Interactive Browser Server initialized with timeout: {:?}",
        web_action_timeout
    );

    // Initialize Agent Session Manager with proxy manager
    // Get proxy manager directly as Arc (safe now)
    let proxy_manager_arc = get_mcp_service_proxy_manager();

    // Get session repository as Arc<dyn SessionRepository> for dependency injection
    let session_repo_arc: Arc<dyn repositories::SessionRepository> =
        Arc::new(state::get_session_repository().clone());

    let agent_session_manager = agent::AgentSessionManager::new(
        app.handle().clone(),
        proxy_manager_arc,
        session_repo_arc,
    );
    app.manage(agent_session_manager);

    // Initialize global AppHandle for event emission from builtin tools
    crate::state::init_app_handle(app.handle().clone());
    info!("✅ Global AppHandle initialized for event emission");

    // Spawn HTTP Server for External Features
    let server_manager = app
        .state::<agent::AgentSessionManager>()
        .inner()
        .clone_for_task();
    tauri::async_runtime::spawn(async move {
        // TODO: Make port configurable via settings
        if let Err(e) = crate::server::init(std::sync::Arc::new(server_manager), 3030).await {
            log::error!("Failed to start HTTP server: {}", e);
        }
    });
    info!("✅ HTTP Server spawned on port 3030");

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
}
