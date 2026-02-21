use crate::agent;
use crate::lifecycle::settings::SystemSettings;
use crate::logger;
use crate::repositories;
use crate::repositories::settings_repository::SettingsRepository;
use crate::services::{DroppedFileService, InteractiveBrowserServer, SecureFileManager};
use crate::state;
use log::info;
#[cfg(target_os = "linux")]
use log::warn;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{App, Emitter, Listener, Manager};

/// Marker file written into every bundled skill directory in AppData.
/// Used to distinguish bundled skills from user-created ones so that skills
/// removed from the bundle can be cleaned up automatically on the next launch.
const BUNDLED_SKILL_MARKER: &str = ".bundled_skill";

/// Copy bundled skills from app resources to AppData/skills directory.
///
/// Rules:
/// - `.force_update` present  → always overwrite existing skill
/// - `.force_update` absent   → copy only if destination doesn't exist (preserves user edits)
/// - Skill has `.bundled_skill` marker but is no longer in bundle → remove (cleanup stale skills)
async fn copy_bundled_skills_to_app_data(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let resource_dir = app.path().resource_dir()?;
    let bundled_skills_dir = resource_dir.join("bundled_skills");

    let app_data_dir = app.path().app_data_dir()?;
    let target_skills_dir = app_data_dir.join("skills");

    if !bundled_skills_dir.exists() {
        log::debug!("No bundled_skills directory found in resources");
        return Ok(());
    }

    fs::create_dir_all(&target_skills_dir)?;

    // Build set of current bundled skill names
    let bundled_names: std::collections::HashSet<std::ffi::OsString> =
        fs::read_dir(&bundled_skills_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();

    // Remove stale bundled skills: present in AppData but no longer in bundle
    for entry in fs::read_dir(&target_skills_dir)? {
        let entry = entry?;
        let skill_name = entry.file_name();
        let target_skill_dir = entry.path();

        if target_skill_dir.is_dir()
            && !bundled_names.contains(&skill_name)
            && target_skill_dir.join(BUNDLED_SKILL_MARKER).exists()
        {
            log::info!("🗑️  Removing stale bundled skill: {:?}", skill_name);
            fs::remove_dir_all(&target_skill_dir)?;
        }
    }

    // Copy / update each bundled skill
    for entry in fs::read_dir(&bundled_skills_dir)? {
        let entry = entry?;
        let skill_name = entry.file_name();
        let source_skill_dir = entry.path();
        let target_skill_dir = target_skills_dir.join(&skill_name);

        let force_update_marker = source_skill_dir.join(".force_update");
        let should_force_update = force_update_marker.exists();

        if should_force_update {
            if target_skill_dir.exists() {
                log::info!("🔄 Force updating skill: {:?}", skill_name);
                fs::remove_dir_all(&target_skill_dir)?;
            } else {
                log::info!("📦 Installing new skill: {:?}", skill_name);
            }
            copy_dir_recursive(&source_skill_dir, &target_skill_dir)?;
        } else if !target_skill_dir.exists() {
            log::info!("📦 Copying bundled skill: {:?}", skill_name);
            copy_dir_recursive(&source_skill_dir, &target_skill_dir)?;
        } else {
            log::debug!("⏭️  Skill already exists, skipping: {:?}", skill_name);
        }

        // Write bundled marker so future runs can identify this as a bundled skill
        let marker_path = target_skill_dir.join(BUNDLED_SKILL_MARKER);
        if !marker_path.exists() {
            fs::write(&marker_path, "")?;
        }
    }

    Ok(())
}

/// Recursively copy directory contents
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

pub fn setup_app(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    // Setup custom file logger FIRST (before any log calls)
    let log_dir = app.path().app_log_dir()?;
    logger::setup_file_logger(log_dir)?;

    // Test if Rust logger is properly initialized
    log::info!("🔥 Logger initialized - testing Rust log to file");
    info!("🚀 LibrAgent initializing...");

    // Initialize SecureFileManager and add to managed state
    let global_file_dir = app.path().app_data_dir()?.join("global_shared");
    let file_manager = SecureFileManager::new_with_base_dir(global_file_dir);
    app.manage(file_manager);
    info!("✅ SecureFileManager initialized");

    // Initialize DroppedFileService
    let dropped_file_service = DroppedFileService::new();
    app.manage(dropped_file_service);
    info!("✅ DroppedFileService initialized");

    // Copy bundled skills to AppData/skills if not already present
    tauri::async_runtime::block_on(async {
        if let Err(e) = copy_bundled_skills_to_app_data(app).await {
            log::warn!("⚠️  Failed to copy bundled skills: {}", e);
        } else {
            info!("✅ Bundled skills initialized");
        }
    });

    // Fetch System Settings
    let (web_action_timeout, http_port, http_expose) = tauri::async_runtime::block_on(async {
        use crate::state::get_settings_repository;
        let settings_repo = get_settings_repository();
        match settings_repo.get("systemSettings").await {
            Ok(Some(model)) => {
                let s: SystemSettings = serde_json::from_str(&model.value).unwrap_or_default();
                (
                    std::time::Duration::from_secs(s.web_action_timeout_seconds.unwrap_or(30)),
                    s.http_server_port.unwrap_or(3030),
                    s.http_server_expose.unwrap_or(false),
                )
            }
            _ => (std::time::Duration::from_secs(30), 3030, false),
        }
    });

    let browser_server = InteractiveBrowserServer::new(app.handle().clone(), web_action_timeout);
    app.manage(browser_server);
    info!(
        "✅ Interactive Browser Server initialized with timeout: {:?}",
        web_action_timeout
    );

    // Initialize Agent Session Manager with proxy manager
    // Get proxy manager directly as Arc (safe now)
    use crate::state::get_mcp_service_proxy_manager;
    let proxy_manager_arc = get_mcp_service_proxy_manager();

    // Get session repository as Arc<dyn SessionRepository> for dependency injection
    let session_repo_arc: Arc<dyn repositories::SessionRepository> =
        Arc::new(state::get_session_repository().clone());

    let agent_session_manager =
        agent::AgentSessionManager::new(app.handle().clone(), proxy_manager_arc, session_repo_arc);
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
        if let Err(e) =
            crate::server::init(std::sync::Arc::new(server_manager), http_port, http_expose).await
        {
            log::error!("Failed to start HTTP server on port {}: {}", http_port, e);
        }
    });
    info!(
        "✅ HTTP Server spawned on {}:{}",
        if http_expose { "0.0.0.0" } else { "127.0.0.1" },
        http_port
    );

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
    info!("✅ Builtin servers initialized with SessionManager support");

    // Setup OAuth deep link handler
    let app_handle = app.handle().clone();
    app.listen("deep-link://new-url", move |event| {
        let url_str = event.payload();
        log::info!("Deep link received: {url_str}");

        if url_str.starts_with("libr-agent://oauth/callback") {
            log::info!("OAuth callback detected: {url_str}");
            if let Err(e) = app_handle.emit("oauth-callback", url_str) {
                log::error!("Failed to emit oauth-callback event: {e}");
            }
        }
    });
    info!("✅ OAuth deep link handler registered");

    // Linux-specific checks
    #[cfg(target_os = "linux")]
    {
        info!("🐧 Linux detected - WebKit compatibility flags already set in main.rs");
        if std::env::var("container").is_ok() || std::env::var("DISPLAY").is_err() {
            warn!("⚠️  Warning: Running in limited graphics environment");
        }
    }

    info!("✅ LibrAgent setup completed successfully");
    Ok(())
}
