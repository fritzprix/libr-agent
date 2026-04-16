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
use std::sync::Arc;
use tauri::{App, Emitter, Listener, Manager};

/// Marker file written into every bundled skill directory in AppData.
/// Used to distinguish bundled skills from user-created ones so that skills
/// removed from the bundle can be cleaned up automatically on the next launch.
const BUNDLED_SKILL_MARKER: &str = ".bundled_skill";

/// Migration decision for a legacy skill directory from AppData/skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySkillMigrationAction {
    DeleteLegacyCopy,
    MigrateToUser,
}

/// Classifies a legacy AppData/skills entry for managed storage migration.
///
/// Any directory marked with `.bundled_skill` is treated as disposable snapshot
/// data from the old global bundled mirror and is never migrated into
/// `user_skills`. Only unmarked directories are considered user-managed content.
pub fn classify_legacy_skill_for_managed_storage(
    legacy_skill_dir: &std::path::Path,
) -> Result<LegacySkillMigrationAction, String> {
    if !legacy_skill_dir.is_dir() {
        return Ok(LegacySkillMigrationAction::MigrateToUser);
    }

    if legacy_skill_dir.join(BUNDLED_SKILL_MARKER).exists() {
        Ok(LegacySkillMigrationAction::DeleteLegacyCopy)
    } else {
        Ok(LegacySkillMigrationAction::MigrateToUser)
    }
}

