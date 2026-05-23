use crate::agent;
use crate::lifecycle::settings::SystemSettings;
use crate::logger;
use crate::repositories;
use crate::repositories::settings_repository::SettingsRepository;
use crate::services::skill_service::{
    LEGACY_SYSTEM_SKILLS_DIR_NAME, MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME, SKILL_FILE_NAME,
    SYSTEM_SKILLS_DIR_NAME, USER_SKILLS_DIR_NAME,
};
use crate::services::{DroppedFileService, InteractiveBrowserServer, SecureFileManager};
use crate::state;
use log::info;
#[cfg(target_os = "linux")]
use log::warn;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{App, Emitter, Listener, Manager};

/// Marker file written into every bundled skill directory in AppData.
/// Used to distinguish bundled skills from user-created ones so that skills
/// removed from the bundle can be cleaned up automatically on the next launch.
const BUNDLED_SKILL_MARKER: &str = ".bundled_skill";
const MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledSkillsManifest {
    schema_version: u32,
    skills: BTreeMap<String, String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct LegacySkillMigrationSummary {
    removed_legacy_snapshot_entries: usize,
    migrated_user_skills: usize,
    skipped_existing_user_skills: usize,
    removed_legacy_root_dir: bool,
}

impl LegacySkillMigrationSummary {
    fn is_noop(&self) -> bool {
        self.removed_legacy_snapshot_entries == 0
            && self.migrated_user_skills == 0
            && self.skipped_existing_user_skills == 0
            && !self.removed_legacy_root_dir
    }
}

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

fn spawn_managed_skills_startup_work(bundled_skills_dir: PathBuf, system_skills_dir: PathBuf) {
    crate::state::begin_managed_skills_sync();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = async {
            match migrate_legacy_skills_to_managed_storage().await {
                Ok(summary) if summary.is_noop() => {
                    log::debug!("No legacy skills migration work was needed");
                }
                Ok(summary) => {
                    info!(
                        "✅ Legacy skills migration completed (removed snapshots: {}, migrated user skills: {}, skipped existing user skills: {}, removed legacy root: {})",
                        summary.removed_legacy_snapshot_entries,
                        summary.migrated_user_skills,
                        summary.skipped_existing_user_skills,
                        summary.removed_legacy_root_dir
                    );
                }
                Err(error) => {
                    log::warn!("⚠️  Failed to migrate legacy skills: {}", error);
                }
            }

            if let Err(error) =
                sync_managed_system_skills_snapshot(&bundled_skills_dir, &system_skills_dir)
            {
                log::warn!("⚠️  Failed to sync managed system skills snapshot: {}", error);
            } else {
                info!("✅ Managed system skills snapshot synchronized");
            }

            crate::services::skill_service::invalidate_skill_scan_cache();

            if let Err(error) = crate::services::skill_service::prewarm_managed_skill_scans().await
            {
                log::warn!("⚠️  Failed to prewarm managed skill cache: {}", error);
            }

            Ok::<(), String>(())
        }
        .await
        {
            log::warn!("⚠️  Managed skills startup preparation finished with warnings: {}", e);
        }

        crate::state::complete_managed_skills_sync();
    });
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

    let _indexing_worker = crate::search::IndexingWorker::new(std::time::Duration::from_secs(
        search_index_frequency_minutes * 60,
    ));
    info!("✅ Background message indexing worker started");
}

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

fn collect_skill_directory_names(skills_dir: &Path) -> Result<BTreeSet<String>, String> {
    if !skills_dir.exists() {
        return Ok(BTreeSet::new());
    }

    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(skills_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }

        names.insert(entry.file_name().to_string_lossy().to_string());
    }

    Ok(names)
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| format!("Failed to strip prefix for {}: {}", path.display(), error))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

