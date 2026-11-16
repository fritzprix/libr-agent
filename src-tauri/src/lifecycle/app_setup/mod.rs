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
use crate::logger;
use crate::models::system_settings::SystemSettings;
use crate::repositories;
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
    use crate::state::get_settings_repository;

    let settings_repo = get_settings_repository();
    let system_settings = SystemSettings::load(settings_repo).await;

    crate::utils::keep_awake::set_user_preference(system_settings.prevent_sleep_during_agent_work);

    let advanced_settings =
        crate::models::advanced_settings::AdvancedSettings::load(settings_repo).await;

    StartupSettingsSnapshot {
        web_action_timeout: std::time::Duration::from_secs(
            system_settings.web_action_timeout_seconds,
        ),
        http_port: system_settings.http_server_port,
        http_expose: system_settings.http_server_expose,
        search_index_frequency_minutes: system_settings.search_index_frequency_minutes,
        active_agents: advanced_settings.max_concurrent_active_sessions,
        suspended_agents: advanced_settings.max_suspended_sessions,
        active_processes: advanced_settings.max_concurrent_active_processes,
        suspended_processes: advanced_settings.max_suspended_processes,
    }
}

fn spawn_startup_maintenance_tasks(
    session_manager: crate::session::SessionManager,
    search_index_frequency_minutes: u64,
) {
    // Sweep MCP secrets pending deletes if any exist from a keychain outage (#2588).
    tauri::async_runtime::spawn(async move {
        crate::mcp::secret_fallback::sweep_pending_deletes_at_startup().await;
    });

    tauri::async_runtime::spawn(async move {
        match session_manager.cleanup_old_sessions(24, 5).await {
            Ok(count) => info!(
                "🧹 Session cleanup completed: removed {} old sessions",
                count
            ),
            Err(error) => log::error!("❌ Session cleanup failed: {}", error),
        }
    });

    // The sweep removes every labelled container absent from this instance's DB, so an
    // instance with an overridden data dir (e.g. an isolated bench instance) must not run it
    // against the Docker daemon it shares with the primary instance.
    if should_sweep_stale_containers(std::env::var_os("LIBRAGENT_DATA_DIR").as_deref()) {
        tauri::async_runtime::spawn(async move {
            let session_repo = crate::state::get_session_repository();
            let active_session_ids = match session_repo.get_all_session_ids().await {
                Ok(ids) => ids.into_iter().collect::<HashSet<_>>(),
                Err(error) => {
                    log::warn!(
                        "Failed to load sessions for Docker stale container sweep: {}",
                        error
                    );
                    return;
                }
            };

            match crate::services::WorkspaceRuntimeManager::sweep_stale_containers(
                &active_session_ids,
            )
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
    } else {
        info!("🧹 Docker stale container sweep skipped: LIBRAGENT_DATA_DIR override in effect");
    }

    let _indexing_worker = crate::search::IndexingWorker::new(std::time::Duration::from_secs(
        search_index_frequency_minutes * 60,
    ));
    info!("✅ Background message indexing worker started");
}

/// Startup session recovery with the multi-backend guard (#2855 G1.1 / G2.1).
///
/// When another live backend already serves the same profile, recovery would
/// reset that backend's Busy/Queued sessions — mark `Skipped` and return
/// without calling `recover`. Temporary until #2856 ownership replaces this.
///
/// Two GUIs that start in the same grace window may each see the other as
/// booting and both skip, leaving prior crash residue as Busy (rare; #2856).
pub(crate) async fn run_startup_recovery<R, Fut, M>(has_other_backend: bool, recover: R, mark: M)
where
    R: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
    M: FnOnce(crate::server::readiness::RecoveryState),
{
    use crate::server::readiness::RecoveryState;

    if has_other_backend {
        mark(RecoveryState::Skipped);
        return;
    }

    match recover().await {
        Ok(()) => mark(RecoveryState::Ok),
        Err(_) => mark(RecoveryState::Failed),
    }
}

/// One GUI's in-process backend hosting sequence (#2853). Steps are
/// injected so tests can pin the contract: the HTTP bind runs first;
/// recovery and the scheduler start only after the bind succeeds (#2823);
/// the own-record publish wait runs last and its failure fails startup.
pub(crate) async fn run_gui_host_startup<B, R, S, W>(
    bind: B,
    start_recovery: R,
    start_scheduler: S,
    publish_wait: W,
) -> Result<(), String>
where
    B: std::future::Future<Output = Result<u16, String>>,
    R: FnOnce(),
    S: FnOnce(),
    W: std::future::Future<Output = Result<(), String>>,
{
    bind.await?;
    start_recovery();
    start_scheduler();
    publish_wait.await
}

fn should_sweep_stale_containers(data_dir_override: Option<&std::ffi::OsStr>) -> bool {
    data_dir_override.is_none_or(|value| value.is_empty())
}

pub fn setup_app(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    // Setup custom file logger FIRST (before any log calls)
    let log_dir = app.path().app_log_dir()?;
    let log_retention_days =
        tauri::async_runtime::block_on(SystemSettings::load_global()).log_retention_days;
    // Only the first backend on this profile may rename the shared log;
    // a second host must not rotate the first one's open log (#2823, #2855).
    let rotate_log = crate::server::backend_descriptor::find_live_backend().is_none()
        && !crate::server::backend_descriptor::profile_has_other_backend();
    logger::setup_file_logger_with_rotation(log_dir, log_retention_days, rotate_log)?;

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
    // App chrome control (`POST /mcp/control`) — requires MCP HTTP plus explicit opt-in
    let app_control_enabled = std::env::var("LIBRAGENT_APP_CONTROL").is_ok()
        || std::env::args().any(|a| a == "--app-control");
    if app_control_enabled && !mcp_enabled {
        log::warn!(
            "LIBRAGENT_APP_CONTROL / --app-control set but MCP HTTP is off; enable --mcp or LIBRAGENT_MCP_ENABLE for /mcp/control"
        );
    }

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

    // Get repositories as Arc<dyn Trait> for dependency injection
    let session_repo_arc: Arc<dyn repositories::SessionRepository> =
        Arc::new((*state::get_session_repository()).clone());
    let message_repo_arc: Arc<dyn repositories::message_repository::MessageRepository> =
        Arc::new((*state::get_message_repository()).clone());
    let pending_queue_repo_arc: Arc<dyn repositories::PendingQueueRepository> =
        Arc::new((*state::get_pending_queue_repository()).clone());
    let compact_context_repo_arc: Arc<dyn repositories::CompactContextRepository> =
        Arc::new((*state::get_compact_context_repository()).clone());

    let repositories = agent::AgentRepositories::new(session_repo_arc)
        .with_message(message_repo_arc)
        .with_pending_queue(pending_queue_repo_arc)
        .with_compact_context(compact_context_repo_arc);

    let agent_session_manager = agent::AgentSessionManager::new_with_repositories(
        app.handle().clone(),
        proxy_manager_arc,
        repositories,
        Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    );

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

    // Each GUI hosts its own backend (#2853). A new window starts its own
    // HTTP server, scheduler, and session recovery. It does not attach to
    // another process. HTTP bind runs first. Recovery and the scheduler
    // start only after it succeeds, so a refused or failed bind cannot leave
    // an executor running (#2823).
    let server_manager = app
        .state::<agent::AgentSessionManager>()
        .inner()
        .clone_for_task();
    let recovery_manager = app
        .state::<agent::AgentSessionManager>()
        .inner()
        .clone_for_task();
    let app_handle = app.handle().clone();
    let http_port = startup_settings.http_port;
    let http_expose = startup_settings.http_expose;
    let search_minutes = startup_settings.search_index_frequency_minutes;
    let host_backend = run_gui_host_startup(
        async move {
            let server_manager = std::sync::Arc::new(server_manager);
            crate::server::wake_listener::install(server_manager.clone());
            match crate::server::init(
                server_manager,
                http_port,
                http_expose,
                mcp_enabled,
                app_control_enabled,
            )
            .await
            {
                Ok(actual_port) => {
                    info!(
                        "✅ HTTP Server spawned on {}:{} (mcp={} app_control={})",
                        if http_expose { "0.0.0.0" } else { "127.0.0.1" },
                        actual_port,
                        mcp_enabled,
                        app_control_enabled
                    );
                    Ok(actual_port)
                }
                Err(e) => {
                    let message = e.to_string();
                    log::error!(
                        "Failed to start HTTP server on port range starting from {}: {}",
                        http_port,
                        message
                    );
                    Err(message)
                }
            }
        },
        move || {
            spawn_startup_maintenance_tasks(session_manager, search_minutes);
            tauri::async_runtime::spawn(async move {
                use crate::server::readiness::mark_recovery;

                let has_other = crate::server::backend_descriptor::profile_has_other_backend();
                if has_other {
                    info!(
                        "Skipping session recovery: another live backend is on this profile (#2855; replaced by #2856)"
                    );
                }
                run_startup_recovery(
                    has_other,
                    || async {
                        recovery_manager.recover_sessions().await.map_err(|e| {
                            log::error!("❌ Session recovery failed: {}", e);
                            e.to_string()
                        })
                    },
                    mark_recovery,
                )
                .await;
            });
            info!("🔄 Session recovery initiated in background");
        },
        move || {
            let scheduler_worker = crate::scheduled::SchedulerWorker::new(
                app_handle.clone(),
                std::time::Duration::from_secs(60),
            );
            app_handle.manage(scheduler_worker);
            info!("✅ Scheduled task worker started");
        },
        async {
            tokio::task::spawn_blocking(|| {
                crate::server::backend_descriptor::wait_until_own_record_live(
                    crate::server::shared_backend::OWN_PUBLISH_TIMEOUT,
                )
            })
            .await
            .map_err(|err| format!("backend publish wait panicked: {err}"))?
        },
    );

    // Setup fails when this process never publishes a live record (bind
    // failure or timeout): a started-but-unreachable backend is never
    // reported as ready (#2823). Every GUI hosts its own backend now, so a
    // second GUI window is a normal startup, not a refusal (#2855).
    if let Err(message) = tauri::async_runtime::block_on(host_backend) {
        return Err(std::io::Error::other(message).into());
    }

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

#[cfg(test)]
mod gui_host_policy;

#[cfg(test)]
mod sweep_rule_tests {
    use super::should_sweep_stale_containers;
    use std::ffi::OsStr;

    #[test]
    fn sweeps_only_without_data_dir_override() {
        assert!(should_sweep_stale_containers(None));
        assert!(should_sweep_stale_containers(Some(OsStr::new(""))));
        assert!(!should_sweep_stale_containers(Some(OsStr::new(
            "/tmp/bench-instance"
        ))));
    }
}