/// Replaces the legacy AppData/skills directory with an exact mirror of the
/// currently bundled skills shipped in application resources.
pub fn sync_legacy_global_skills_to_bundled_snapshot(
    bundled_skills_dir: &std::path::Path,
    legacy_skills_dir: &std::path::Path,
) -> Result<(), String> {
    if legacy_skills_dir.exists() {
        std::fs::remove_dir_all(legacy_skills_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(legacy_skills_dir).map_err(|e| e.to_string())?;

    if !bundled_skills_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(bundled_skills_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        if !source_path.is_dir() {
            continue;
        }

        let target_path = legacy_skills_dir.join(entry.file_name());
        copy_dir_recursive_path(&source_path, &target_path).map_err(|e| e.to_string())?;
        std::fs::write(target_path.join(BUNDLED_SKILL_MARKER), "").map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Move legacy user-managed skills from AppData/skills into AppData/user_skills.
/// Old `.bundled_skill` snapshot entries are always discarded so the legacy
/// global mirror can be rebuilt as an exact copy of the current bundle.
async fn migrate_legacy_skills_to_managed_storage(
    _app: &App,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let base_data_dir = crate::session::get_session_manager()
        .map_err(std::io::Error::other)?
        .get_base_data_dir()
        .clone();
    let legacy_skills_dir = base_data_dir.join("skills");
    let user_skills_dir = base_data_dir.join("user_skills");

    if !legacy_skills_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(&user_skills_dir)?;

    for entry in fs::read_dir(&legacy_skills_dir)? {
        let entry = entry?;
        let skill_name = entry.file_name();
        let legacy_skill_dir = entry.path();
        if !legacy_skill_dir.is_dir() {
            continue;
        }

        let migration_action = classify_legacy_skill_for_managed_storage(&legacy_skill_dir)?;

        let target_skill_dir = user_skills_dir.join(&skill_name);
        if migration_action == LegacySkillMigrationAction::DeleteLegacyCopy {
            log::info!(
                "🧹 Removing legacy bundled skill snapshot entry: {:?}",
                skill_name
            );
            fs::remove_dir_all(&legacy_skill_dir)?;
            continue;
        }

        if target_skill_dir.exists() {
            log::info!(
                "⏭️  Managed user skill already exists, leaving legacy copy untouched: {:?}",
                skill_name
            );
            continue;
        }

        match fs::rename(&legacy_skill_dir, &target_skill_dir) {
            Ok(_) => {
                log::info!(
                    "📦 Migrated legacy skill into managed storage: {:?}",
                    skill_name
                );
            }
            Err(error) => {
                log::warn!(
                    "Failed to move legacy skill {:?} directly ({}), copying instead",
                    skill_name,
                    error
                );
                copy_dir_recursive(&legacy_skill_dir, &target_skill_dir)?;
                fs::remove_dir_all(&legacy_skill_dir)?;
            }
        }
    }

    Ok(())
}

/// Recursively copy directory contents
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    copy_dir_recursive_path(src, dst)
}

fn copy_dir_recursive_path(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
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
    let app_data_dir = app.path().app_data_dir()?;
    let global_file_dir = app_data_dir.join("global_shared");
    let file_manager = SecureFileManager::new_with_base_dir(global_file_dir);
    app.manage(file_manager);
    info!("✅ SecureFileManager initialized");

    // Initialize DroppedFileService
    let dropped_file_service =
        DroppedFileService::new_with_trusted_hidden_root(app_data_dir.clone());
    app.manage(dropped_file_service);
    info!("✅ DroppedFileService initialized");

    // Migrate legacy AppData/skills user content into the managed user_skills directory.
    tauri::async_runtime::block_on(async {
        if let Err(e) = migrate_legacy_skills_to_managed_storage(app).await {
            log::warn!("⚠️  Failed to migrate legacy skills: {}", e);
        } else {
            info!("✅ Legacy skills migration completed");
        }
    });

    // Keep the legacy AppData/skills snapshot aligned with bundled_skills so the
    // app's global skill directory never drifts and missing bundled skills recover automatically.
    if let Err(e) = {
        let resource_dir = app.path().resource_dir()?;
        let bundled_skills_dir = resource_dir.join("bundled_skills");
        let legacy_skills_dir = crate::session::get_session_manager()
            .map_err(std::io::Error::other)?
            .get_base_data_dir()
            .join("skills");
        sync_legacy_global_skills_to_bundled_snapshot(&bundled_skills_dir, &legacy_skills_dir)
    } {
        log::warn!("⚠️  Failed to sync bundled skills snapshot: {}", e);
    } else {
        info!("✅ Bundled skills snapshot synchronized");
    }

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

    // MCP HTTP endpoint is enabled via env var or --mcp CLI flag
    let mcp_enabled =
        std::env::var("LIBRAGENT_MCP_ENABLE").is_ok() || std::env::args().any(|a| a == "--mcp");

    let browser_env = Arc::new(
        crate::services::interactive_browser_server::tauri_env::TauriBrowserEnvironment::new(
            app.handle().clone(),
        ),
    );
    let browser_server = InteractiveBrowserServer::new(browser_env, web_action_timeout);
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

    // SP6: Expose the shared sessions map globally so builtin MCP tools can read
    //      per-session cancellation tokens without Tauri managed-state access.
    crate::state::init_active_sessions(agent_session_manager.active_sessions_arc());

    app.manage(agent_session_manager);

    // SP1 + SP2: Initialize SessionBus and ConcurrencyGate from advanced settings.
    // Read values with fallback to hardcoded defaults when settings are absent.
    tauri::async_runtime::block_on(async {
        use crate::agent::concurrency::{
            ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
            DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
        };
        use crate::agent::session_bus::SessionBus;
        use crate::state::get_settings_repository;

        let (active_agents, suspended_agents, active_procs, suspended_procs) =
            match get_settings_repository().get("advancedSettings").await {
                Ok(Some(model)) => {
                    let json: serde_json::Value =
                        serde_json::from_str(&model.value).unwrap_or_default();
                    let get_u32 = |key: &str, default: u32| -> u32 {
                        json.get(key)
                            .and_then(|v| v.as_u64())
                            .map(|v| v.clamp(1, 256) as u32)
                            .unwrap_or(default)
                    };
                    (
                        get_u32("maxConcurrentActiveSessions", DEFAULT_MAX_ACTIVE_AGENTS),
                        get_u32("maxSuspendedSessions", DEFAULT_MAX_SUSPENDED_AGENTS),
                        get_u32("maxConcurrentActiveProcesses", DEFAULT_MAX_ACTIVE_PROCESSES),
                        get_u32("maxSuspendedProcesses", DEFAULT_MAX_SUSPENDED_PROCESSES),
                    )
                }
                _ => (
                    DEFAULT_MAX_ACTIVE_AGENTS,
                    DEFAULT_MAX_SUSPENDED_AGENTS,
                    DEFAULT_MAX_ACTIVE_PROCESSES,
                    DEFAULT_MAX_SUSPENDED_PROCESSES,
                ),
            };

        crate::state::init_session_bus(SessionBus::new());
        crate::state::init_concurrency_gate(ConcurrencyGate::new(
            active_agents,
            suspended_agents,
            active_procs,
            suspended_procs,
        ));

        info!(
            "✅ ConcurrencyGate initialized: active_agents={} suspended_agents={} \
             active_processes={} suspended_processes={}",
            active_agents, suspended_agents, active_procs, suspended_procs,
        );
    });

    // Initialize global AppHandle for event emission from builtin tools
    crate::state::init_app_handle(app.handle().clone());
    info!("✅ Global AppHandle initialized for event emission");

    // Spawn HTTP Server for External Features
    let server_manager = app
        .state::<agent::AgentSessionManager>()
        .inner()
        .clone_for_task();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::server::init(
            std::sync::Arc::new(server_manager),
            http_port,
            http_expose,
            mcp_enabled,
        )
        .await
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

    // Start scheduled task background worker (polls every 60s).
    // Must be stored in managed state to prevent being dropped when setup_app returns.
    let scheduler_app_handle = app.handle().clone();
    let scheduler_worker = crate::scheduled::SchedulerWorker::new(
        scheduler_app_handle,
        std::time::Duration::from_secs(60),
    );
    app.manage(scheduler_worker);
    info!("✅ Scheduled task worker started");

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

    // Listen for frontend readiness
    app.listen("frontend-ready", |_| {
        info!("🖥️  Frontend signaled readiness");
        crate::lifecycle::frontend_ready::mark_as_ready();
    });
    info!("✅ Frontend readiness listener registered");

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