fn hash_skill_directory(skill_dir: &Path) -> Result<String, String> {
    let mut files = walkdir::WalkDir::new(skill_dir)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to scan {}: {}", skill_dir.display(), error))?
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let path = entry.into_path();
            let normalized = normalized_relative_path(skill_dir, &path)?;
            Ok((normalized, path))
        })
        .collect::<Result<Vec<_>, String>>()?;

    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative_path, full_path) in files {
        hasher.update(relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(
            std::fs::read(&full_path)
                .map_err(|error| format!("Failed to read {}: {}", full_path.display(), error))?,
        );
        hasher.update([0]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn build_bundled_skills_manifest(skills_dir: &Path) -> Result<BundledSkillsManifest, String> {
    let mut manifest = BundledSkillsManifest {
        schema_version: MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION,
        skills: BTreeMap::new(),
    };

    if !skills_dir.exists() {
        return Ok(manifest);
    }

    let mut entries = std::fs::read_dir(skills_dir)
        .map_err(|error| format!("Failed to read {}: {}", skills_dir.display(), error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read directory entry: {}", error))?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    for entry in entries {
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        if !skill_path.join(SKILL_FILE_NAME).is_file() {
            continue;
        }

        let skill_dir_name = entry.file_name().to_string_lossy().to_string();
        manifest
            .skills
            .insert(skill_dir_name, hash_skill_directory(&skill_path)?);
    }

    Ok(manifest)
}

fn load_persisted_bundled_skills_manifest(
    manifest_path: &Path,
) -> Result<Option<BundledSkillsManifest>, String> {
    if !manifest_path.exists() {
        return Ok(None);
    }

    let payload = std::fs::read(manifest_path)
        .map_err(|error| format!("Failed to read {}: {}", manifest_path.display(), error))?;
    let manifest = match serde_json::from_slice::<BundledSkillsManifest>(&payload) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };

    if manifest.schema_version != MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION {
        return Ok(None);
    }

    Ok(Some(manifest))
}

fn write_manifest_atomically(
    manifest_path: &Path,
    manifest: &BundledSkillsManifest,
) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Failed to serialize manifest: {}", error))?;
    let temp_path = manifest_path.with_extension("json.tmp");
    let backup_path = manifest_path.with_extension("json.bak");

    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write {}: {}", temp_path.display(), error))?;

    if backup_path.exists() {
        std::fs::remove_file(&backup_path).map_err(|error| {
            format!(
                "Failed to clear backup manifest {}: {}",
                backup_path.display(),
                error
            )
        })?;
    }

    let moved_existing_to_backup = if manifest_path.exists() {
        std::fs::rename(manifest_path, &backup_path).map_err(|error| {
            format!(
                "Failed to move existing manifest {} aside: {}",
                manifest_path.display(),
                error
            )
        })?;
        true
    } else {
        false
    };

    match std::fs::rename(&temp_path, manifest_path) {
        Ok(()) => {
            if moved_existing_to_backup {
                std::fs::remove_file(&backup_path).map_err(|error| {
                    format!(
                        "Failed to remove backup manifest {}: {}",
                        backup_path.display(),
                        error
                    )
                })?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            if moved_existing_to_backup && !manifest_path.exists() {
                let _ = std::fs::rename(&backup_path, manifest_path);
            }
            Err(format!(
                "Failed to finalize manifest {}: {}",
                manifest_path.display(),
                error
            ))
        }
    }
}

fn replace_skill_directory_atomically(source_dir: &Path, target_dir: &Path) -> Result<(), String> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| format!("Invalid managed skill path: {}", target_dir.display()))?;
    let skill_name = target_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "Invalid managed skill directory name: {}",
                target_dir.display()
            )
        })?;

    let temp_dir = parent.join(format!(".sync-tmp-{}", skill_name));
    let backup_dir = parent.join(format!(".sync-backup-{}", skill_name));

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).map_err(|error| {
            format!("Failed to clear temp dir {}: {}", temp_dir.display(), error)
        })?;
    }
    if backup_dir.exists() {
        std::fs::remove_dir_all(&backup_dir).map_err(|error| {
            format!(
                "Failed to clear backup dir {}: {}",
                backup_dir.display(),
                error
            )
        })?;
    }

    copy_dir_recursive_path(source_dir, &temp_dir).map_err(|e| e.to_string())?;

    let moved_existing_to_backup = if target_dir.exists() {
        std::fs::rename(target_dir, &backup_dir).map_err(|error| {
            format!(
                "Failed to move existing managed skill {} aside: {}",
                target_dir.display(),
                error
            )
        })?;
        true
    } else {
        false
    };

    match std::fs::rename(&temp_dir, target_dir) {
        Ok(()) => {
            if moved_existing_to_backup {
                std::fs::remove_dir_all(&backup_dir).map_err(|error| {
                    format!(
                        "Failed to remove backup dir {}: {}",
                        backup_dir.display(),
                        error
                    )
                })?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            if moved_existing_to_backup && !target_dir.exists() {
                let _ = std::fs::rename(&backup_dir, target_dir);
            }
            Err(format!(
                "Failed to activate managed skill {}: {}",
                target_dir.display(),
                error
            ))
        }
    }
}

