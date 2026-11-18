//! Ephemeral runtime, database, and cleanup lifecycle (#2887).
//!
//! The per-process `TempDir`, the read-only SQLite snapshot, the fork base, and
//! the run write-back.

use crate::agent::context::registry::ContextRegistry;
use crate::agent::session_manager::execution_mode::set_execution_mode_in;
use crate::agent::state::AgentSession;
use crate::headless::cli::HeadlessArgs;
use crate::headless::ephemeral_db::{
    resolve_headless_config, resolve_workspace_path, EphemeralEnvironment, ResolvedHeadlessConfig,
};
use crate::headless::output::{write_final_output, HeadlessResultPayload};
use crate::headless::runner::{monitor_headless_session, HeadlessMonitorParams, InterruptSignal};
use crate::headless::writeback;
use crate::models::chat::{Message, MessageSource};
use crate::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteAssistantRepository,
    SqliteSessionRepository, SqliteSettingsRepository,
};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{watch, RwLock};

/// Connect and migrate the ephemeral SQLite database inside `EphemeralEnvironment`.
pub async fn init_ephemeral_database(db_path: &Path) -> Result<DatabaseConnection, String> {
    let db_url = crate::utils::sqlite::format_sqlite_url(db_path.to_string_lossy().as_ref());
    crate::lifecycle::database::init_database(&db_url)
        .await
        .map_err(|e| format!("Failed to initialize ephemeral database: {e}"))
}

/// Point-in-time marker of the persisted history a run started from (#2859).
///
/// S8 (#2860) uses it as the merge-or-fork base when a `--session` run is
/// written back to the profile DB. A fresh-session run captures an empty base.
/// #2878: the base is the session's `version` counter, which every message
/// insert, update (including upsert-on-conflict), and delete bumps — so an
/// in-place edit of an existing message cannot hide behind an unchanged
/// message count and newest id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionForkBase {
    pub session_id: String,
    pub version: i64,
}

