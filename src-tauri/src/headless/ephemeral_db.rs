//! Ephemeral database snapshot and initial settings resolution for `liblet` (#2703).
//!
//! Each headless run creates an isolated `TempDir`, snapshots the profile's
//! SQLite database read-only via `rusqlite::backup::Backup`, and clears all
//! persisted session/message/task tables in the ephemeral copy. This guarantees:
//! 1. The headless run inherits `settings`, `assistants`, and `mcp_servers`.
//! 2. The headless session never appears in the shared GUI/TUI session list.
//! 3. Multiple headless runs can execute concurrently without SQLite write
//!    contention against the GUI, TUI, or one another.

use crate::agent::config_resolve::build_agent_config_from_assistant;
use crate::agent::AgentConfig;
use crate::execution_mode::ExecutionMode;
use crate::headless::cli::HeadlessArgs;
use crate::repositories::assistant_repository::AssistantRepository;
use crate::repositories::settings_repository::SettingsRepository;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

/// Tables cleared in the ephemeral SQLite copy so no prior sessions, messages,
/// or scheduled tasks exist in the ephemeral runtime.
const EPHEMERAL_RUNTIME_TABLES: &[&str] = &[
    "pending_queue",
    "compact_contexts",
    "message_index_meta",
    "messages",
    "planning_todos",
    "planning_goals",
    "planning_scratchpad",
    "scheduled_tasks",
    "sessions",
];

/// Holds the ephemeral directory and database path for a single headless run.
#[derive(Debug)]
pub struct EphemeralEnvironment {
    pub temp_dir: TempDir,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
}

impl EphemeralEnvironment {
    /// Create a fresh ephemeral environment, snapshotting `source_db_path` if it exists.
    pub fn create(source_db_path: Option<&Path>) -> Result<Self, String> {
        let temp_dir = tempfile::Builder::new()
            .prefix("liblet-headless-")
            .tempdir()
            .map_err(|e| format!("Failed to create ephemeral temp directory: {e}"))?;

        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| format!("Failed to create ephemeral data directory: {e}"))?;

        let db_path = data_dir.join("libragent_ephemeral.db");

        if let Some(src) = source_db_path {
            if src.exists() && src.is_file() {
                snapshot_sqlite_db_stripping_sessions(src, &db_path)?;
            }
        }

        Ok(Self {
            temp_dir,
            data_dir,
            db_path,
        })
    }
}

/// Copy `source_db` into `dest_db` using SQLite's transactionally consistent
/// `VACUUM INTO` (read-only on `source_db`) and strip persisted session/message/task
/// rows in `dest_db`.
pub fn snapshot_sqlite_db_stripping_sessions(
    source_db: &Path,
    dest_db: &Path,
) -> Result<(), String> {
    if dest_db.exists() {
        let _ = std::fs::remove_file(dest_db);
    }

    let src_conn = rusqlite::Connection::open_with_flags(
        source_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open source profile DB {:?}: {e}", source_db))?;
    let _ = src_conn.busy_timeout(Duration::from_secs(5));

    let dest_str = dest_db.to_string_lossy().to_string();
    if let Err(_vacuum_err) = src_conn.execute("VACUUM INTO ?1", [&dest_str]) {
        std::fs::copy(source_db, dest_db)
            .map_err(|e| format!("Failed to snapshot source DB {:?}: {e}", source_db))?;
    }
    drop(src_conn);

    let dst_conn = rusqlite::Connection::open(dest_db)
        .map_err(|e| format!("Failed to open ephemeral DB {:?}: {e}", dest_db))?;
    let _ = dst_conn.busy_timeout(Duration::from_secs(5));

    let _ = dst_conn.execute_batch("PRAGMA foreign_keys = OFF;");
    for table in EPHEMERAL_RUNTIME_TABLES {
        // Ignore "no such table" if the source DB is from an older migration state.
        let _ = dst_conn.execute(&format!("DELETE FROM {table};"), []);
    }
    let _ = dst_conn.execute_batch("PRAGMA foreign_keys = ON;");

    Ok(())
}

/// Resolved initial configuration for a headless run after merging CLI flags
/// with the profile's persisted settings and assistants.
#[derive(Debug, Clone)]
pub struct ResolvedHeadlessConfig {
    pub workspace_path: PathBuf,
    pub model: String,
    pub provider: String,
    pub execution_mode: ExecutionMode,
    pub assistant_id: Option<String>,
    pub agent_config: AgentConfig,
}

/// Resolve workspace path (`-C` / `--cd` or `cwd`), validating that the directory exists.
pub fn resolve_workspace_path(cd_override: Option<&Path>) -> Result<PathBuf, String> {
    let raw = match cd_override {
        Some(path) => {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|e| format!("Failed to resolve current working directory: {e}"))?
                    .join(path)
            }
        }
        None => std::env::current_dir()
            .map_err(|e| format!("Failed to resolve current working directory: {e}"))?,
    };

    if !raw.exists() {
        return Err(format!(
            "Workspace directory does not exist: {}",
            raw.display()
        ));
    }
    if !raw.is_dir() {
        return Err(format!(
            "Workspace path is not a directory: {}",
            raw.display()
        ));
    }

    Ok(std::fs::canonicalize(&raw).unwrap_or(raw))
}

