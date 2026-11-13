//! Headless execution and CLI entry point for `liblet` (#2703).
//!
//! Architecture:
//! - `cli`: Flag parsing, TTY/stdin mode selection, and exit code constants (`0`, `1`, `2`, `3`, `4`, `124`, `130`, `143`).
//! - `output`: `text`, `json`, and `stream-json` formatters on `stdout`.
//! - `ephemeral_db`: Per-process `TempDir` and read-only SQLite snapshot of the user's profile DB with session tables cleared.
//! - `runner`: Ephemeral session execution monitor (completion, approval stop `3`, question/secret stop `4`, timeout `124`, interrupt `130`/`143`, error `1`).

pub mod cli;
pub mod ephemeral_db;
pub mod output;
pub mod runner;

use crate::agent::context::registry::ContextRegistry;
use crate::agent::session_manager::execution_mode::set_execution_mode_in;
use crate::agent::state::AgentSession;
use crate::execution_mode::ExecutionMode;
use crate::headless::cli::{
    exit_codes, parse_cli, HeadlessArgs, LibletCommand, ListSessionsArgs, TtyState,
};
use crate::headless::ephemeral_db::{
    resolve_headless_config, resolve_workspace_path, EphemeralEnvironment, ResolvedHeadlessConfig,
};
use crate::headless::output::{write_final_output, HeadlessResultPayload};
use crate::headless::runner::{
    monitor_headless_session, spawn_interrupt_watcher, HeadlessMonitorParams, InterruptSignal,
};
use crate::models::chat::{Message, MessageSource};
use crate::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteAssistantRepository,
    SqliteSessionRepository, SqliteSettingsRepository,
};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;
use tempfile::TempDir;
use tokio::sync::{watch, RwLock};

/// Determine whether `liblet` should read piped `stdin` before parsing CLI flags.
pub fn should_read_piped_stdin(args: &[String], tty: TtyState) -> bool {
    if tty.stdin_is_tty {
        return false;
    }
    for arg in args {
        if arg == "--" {
            break;
        }
        if matches!(
            arg.as_str(),
            "-h" | "--help" | "-V" | "--version" | "--list-sessions" | "--hook"
        ) || arg.starts_with("--hook=")
        {
            return false;
        }
    }
    true
}

/// Connect and migrate the ephemeral SQLite database inside `EphemeralEnvironment`.
pub async fn init_ephemeral_database(db_path: &Path) -> Result<DatabaseConnection, String> {
    let db_url = crate::utils::sqlite::format_sqlite_url(db_path.to_string_lossy().as_ref());
    crate::lifecycle::database::init_database(&db_url)
        .await
        .map_err(|e| format!("Failed to initialize ephemeral database: {e}"))
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
}

impl EphemeralHeadlessRuntime {
    /// Bootstrap an isolated ephemeral runtime from `source_db_path` and `HeadlessArgs`.
    pub async fn bootstrap(
        source_db_path: Option<&Path>,
        args: &HeadlessArgs,
    ) -> Result<Self, String> {
        let env = EphemeralEnvironment::create(source_db_path)?;
        let db = init_ephemeral_database(&env.db_path).await?;

        let settings_repo = SqliteSettingsRepository::new(db.clone());
        let assistant_repo = SqliteAssistantRepository::new(db.clone());
        let session_repo: Arc<dyn SessionRepository> =
            Arc::new(SqliteSessionRepository::new(db.clone()));

        let resolved = resolve_headless_config(args, &settings_repo, &assistant_repo).await?;
        let session_id = format!("headless-{}", uuid::Uuid::new_v4());
        let now = chrono::Utc::now().timestamp_millis();
        let workspace_str = resolved.workspace_path.to_string_lossy().to_string();

        let metadata = SessionMetadata {
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
        };

        session_repo
            .upsert_session(&metadata)
            .await
            .map_err(|e| format!("Failed to create ephemeral session record: {e}"))?;

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
        })
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

        let payload = monitor_headless_session(monitor_params, interrupt_rx, stdout).await;

        driver_handle.abort();
        // Reap the aborted driver task before teardown: an unreaped task can
        // still hold a pooled SQLite connection, and Windows (unlike Linux)
        // refuses to delete a file while a handle to it is open.
        let _ = driver_handle.await;

        cleanup_headless_run(
            &self.session_id,
            &self.session_repo,
            &self.active_sessions,
            &self.db,
            &self.env.temp_dir,
        )
        .await;

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
fn remove_temp_dir_with_retry(path: &Path) {
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

/// Format a startup failure before a session could be created and return exit code `1`.
pub fn emit_startup_error<Out: Write, ErrOut: Write>(
    args: &HeadlessArgs,
    error_message: String,
    stdout: &mut Out,
    stderr: &mut ErrOut,
) -> HeadlessResultPayload {
    let workspace_path = args
        .cd
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| ".".to_string());
    let execution_mode = args.execution_mode.unwrap_or(ExecutionMode::Normal);
    let payload = HeadlessResultPayload::error(error_message, workspace_path, execution_mode);
    let _ = write_final_output(args.output_format, &payload, stdout, stderr);
    payload
}