/// Read the session's persisted message-change counter from the snapshot, so
/// the fork base is identical whether the session was continued or just
/// created.
async fn capture_fork_base(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<SessionForkBase, String> {
    use crate::entity::session;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

    let version: Option<i64> = session::Entity::find()
        .filter(session::Column::Id.eq(session_id))
        .select_only()
        .column(session::Column::Version)
        .into_tuple()
        .one(db)
        .await
        .map_err(|e| format!("Failed to read version of session {session_id}: {e}"))?;
    // A resumed run's session row always exists in the snapshot; a missing
    // row means the snapshot is broken and must not look like version 0.
    let version = version
        .ok_or_else(|| format!("Cannot capture fork base: session {session_id} is missing"))?;
    Ok(SessionForkBase {
        session_id: session_id.to_string(),
        version,
    })
}

/// Write a finished headless run's history back into the profile database
/// (#2860): merge into the original `--session` when it is unchanged and not
/// open elsewhere, fork it otherwise, and insert a fresh run's rows as one
/// new session. See [`writeback`] for the full rules.
pub fn write_back_run(
    runtime: &EphemeralHeadlessRuntime,
) -> Result<writeback::WriteBackOutcome, String> {
    writeback::write_back_run(runtime)
}

/// Self-contained ephemeral runtime context for a single headless run.
///
/// Holds the isolated `TempDir` (`EphemeralEnvironment`), SeaORM connection to
/// the ephemeral SQLite copy, `SqliteSessionRepository`, and active session map.
/// Dropping this struct removes the `TempDir` and all temporary files.
pub struct EphemeralHeadlessRuntime {
    pub env: EphemeralEnvironment,
    pub db: DatabaseConnection,
    pub session_repo: Arc<dyn SessionRepository>,
    pub settings_repo: SqliteSettingsRepository,
    pub assistant_repo: SqliteAssistantRepository,
    pub active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    pub resolved: ResolvedHeadlessConfig,
    pub session_id: String,
    /// Persisted history the run started from, for the S8 write-back seam.
    pub fork_base: SessionForkBase,
    /// True when `--session <id>` continued an existing persisted session.
    pub resumed: bool,
}

impl EphemeralHeadlessRuntime {
    /// Bootstrap an isolated ephemeral runtime from `source_db_path` and `HeadlessArgs`.
    ///
    /// With `args.session`, the run continues that persisted session (#2859):
    /// the snapshot kept its rows, an unknown id fails clearly, and the
    /// session's own model/provider/assistant/workspace win over profile
    /// defaults (CLI flags still override). Otherwise a fresh
    /// `headless-<uuid>` session is created as before.
    pub async fn bootstrap(
        source_db_path: Option<&Path>,
        args: &HeadlessArgs,
    ) -> Result<Self, String> {
        let env = EphemeralEnvironment::create(source_db_path, args.session.as_deref())?;
        let db = init_ephemeral_database(&env.db_path).await?;

        let settings_repo = SqliteSettingsRepository::new(db.clone());
        let assistant_repo = SqliteAssistantRepository::new(db.clone());
        let session_repo: Arc<dyn SessionRepository> =
            Arc::new(SqliteSessionRepository::new(db.clone()));

        let mut resolved = resolve_headless_config(args, &settings_repo, &assistant_repo).await?;
        let existing_session = match args.session.as_deref() {
            Some(id) => Some(
                session_repo
                    .get_session(id)
                    .await
                    .map_err(|e| format!("Failed to load session {id}: {e}"))?
                    .ok_or_else(|| format!("Session not found: {id}"))?,
            ),
            None => None,
        };
        if let Some(existing) = existing_session.as_ref() {
            Self::merge_resolved_with_session(args, &mut resolved, existing)?;
        }

        let resumed = existing_session.is_some();
        let session_id = match existing_session.as_ref() {
            Some(existing) => existing.id.clone(),
            None => format!("headless-{}", uuid::Uuid::new_v4()),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let workspace_str = resolved.workspace_path.to_string_lossy().to_string();

        let metadata = match existing_session.as_ref() {
            Some(existing) => SessionMetadata {
                status: SessionStatus::Idle,
                status_reason: None,
                model: resolved.model.clone(),
                provider: resolved.provider.clone(),
                assistant_id: resolved.assistant_id.clone(),
                execution_mode: resolved.execution_mode,
                workspace_override: Some(workspace_str),
                workspace_isolation: args.isolation,
                updated_at: now,
                ..(*existing).clone()
            },
            None => SessionMetadata {
                id: session_id.clone(),
                name: None,
                status: SessionStatus::Idle,
                model: resolved.model.clone(),
                provider: resolved.provider.clone(),
                assistant_id: resolved.assistant_id.clone(),
                parent_session_id: None,
                lineage_id: Some(session_id.clone()),
                depth: Some(0),
                max_depth: None,
                max_fanout: None,
                org_id: None,
                org_name: None,
                org_root_session_id: None,
                created_at: now,
                updated_at: now,
                last_viewed_at: None,
                last_message_at: None,
                last_attention_at: None,
                last_attention_reason: None,
                is_bookmarked: false,
                execution_mode: resolved.execution_mode,
                workspace_override: Some(workspace_str),
                workspace_isolation: args.isolation,
                docker_config: None,
                docker_container_name: None,
                docker_host_workspace_path: None,
                status_reason: None,
                forked_from: None,
            },
        };

        session_repo
            .upsert_session(&metadata)
            .await
            .map_err(|e| format!("Failed to create ephemeral session record: {e}"))?;

        let fork_base = capture_fork_base(&db, &session_id).await?;

        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        let session = AgentSession::new(metadata, Arc::new(ContextRegistry::new()), None);
        active_sessions
            .write()
            .await
            .insert(session_id.clone(), session);

        let _ = set_execution_mode_in(
            &active_sessions,
            &session_repo,
            None,
            &session_id,
            resolved.execution_mode,
        )
        .await;

        Ok(Self {
            env,
            db,
            session_repo,
            settings_repo,
            assistant_repo,
            active_sessions,
            resolved,
            session_id,
            fork_base,
            resumed,
        })
    }

    /// A continued session keeps its own model, provider, assistant, and
    /// workspace; CLI flags still win, and profile defaults never replace the
    /// session's own configuration (#2859).
    pub(super) fn merge_resolved_with_session(
        args: &HeadlessArgs,
        resolved: &mut ResolvedHeadlessConfig,
        existing: &SessionMetadata,
    ) -> Result<(), String> {
        if args.model.is_none() {
            resolved.model = existing.model.clone();
        }
        if args.provider.is_none() {
            resolved.provider = existing.provider.clone();
        }
        if args.assistant.is_none() && existing.assistant_id.is_some() {
            resolved.assistant_id = existing.assistant_id.clone();
        }
        if let Some(stored) = existing.workspace_override.as_deref() {
            resolved.workspace_path = resolve_workspace_path(Some(Path::new(stored)))
                .map_err(|e| format!("Workspace of session {} is unavailable: {e}", existing.id))?;
        }
        Ok(())
    }

    /// Run a headless turn using a custom async turn driver and monitor it to completion.
    pub async fn run_with_driver<F, Fut, Out, ErrOut>(
        self,
        args: &HeadlessArgs,
        interrupt_rx: watch::Receiver<Option<InterruptSignal>>,
        driver: F,
        stdout: &mut Out,
        stderr: &mut ErrOut,
    ) -> HeadlessResultPayload
    where
        F: FnOnce(
                String,
                Arc<dyn SessionRepository>,
                Arc<RwLock<HashMap<String, AgentSession>>>,
                ResolvedHeadlessConfig,
            ) -> Fut
            + Send
            + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
        Out: Write,
        ErrOut: Write,
    {
        let workspace_str = self.resolved.workspace_path.to_string_lossy().to_string();
        let execution_mode = self.resolved.execution_mode;

        let user_msg = Message::new_user_message(
            self.session_id.clone(),
            args.prompt.clone(),
            Some(MessageSource::Ui),
            self.resolved.assistant_id.clone(),
        );
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(&self.session_id) {
                session.messages.write().await.push(user_msg);
            }
        }

        let driver_session_id = self.session_id.clone();
        let driver_repo = self.session_repo.clone();
        let driver_active = self.active_sessions.clone();
        let driver_resolved = self.resolved.clone();

        let driver_handle = tokio::spawn(async move {
            if let Err(err) = driver(
                driver_session_id.clone(),
                driver_repo.clone(),
                driver_active.clone(),
                driver_resolved,
            )
            .await
            {
                let _ = driver_repo
                    .update_status(&driver_session_id, SessionStatus::Error, Some(&err))
                    .await;
                let mut active = driver_active.write().await;
                if let Some(session) = active.get_mut(&driver_session_id) {
                    session.metadata.status = SessionStatus::Error;
                    session.metadata.status_reason = Some(err);
                }
            }
        });

        let monitor_params = HeadlessMonitorParams {
            session_repo: &self.session_repo,
            active_sessions: &self.active_sessions,
            session_id: &self.session_id,
            workspace_path: workspace_str,
            execution_mode,
            model: self.resolved.model.clone(),
            provider: self.resolved.provider.clone(),
            assistant_id: self.resolved.assistant_id.clone(),
            output_format: args.output_format,
            timeout: args.timeout_secs.map(Duration::from_secs),
            poll_interval: Duration::from_millis(15),
        };

        let mut payload = monitor_headless_session(monitor_params, interrupt_rx, stdout).await;

        driver_handle.abort();
        // Reap the aborted driver task before teardown: an unreaped task can
        // still hold a pooled SQLite connection, and Windows (unlike Linux)
        // refuses to delete a file while a handle to it is open.
        let _ = driver_handle.await;

        // S8 (#2860): the run's rows are still present here, so the
        // write-back must happen before cleanup removes them. It covers
        // every terminal status: the monitor returned, so the run is over.
        let write_back = match write_back_run(&self) {
            Ok(outcome) => Some(outcome),
            Err(err) => {
                eprintln!("warning: headless write-back failed: {err}");
                None
            }
        };

        cleanup_headless_run(
            &self.session_id,
            &self.session_repo,
            &self.active_sessions,
            &self.db,
            &self.env.temp_dir,
        )
        .await;

        // Report the profile-database session this run ended as, before the
        // final result line is emitted (#2860).
        if let Some(outcome) = write_back {
            payload.session_id = outcome.session_id;
            payload.forked_from = outcome.forked_from;
        }

        let _ = write_final_output(args.output_format, &payload, stdout, stderr);
        payload
    }
}

/// Remove `path` recursively with a bounded retry, reporting the final
/// failure on `stderr` instead of swallowing it.
///
/// Used by every headless teardown path because SQLite and WebView file
/// handles are released asynchronously after close: a single best-effort
/// delete races with handle release and leaves the directory behind on
/// Windows (Linux silently unlinks open files and never notices).
pub(super) fn remove_temp_dir_with_retry(path: &Path) {
    for attempt in 0..40u32 {
        match std::fs::remove_dir_all(path) {
            Ok(()) => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) if attempt < 39 => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                eprintln!("warning: failed to remove ephemeral headless directory {path:?}: {err}");
            }
        }
    }
}

/// Shared teardown for every headless run path (production binary and tests):
/// drop the active session entry, delete the ephemeral session record, close
/// the database connection, and remove the ephemeral `TempDir`.
pub async fn cleanup_headless_run(
    session_id: &str,
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    db: &DatabaseConnection,
    temp_dir: &TempDir,
) {
    // Dropping the in-memory entry cannot fail. The other two can, and a failed
    // delete leaves an orphan session row, so say so on stderr.
    active_sessions.write().await.remove(session_id);
    if let Err(err) = session_repo.delete_session(session_id).await {
        eprintln!("warning: failed to delete ephemeral session {session_id}: {err}");
    }
    if let Err(err) = db.clone().close().await {
        eprintln!("warning: failed to close ephemeral database: {err}");
    }
    // Remove the temp dir explicitly instead of relying on the implicit
    // `TempDir` drop so the retry above runs while the caller can still
    // report the outcome; the later `TempDir::drop` sees `NotFound` and
    // treats it as success.
    remove_temp_dir_with_retry(temp_dir.path());
}
