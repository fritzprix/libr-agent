mod assistant_skills;
mod managed_skills;
mod skills_manifest;

pub use assistant_skills::sync_assistant_bundled_skills;
pub use managed_skills::{
    classify_legacy_skill_for_managed_storage, remove_legacy_skills_dir_if_empty,
    sync_managed_system_skills_snapshot, LegacySkillMigrationAction,
};
pub use skills_manifest::{
    hash_skill_directory, load_persisted_bundled_skills_manifest,
    replace_skill_directory_atomically, write_manifest_atomically, BundledSkillsManifest,
};

use crate::agent;
use crate::lifecycle::settings::SystemSettings;
use crate::logger;
use crate::repositories;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::SessionRepository;
use crate::services::skill_service::SYSTEM_SKILLS_DIR_NAME;
use crate::services::{DroppedFileService, InteractiveBrowserServer, SecureFileManager};
use crate::state;
use log::info;
#[cfg(target_os = "linux")]
use log::warn;
use managed_skills::spawn_managed_skills_startup_work;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::{App, Emitter, Listener, Manager};

#[derive(Debug, Clone, Copy)]
struct StartupSettingsSnapshot {
    web_action_timeout: std::time::Duration,
    http_port: u16,
    http_expose: bool,
    search_index_frequency_minutes: u64,
    active_agents: u32,
    suspended_agents: u32,
    active_processes: u32,
    suspended_processes: u32,
}

pub struct ManagedSkillsSyncHandle(pub tauri::async_runtime::JoinHandle<()>);

async fn load_startup_settings() -> StartupSettingsSnapshot {
    use crate::agent::concurrency::{
        DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES, DEFAULT_MAX_SUSPENDED_AGENTS,
        DEFAULT_MAX_SUSPENDED_PROCESSES,
    };
    use crate::state::get_settings_repository;

    let settings_repo = get_settings_repository();
    let system_settings = match settings_repo.get("systemSettings").await {
        Ok(Some(model)) => serde_json::from_str::<SystemSettings>(&model.value).unwrap_or_default(),
        _ => SystemSettings::default(),
    };

    crate::utils::keep_awake::set_user_preference(
        system_settings.prevent_sleep_during_agent_work_or_default(),
    );

    let advanced_settings = match settings_repo.get("advancedSettings").await {
        Ok(Some(model)) => {
            serde_json::from_str::<serde_json::Value>(&model.value).unwrap_or_default()
        }
        _ => serde_json::Value::Null,
    };

    let get_u32 = |key: &str, default: u32| -> u32 {
        advanced_settings
            .get(key)
            .and_then(|value| value.as_u64())
            .map(|value| value.clamp(1, 256) as u32)
            .unwrap_or(default)
    };

    StartupSettingsSnapshot {
        web_action_timeout: std::time::Duration::from_secs(
            system_settings.web_action_timeout_seconds.unwrap_or(30),
        ),
        http_port: system_settings.http_server_port.unwrap_or(3030),
        http_expose: system_settings.http_server_expose.unwrap_or(false),
        search_index_frequency_minutes: system_settings.search_index_frequency_minutes.unwrap_or(5),
        active_agents: get_u32("maxConcurrentActiveSessions", DEFAULT_MAX_ACTIVE_AGENTS),
        suspended_agents: get_u32("maxSuspendedSessions", DEFAULT_MAX_SUSPENDED_AGENTS),
        active_processes: get_u32("maxConcurrentActiveProcesses", DEFAULT_MAX_ACTIVE_PROCESSES),
        suspended_processes: get_u32("maxSuspendedProcesses", DEFAULT_MAX_SUSPENDED_PROCESSES),
    }
}