/// Execute `--list-sessions` against the profile database, read-only.
///
/// Prints one tab-separated line per session of the target directory:
/// `id  status  model  updated_at  title`. A profile with no database yet has no
/// sessions, which is a success. A database that cannot be read (it cannot be
/// opened, has no `sessions` table, or has a bad row) is an error on stderr and
/// exit `1`, never an empty list.
pub fn run_list_sessions<Out: Write, ErrOut: Write>(
    list_args: &ListSessionsArgs,
    source_db_path: &Path,
    stdout: &mut Out,
    stderr: &mut ErrOut,
) -> i32 {
    let target_dir = match resolve_workspace_path(list_args.cd.as_deref()) {
        Ok(dir) => dir,
        Err(err) => {
            let _ = writeln!(stderr, "error: {err}");
            return exit_codes::ERROR;
        }
    };

    if !source_db_path.exists() {
        return exit_codes::SUCCESS;
    }

    match query_sessions_of(source_db_path, &target_dir) {
        Ok(rows) => {
            for (id, name, status, model, updated_at) in rows {
                let title = if name.trim().is_empty() {
                    "(untitled)"
                } else {
                    name.as_str()
                };
                let _ = writeln!(stdout, "{id}\t{status}\t{model}\t{updated_at}\t{title}");
            }
            exit_codes::SUCCESS
        }
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to list sessions: {err}");
            exit_codes::ERROR
        }
    }
}

type SessionRow = (String, String, String, String, i64);

/// Select the sessions whose `workspace_override` designates `workspace`.
///
/// The app stores `workspace_override` exactly as the user gave it (only `~`
/// expanded), while `workspace` arrives canonicalized from
/// [`resolve_workspace_path`]. Raw string equality therefore misses real
/// sessions on Windows (`\\?\C:\...`) and macOS (`/private/var/...`) or behind
/// symlinks, so both sides are canonicalized in Rust and compared. When
/// canonicalizing a stored value fails (the directory no longer exists), the
/// row falls back to plain string equality against the canonical target.
fn query_sessions_of(db_path: &Path, workspace: &Path) -> Result<Vec<SessionRow>, rusqlite::Error> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let canonical_target =
        std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let canonical_target_str = canonical_target.to_string_lossy();
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(name, ''), status, model, updated_at, workspace_override \
         FROM sessions \
         WHERE workspace_override IS NOT NULL \
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    rows.filter_map(|row| match row {
        Ok((id, name, status, model, updated_at, stored_override)) => {
            let matches_workspace = match std::fs::canonicalize(&stored_override) {
                Ok(canonical_override) => canonical_override == canonical_target,
                Err(_) => stored_override == canonical_target_str,
            };
            matches_workspace.then_some(Ok((id, name, status, model, updated_at)))
        }
        Err(err) => Some(Err(err)),
    })
    .collect()
}