/// Resolve model, provider, execution mode, and assistant config from repositories + CLI overrides.
pub async fn resolve_headless_config(
    args: &HeadlessArgs,
    settings_repo: &dyn SettingsRepository,
    assistant_repo: &dyn AssistantRepository,
) -> Result<ResolvedHeadlessConfig, String> {
    let workspace_path = resolve_workspace_path(args.cd.as_deref())?;

    let (default_model, default_provider) = match settings_repo.get("preferredModel").await {
        Ok(Some(setting)) => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&setting.value) {
                let m = val
                    .get("model")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("gpt-4o")
                    .to_string();
                let p = val
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("openai")
                    .to_string();
                (m, p)
            } else {
                ("gpt-4o".to_string(), "openai".to_string())
            }
        }
        _ => ("gpt-4o".to_string(), "openai".to_string()),
    };

    let model = args.model.clone().unwrap_or(default_model);
    let provider = args.provider.clone().unwrap_or(default_provider);

    let execution_mode = match args.execution_mode {
        Some(mode) => mode,
        None => resolve_default_execution_mode(settings_repo).await,
    };

    let (assistant_id, agent_config) =
        resolve_assistant_config(args.assistant.as_deref(), settings_repo, assistant_repo).await?;

    Ok(ResolvedHeadlessConfig {
        workspace_path,
        model,
        provider,
        execution_mode,
        assistant_id,
        agent_config,
    })
}

async fn resolve_default_execution_mode(settings_repo: &dyn SettingsRepository) -> ExecutionMode {
    for key in ["defaultExecutionMode", "executionMode"] {
        if let Ok(Some(setting)) = settings_repo.get(key).await {
            let trimmed = setting.value.trim().trim_matches('"');
            if let Ok(mode) = trimmed.parse::<ExecutionMode>() {
                return mode;
            }
        }
    }
    ExecutionMode::Normal
}

async fn resolve_assistant_config(
    requested: Option<&str>,
    settings_repo: &dyn SettingsRepository,
    assistant_repo: &dyn AssistantRepository,
) -> Result<(Option<String>, AgentConfig), String> {
    if let Some(target) = requested {
        let target = target.trim();
        if let Some(found) = assistant_repo
            .get_assistant(target)
            .await
            .map_err(|e| format!("Failed to query assistant '{target}': {e}"))?
        {
            let config = build_agent_config_from_assistant(&found)?;
            return Ok((Some(found.id), config));
        }

        let all = assistant_repo
            .list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {e}"))?;
        if let Some(found) = all
            .into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(target))
        {
            let config = build_agent_config_from_assistant(&found)?;
            return Ok((Some(found.id), config));
        }

        return Err(format!("Assistant not found: '{target}'"));
    }

    for key in ["defaultAssistantId", "selectedAssistantId"] {
        if let Ok(Some(setting)) = settings_repo.get(key).await {
            let candidate_id = setting.value.trim().trim_matches('"');
            if !candidate_id.is_empty() {
                if let Ok(Some(found)) = assistant_repo.get_assistant(candidate_id).await {
                    if let Ok(config) = build_agent_config_from_assistant(&found) {
                        return Ok((Some(found.id), config));
                    }
                }
            }
        }
    }

    let all = assistant_repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list assistants: {e}"))?;
    let preferred = all
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("Libr Assistant"))
        .or_else(|| {
            all.iter()
                .find(|a| a.name.eq_ignore_ascii_case("Coding Expert"))
        })
        .or_else(|| all.first());

    if let Some(assistant) = preferred {
        let config = build_agent_config_from_assistant(assistant)?;
        return Ok((Some(assistant.id.clone()), config));
    }

    Ok((None, AgentConfig::default()))
}