fn spawn_startup_maintenance_tasks(
    session_manager: crate::session::SessionManager,
    search_index_frequency_minutes: u64,
) {
    tauri::async_runtime::spawn(async move {
        match session_manager.cleanup_old_sessions(24, 5).await {
            Ok(count) => info!(
                "🧹 Session cleanup completed: removed {} old sessions",
                count
            ),
            Err(error) => log::error!("❌ Session cleanup failed: {}", error),
        }
    });

    tauri::async_runtime::spawn(async move {
        let session_repo = crate::state::get_session_repository();
        let active_session_ids = match session_repo.get_all_sessions().await {
            Ok(sessions) => sessions
                .into_iter()
                .map(|session| session.id)
                .collect::<HashSet<_>>(),
            Err(error) => {
                log::warn!(
                    "Failed to load sessions for Docker stale container sweep: {}",
                    error
                );
                return;
            }
        };

        match crate::services::WorkspaceRuntimeManager::sweep_stale_containers(&active_session_ids)
            .await
        {
            Ok(removed) if removed.is_empty() => {
                info!("🧹 Docker stale container sweep completed: no stale containers");
            }
            Ok(removed) => {
                info!(
                    "🧹 Docker stale container sweep removed {} container(s): {}",
                    removed.len(),
                    removed.join(", ")
                );
            }
            Err(error) => {
                log::warn!("Docker stale container sweep skipped or failed: {}", error);
            }
        }
    });

    let _indexing_worker = crate::search::IndexingWorker::new(std::time::Duration::from_secs(
        search_index_frequency_minutes * 60,
    ));
    info!("✅ Background message indexing worker started");
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

    let session_manager = crate::session::get_session_manager()
        .map_err(std::io::Error::other)?
        .clone();
    let resource_dir = app.path().resource_dir()?;

    // Ensure default assistants are loaded from the resource directory (Fixes Correctness #2)
    let assistants_start = std::time::Instant::now();
    if let Err(e) = tauri::async_runtime::block_on(
        crate::services::assistant_init::ensure_default_assistants(Some(&resource_dir)),
    ) {
        log::error!("❌ Failed to ensure default assistants from bundle: {}", e);
    }
    crate::state::log_startup_phase(
        "ensure_default_assistants",
        Some(assistants_start.elapsed().as_millis()),
    );

    let base_data_dir = session_manager.get_base_data_dir().clone();
    let bundled_skills_start = std::time::Instant::now();
    if let Err(error) =
        tauri::async_runtime::block_on(sync_assistant_bundled_skills(&resource_dir, &base_data_dir))
    {
        log::warn!("⚠️  Failed to sync assistant bundled skills: {}", error);
    } else {
        crate::services::skill_service::invalidate_skill_scan_cache();
        info!("✅ Assistant bundled skills synchronized");
    }
    crate::state::log_startup_phase(
        "sync_assistant_bundled_skills",
        Some(bundled_skills_start.elapsed().as_millis()),
    );

    let bundled_skills_dir = resource_dir.join("bundled_skills");
    let system_skills_dir = base_data_dir.join(SYSTEM_SKILLS_DIR_NAME);
    let sync_handle = spawn_managed_skills_startup_work(bundled_skills_dir, system_skills_dir);
    app.manage(ManagedSkillsSyncHandle(sync_handle));

    let settings_start = std::time::Instant::now();
    let startup_settings = tauri::async_runtime::block_on(load_startup_settings());
    crate::state::log_startup_phase(
        "load_startup_settings",
        Some(settings_start.elapsed().as_millis()),
    );

    // MCP HTTP endpoint is enabled via env var or --mcp CLI flag
    let mcp_enabled =
        std::env::var("LIBRAGENT_MCP_ENABLE").is_ok() || std::env::args().any(|a| a == "--mcp");

    let browser_server = InteractiveBrowserServer::new(startup_settings.web_action_timeout);
    app.manage(browser_server);
    info!(
        "✅ Interactive Browser Server initialized with timeout: {:?}",
        startup_settings.web_action_timeout
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
    crate::state::init_channel_dispatch_agent(agent_session_manager.clone());

    app.manage(agent_session_manager);

    {
        use crate::agent::concurrency::ConcurrencyGate;
        use crate::agent::session_bus::SessionBus;

        crate::state::init_session_bus(SessionBus::new());
        crate::state::init_concurrency_gate(ConcurrencyGate::new(
            startup_settings.active_agents,
            startup_settings.suspended_agents,
            startup_settings.active_processes,
            startup_settings.suspended_processes,
        ));

        info!(
            "✅ ConcurrencyGate initialized: active_agents={} suspended_agents={} \
             active_processes={} suspended_processes={}",
            startup_settings.active_agents,
            startup_settings.suspended_agents,
            startup_settings.active_processes,
            startup_settings.suspended_processes,
        );
    }

    // Initialize global AppHandle for event emission from builtin tools
    crate::state::init_app_handle(app.handle().clone());
    info!("✅ Global AppHandle initialized for event emission");

    spawn_startup_maintenance_tasks(
        session_manager,
        startup_settings.search_index_frequency_minutes,
    );

    // Spawn HTTP Server for External Features
    let server_manager = app
        .state::<agent::AgentSessionManager>()
        .inner()
        .clone_for_task();
    tauri::async_runtime::spawn(async move {
        match crate::server::init(
            std::sync::Arc::new(server_manager),
            startup_settings.http_port,
            startup_settings.http_expose,
            mcp_enabled,
        )
        .await
        {
            Ok(actual_port) => {
                info!(
                    "✅ HTTP Server spawned on {}:{}",
                    if startup_settings.http_expose {
                        "0.0.0.0"
                    } else {
                        "127.0.0.1"
                    },
                    actual_port
                );
            }
            Err(e) => {
                log::error!(
                    "Failed to start HTTP server on port range starting from {}: {}",
                    startup_settings.http_port,
                    e
                );
            }
        }
    });

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
        if let Some(elapsed_ms) = crate::state::startup_elapsed_ms() {
            info!("⏱️ Startup metric: frontend ready after {}ms", elapsed_ms);
        }
    });
    info!("✅ Frontend readiness listener registered");

    // Linux-specific checks
    #[cfg(target_os = "linux")]
    {
        info!("🐧 Linux detected - running with default WebKit rendering path");
        if std::env::var("container").is_ok() || std::env::var("DISPLAY").is_err() {
            warn!("⚠️  Warning: Running in limited graphics environment");
        }
    }

    // Windows: keep minimize → taskbar behavior reliable (no orphan tray tool-window).
    crate::lifecycle::windows_taskbar::ensure_main_window_taskbar_button(app.handle());

    info!("✅ LibrAgent setup completed successfully");
    Ok(())
}