/// Run a full headless invocation using the hidden Tauri WebView runtime.
///
/// Shares the bootstrap (`EphemeralHeadlessRuntime::bootstrap`), the run
/// (`run_with_driver` + `monitor_headless_session`), and the cleanup
/// (`cleanup_headless_run`) with the test path; the hidden-WebView turn is
/// just the driver injected into that shared run.
pub fn run_headless_with_hidden_webview(args: HeadlessArgs) -> i32 {
    let profile = crate::profile::resolve_profile();
    let source_db_path = crate::profile::db_path(profile);

    // Pre-validate workspace path before booting Tauri so invalid `-C` fails fast.
    if let Err(err) = resolve_workspace_path(args.cd.as_deref()) {
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
    }

    let mut runtime = match tauri::async_runtime::block_on(EphemeralHeadlessRuntime::bootstrap(
        Some(&source_db_path),
        &args,
    )) {
        Ok(runtime) => runtime,
        Err(err) => {
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
        }
    };

    // Point LibrAgent and WebView data/cache folders into the ephemeral TempDir
    // so this headless process is 100% isolated from the GUI, TUI, and sibling
    // headless processes.
    std::env::set_var("LIBRAGENT_DATA_DIR", &runtime.env.data_dir);
    std::env::set_var("LIBRAGENT_DB_PATH", &runtime.env.db_path);
    let webview_cache_dir = runtime.env.temp_dir.path().join("webview-cache");
    let _ = std::fs::create_dir_all(&webview_cache_dir);
    #[cfg(target_os = "windows")]
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_cache_dir);
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("XDG_CACHE_HOME", &webview_cache_dir);
        if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    let temp_dir_path = runtime.env.temp_dir.path().to_path_buf();
    let db_url =
        crate::utils::sqlite::format_sqlite_url(runtime.env.db_path.to_string_lossy().as_ref());
    crate::state::set_sqlite_db_url(db_url);

    // Wire the process-wide repositories to the ephemeral connection, seed the
    // default assistants into the snapshot (the GUI does the same on startup),
    // and re-resolve the run config so a profile without assistants yet still
    // picks one up before the session is created.
    if let Err(err) = tauri::async_runtime::block_on(async {
        let _ = crate::session::get_session_manager()
            .map_err(|e| format!("Failed to initialize SessionManager: {e}"))?;
        crate::lifecycle::repositories::init_repositories(&runtime.db).await;
        crate::lifecycle::llm_env_seed::seed_llm_settings_from_env().await;
        let _ = crate::services::assistant_init::ensure_default_assistants_hardcoded().await;
        runtime.resolved =
            resolve_headless_config(&args, &runtime.settings_repo, &runtime.assistant_repo).await?;
        Ok::<(), String>(())
    }) {
        tauri::async_runtime::block_on(cleanup_headless_run(
            &runtime.session_id,
            &runtime.session_repo,
            &runtime.active_sessions,
            &runtime.db,
            &runtime.env.temp_dir,
        ));
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
    }

    let exit_code_holder = Arc::new(std::sync::Mutex::new(exit_codes::ERROR));
    let exit_code_for_setup = exit_code_holder.clone();

    let mut context = crate::create_tauri_context();
    for win in &mut context.config_mut().app.windows {
        win.visible = false;
        win.skip_taskbar = true;
        win.focus = false;
    }

    let builder = crate::configure_tauri_builder(tauri::Builder::default());
    let app = match builder
        .setup(move |app| setup_hidden_headless_app(app, args, runtime, exit_code_for_setup))
        .build(context)
    {
        Ok(app) => app,
        Err(err) => {
            eprintln!("error: failed to initialize headless runtime: {err}");
            return exit_codes::ERROR;
        }
    };

    // `run_return` (unlike `run`) returns after the event loop tears down, so
    // the exit code below is actually reached. The session, database, and
    // temp dir were already torn down by the shared cleanup inside
    // `run_with_driver`; this second pass catches files the WebView's own
    // processes still held open during the first pass (mostly Windows).
    app.run_return(|_app_handle, _event| {});

    remove_temp_dir_with_retry(&temp_dir_path);

    let code = *exit_code_holder.lock().unwrap();
    std::process::exit(code);
}