/// Synchronize the managed system skills snapshot with the packaged bundled skills.
///
/// This keeps a runtime-owned app-data mirror instead of treating packaged resources
/// as the live source of truth, while avoiding full delete-and-recopy work when only
/// a subset of bundled skills changed.
pub fn sync_managed_system_skills_snapshot(
    bundled_skills_dir: &std::path::Path,
    system_skills_dir: &std::path::Path,
) -> Result<(), String> {
    if !bundled_skills_dir.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(system_skills_dir).map_err(|e| e.to_string())?;

    let manifest_path = system_skills_dir.join(MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME);
    let source_manifest = build_bundled_skills_manifest(bundled_skills_dir)?;
    let source_skill_names = source_manifest
        .skills
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let installed_skill_names = collect_skill_directory_names(system_skills_dir)?;
    let persisted_manifest = load_persisted_bundled_skills_manifest(&manifest_path)?;
    let installed_manifest = match persisted_manifest.as_ref() {
        Some(manifest) => manifest.clone(),
        None => build_bundled_skills_manifest(system_skills_dir)?,
    };

    if installed_manifest == source_manifest && installed_skill_names == source_skill_names {
        if persisted_manifest.is_none() {
            write_manifest_atomically(&manifest_path, &source_manifest)?;
        }
        return Ok(());
    }

    for obsolete_skill in installed_skill_names.difference(&source_skill_names) {
        std::fs::remove_dir_all(system_skills_dir.join(obsolete_skill)).map_err(|error| {
            format!(
                "Failed to delete obsolete managed skill {}: {}",
                obsolete_skill, error
            )
        })?;
    }

    for (skill_name, source_hash) in &source_manifest.skills {
        let target_dir = system_skills_dir.join(skill_name);
        let needs_update =
            installed_manifest.skills.get(skill_name) != Some(source_hash) || !target_dir.exists();

        if needs_update {
            replace_skill_directory_atomically(&bundled_skills_dir.join(skill_name), &target_dir)?;
        }
    }

    write_manifest_atomically(&manifest_path, &source_manifest)?;
    Ok(())
}

/// Move legacy user-managed skills from AppData/skills into AppData/user_skills.
/// Old `.bundled_skill` snapshot entries are always discarded so the bundled
/// system snapshot can be rebuilt as an exact copy of the current bundle.
pub fn remove_legacy_skills_dir_if_empty(
    legacy_skills_dir: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !legacy_skills_dir.exists() {
        return Ok(false);
    }

    let mut entries = std::fs::read_dir(legacy_skills_dir)?;
    if entries.next().is_some() {
        return Ok(false);
    }

    std::fs::remove_dir(legacy_skills_dir)?;
    Ok(true)
}

async fn migrate_legacy_skills_to_managed_storage(
) -> Result<LegacySkillMigrationSummary, Box<dyn std::error::Error>> {
    use std::fs;

    let base_data_dir = crate::session::get_session_manager()
        .map_err(std::io::Error::other)?
        .get_base_data_dir()
        .clone();
    let legacy_skills_dir = base_data_dir.join(LEGACY_SYSTEM_SKILLS_DIR_NAME);
    let user_skills_dir = base_data_dir.join(USER_SKILLS_DIR_NAME);

    if !legacy_skills_dir.exists() {
        return Ok(LegacySkillMigrationSummary::default());
    }

    fs::create_dir_all(&user_skills_dir)?;
    let mut summary = LegacySkillMigrationSummary::default();

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
            summary.removed_legacy_snapshot_entries += 1;
            continue;
        }

        if target_skill_dir.exists() {
            log::info!(
                "⏭️  Managed user skill already exists, leaving legacy copy untouched: {:?}",
                skill_name
            );
            summary.skipped_existing_user_skills += 1;
            continue;
        }

        match fs::rename(&legacy_skill_dir, &target_skill_dir) {
            Ok(_) => {
                log::info!(
                    "📦 Migrated legacy skill into managed storage: {:?}",
                    skill_name
                );
                summary.migrated_user_skills += 1;
            }
            Err(error) => {
                log::warn!(
                    "Failed to move legacy skill {:?} directly ({}), copying instead",
                    skill_name,
                    error
                );
                copy_dir_recursive(&legacy_skill_dir, &target_skill_dir)?;
                fs::remove_dir_all(&legacy_skill_dir)?;
                summary.migrated_user_skills += 1;
            }
        }
    }

    summary.removed_legacy_root_dir = remove_legacy_skills_dir_if_empty(&legacy_skills_dir)?;

    Ok(summary)
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

    let session_manager = crate::session::get_session_manager()
        .map_err(std::io::Error::other)?
        .clone();
    let resource_dir = app.path().resource_dir()?;
    let bundled_skills_dir = resource_dir.join("bundled_skills");
    let system_skills_dir = session_manager
        .get_base_data_dir()
        .join(SYSTEM_SKILLS_DIR_NAME);
    spawn_managed_skills_startup_work(bundled_skills_dir, system_skills_dir);

    let startup_settings = tauri::async_runtime::block_on(load_startup_settings());

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
        if let Err(e) = crate::server::init(
            std::sync::Arc::new(server_manager),
            startup_settings.http_port,
            startup_settings.http_expose,
            mcp_enabled,
        )
        .await
        {
            log::error!(
                "Failed to start HTTP server on port {}: {}",
                startup_settings.http_port,
                e
            );
        }
    });
    info!(
        "✅ HTTP Server spawned on {}:{}",
        if startup_settings.http_expose {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        },
        startup_settings.http_port
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

    info!("✅ LibrAgent setup completed successfully");
    Ok(())
}