/// Build the agent manager for the hidden-WebView path on the runtime's own
/// session repository and session map, so the manager, the monitor, and the
/// driver share one source of truth for the ephemeral session.
pub fn build_agent_manager(
    app_handle: tauri::AppHandle,
    runtime: &EphemeralHeadlessRuntime,
    proxy_manager: Arc<crate::mcp::MCPServiceProxyManager>,
    message: Arc<dyn crate::repositories::message_repository::MessageRepository>,
    pending_queue: Arc<dyn crate::repositories::PendingQueueRepository>,
    compact_context: Arc<dyn crate::repositories::CompactContextRepository>,
) -> crate::agent::AgentSessionManager {
    let repositories = crate::agent::AgentRepositories::new(runtime.session_repo.clone())
        .with_message(message)
        .with_pending_queue(pending_queue)
        .with_compact_context(compact_context);
    crate::agent::AgentSessionManager::new_with_repositories(
        app_handle,
        proxy_manager,
        repositories,
        runtime.active_sessions.clone(),
    )
}

fn setup_hidden_headless_app(
    app: &mut tauri::App,
    args: HeadlessArgs,
    runtime: EphemeralHeadlessRuntime,
    exit_code_holder: Arc<std::sync::Mutex<i32>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ephemeral_data_dir = runtime.env.data_dir.clone();
    let log_dir = ephemeral_data_dir.join("logs");
    let _ = crate::logger::setup_file_logger(log_dir, 1);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        let _ = window.set_skip_taskbar(true);
    }

    let file_manager = crate::services::SecureFileManager::new_with_base_dir(
        ephemeral_data_dir.join("global_shared"),
    );
    app.manage(file_manager);

    let dropped_file_service =
        crate::services::DroppedFileService::new_with_trusted_hidden_root(ephemeral_data_dir);
    app.manage(dropped_file_service);

    let browser_server = crate::services::InteractiveBrowserServer::new(Duration::from_secs(30));
    app.manage(browser_server);

    let proxy_manager_arc = crate::state::get_mcp_service_proxy_manager();
    let message_repo_arc: Arc<dyn crate::repositories::message_repository::MessageRepository> =
        Arc::new((*crate::state::get_message_repository()).clone());
    let pending_queue_repo_arc: Arc<dyn crate::repositories::PendingQueueRepository> =
        Arc::new((*crate::state::get_pending_queue_repository()).clone());
    let compact_context_repo_arc: Arc<dyn crate::repositories::CompactContextRepository> =
        Arc::new((*crate::state::get_compact_context_repository()).clone());

    let app_handle = app.handle().clone();
    let agent_manager = build_agent_manager(
        app_handle.clone(),
        &runtime,
        proxy_manager_arc,
        message_repo_arc,
        pending_queue_repo_arc,
        compact_context_repo_arc,
    );

    crate::state::init_active_sessions(agent_manager.active_sessions_arc());
    crate::state::init_channel_dispatch_agent(agent_manager.clone());
    crate::state::init_session_bus(crate::agent::session_bus::SessionBus::new());
    crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
        4, 8, 4, 8,
    ));
    crate::state::init_app_handle(app_handle.clone());
    app.manage(agent_manager.clone());

    tauri::async_runtime::spawn(async move {
        let interrupt_rx = spawn_interrupt_watcher();
        let driver_args = args.clone();
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        let payload = runtime
            .run_with_driver(
                &args,
                interrupt_rx,
                move |session_id, _session_repo, _active_sessions, resolved| {
                    let agent_manager = agent_manager.clone();
                    async move {
                        run_hidden_webview_turn(
                            &agent_manager,
                            &session_id,
                            &resolved,
                            &driver_args,
                        )
                        .await
                    }
                },
                &mut stdout,
                &mut stderr,
            )
            .await;
        if let Ok(mut guard) = exit_code_holder.lock() {
            *guard = payload.exit_code;
        }
        app_handle.exit(payload.exit_code);
    });

    Ok(())
}

/// Hidden-WebView turn driver injected into the shared `run_with_driver` run.
///
/// Creates the session in the agent manager, waits for the hidden WebView
/// frontend (`useLLMListener`) to mark itself ready, and starts the workflow.
/// Returning `Err` marks the session `Error`; the shared monitor in
/// `run_with_driver` then emits the final payload and tears everything down.
async fn run_hidden_webview_turn(
    agent_manager: &crate::agent::AgentSessionManager,
    session_id: &str,
    resolved: &ResolvedHeadlessConfig,
    args: &HeadlessArgs,
) -> Result<(), String> {
    if let Ok(sm) = crate::session::get_session_manager() {
        let _ = sm
            .register_session_override(session_id, resolved.workspace_path.clone())
            .await;
    }

    agent_manager
        .create_session_with_repo(
            agent_manager.repositories().session.clone(),
            session_id.to_string(),
            None,
            Some(resolved.model.clone()),
            Some(resolved.provider.clone()),
            resolved.agent_config.clone(),
            args.isolation,
            None,
        )
        .await?;

    let _ = crate::agent::session_manager::execution_mode::set_execution_mode(
        agent_manager,
        session_id,
        resolved.execution_mode,
    )
    .await;

    // Wait for the hidden WebView frontend to mark itself ready. `--timeout`
    // bounds the wait; otherwise a fixed 30 s cap prevents an infinite hang
    // when the WebView never initializes.
    let frontend_wait_timeout = args
        .timeout_secs
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(30));

    tokio::select! {
        _ = crate::lifecycle::frontend_ready::wait_until_ready() => {}
        _ = tokio::time::sleep(frontend_wait_timeout) => {
            return Err(
                "Timed out waiting for hidden LLM runtime to initialize".to_string(),
            );
        }
    }

    let user_message = Message::new_user_message(
        session_id.to_string(),
        args.prompt.clone(),
        Some(MessageSource::Ui),
        resolved.assistant_id.clone(),
    );

    agent_manager
        .start_workflow(session_id.to_string(), user_message)
        .await
}

/// Entry point for the `liblet` binary.
pub fn run_liblet_main() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tty = TtyState {
        stdin_is_tty: std::io::stdin().is_terminal(),
        stdout_is_tty: std::io::stdout().is_terminal(),
    };

    let piped_stdin = if should_read_piped_stdin(&args, tty) {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        Some(buf)
    } else {
        None
    };

    match parse_cli(&args, tty, piped_stdin.as_deref()) {
        Ok(LibletCommand::Help(text)) => {
            print!("{text}");
            let _ = std::io::stdout().flush();
            exit_codes::SUCCESS
        }
        Ok(LibletCommand::Version(text)) => {
            println!("{text}");
            let _ = std::io::stdout().flush();
            exit_codes::SUCCESS
        }
        Ok(LibletCommand::ListSessions(list_args)) => {
            let profile = crate::profile::resolve_profile();
            let source_db_path = crate::profile::db_path(profile);
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            run_list_sessions(&list_args, &source_db_path, &mut stdout, &mut stderr)
        }
        Ok(LibletCommand::Headless(headless_args)) => {
            run_headless_with_hidden_webview(headless_args)
        }
        Ok(LibletCommand::Tui(_tui_args)) => {
            eprintln!(
                "error: interactive TUI mode (#2702) is not available yet; pass -p / --prompt <text> for headless execution."
            );
            exit_codes::ERROR
        }
        Err(usage_err) => {
            eprintln!("{usage_err}");
            usage_err.exit_code()
        }
    }
}
