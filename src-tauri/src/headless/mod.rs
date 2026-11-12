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
        let _ = self.active_sessions.write().await.remove(&self.session_id);
        let _ = self.session_repo.delete_session(&self.session_id).await;
        let _ = self.db.close().await;

        let _ = write_final_output(args.output_format, &payload, stdout, stderr);
        payload
    }
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

/// Execute `--list-sessions` against the profile database read-only.
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

    let conn = match rusqlite::Connection::open_with_flags(
        source_db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(stderr, "error: failed to open session database: {e}");
            return exit_codes::ERROR;
        }
    };

    let target_str = target_dir.to_string_lossy().to_string();
    let mut stmt = match conn.prepare(
        "SELECT id, COALESCE(name, ''), status, model, updated_at \
         FROM sessions \
         WHERE workspace_override = ?1 \
         ORDER BY updated_at DESC",
    ) {
        Ok(s) => s,
        Err(_) => return exit_codes::SUCCESS,
    };

    let rows = stmt.query_map([&target_str], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    });

    if let Ok(iter) = rows {
        for item in iter.flatten() {
            let (id, name, status, model, updated_at) = item;
            let title = if name.trim().is_empty() {
                "(untitled)"
            } else {
                name.as_str()
            };
            let _ = writeln!(stdout, "{id}\t{status}\t{model}\t{updated_at}\t{title}");
        }
    }

    exit_codes::SUCCESS
}

/// Run a full headless invocation using the hidden Tauri WebView runtime in Phase 1.
pub fn run_headless_with_hidden_webview(args: HeadlessArgs) -> i32 {
    let profile = crate::profile::resolve_profile();
    let source_db_path = crate::profile::db_path(profile);

    // Pre-validate workspace path before booting Tauri so invalid `-C` fails fast.
    if let Err(err) = resolve_workspace_path(args.cd.as_deref()) {
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
    }

    let ephemeral_env = match EphemeralEnvironment::create(Some(&source_db_path)) {
        Ok(env) => env,
        Err(err) => {
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
        }
    };

    // Point LibrAgent and WebView data/cache folders into the ephemeral TempDir
    // so this headless process is 100% isolated from the GUI, TUI, and sibling
    // headless processes.
    std::env::set_var("LIBRAGENT_DATA_DIR", &ephemeral_env.data_dir);
    std::env::set_var("LIBRAGENT_DB_PATH", &ephemeral_env.db_path);
    let webview_cache_dir = ephemeral_env.temp_dir.path().join("webview-cache");
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

    let db_url =
        crate::utils::sqlite::format_sqlite_url(ephemeral_env.db_path.to_string_lossy().as_ref());
    crate::state::set_sqlite_db_url(db_url.clone());

    if let Err(err) = tauri::async_runtime::block_on(async {
        let _ = crate::session::get_session_manager()
            .map_err(|e| format!("Failed to initialize SessionManager: {e}"))?;
        let db = crate::lifecycle::database::init_database(&db_url)
            .await
            .map_err(|e| format!("Failed to initialize ephemeral database: {e}"))?;
        crate::lifecycle::repositories::init_repositories(&db).await;
        crate::lifecycle::llm_env_seed::seed_llm_settings_from_env().await;
        Ok::<(), String>(())
    }) {
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
    }

    let exit_code_holder = Arc::new(std::sync::Mutex::new(exit_codes::ERROR));
    let exit_code_for_setup = exit_code_holder.clone();
    let ephemeral_data_dir = ephemeral_env.data_dir.clone();

    let mut context = tauri::generate_context!();
    for win in &mut context.config_mut().app.windows {
        win.visible = false;
        win.skip_taskbar = true;
        win.focus = false;
    }

    let builder = crate::configure_tauri_builder(tauri::Builder::default());
    let app = match builder
        .setup(move |app| {
            setup_hidden_headless_app(app, args, ephemeral_data_dir, exit_code_for_setup)
        })
        .build(context)
    {
        Ok(app) => app,
        Err(err) => {
            eprintln!("error: failed to initialize headless runtime: {err}");
            return exit_codes::ERROR;
        }
    };

    app.run(|_app_handle, _event| {});

    drop(ephemeral_env);
    let code = *exit_code_holder.lock().unwrap();
    code
}

fn setup_hidden_headless_app(
    app: &mut tauri::App,
    args: HeadlessArgs,
    ephemeral_data_dir: std::path::PathBuf,
    exit_code_holder: Arc<std::sync::Mutex<i32>>,
) -> Result<(), Box<dyn std::error::Error>> {
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
    let session_repo_arc: Arc<dyn crate::repositories::SessionRepository> =
        Arc::new((*crate::state::get_session_repository()).clone());
    let message_repo_arc: Arc<dyn crate::repositories::message_repository::MessageRepository> =
        Arc::new((*crate::state::get_message_repository()).clone());
    let pending_queue_repo_arc: Arc<dyn crate::repositories::PendingQueueRepository> =
        Arc::new((*crate::state::get_pending_queue_repository()).clone());
    let compact_context_repo_arc: Arc<dyn crate::repositories::CompactContextRepository> =
        Arc::new((*crate::state::get_compact_context_repository()).clone());

    let repositories = crate::agent::AgentRepositories::new(session_repo_arc)
        .with_message(message_repo_arc)
        .with_pending_queue(pending_queue_repo_arc)
        .with_compact_context(compact_context_repo_arc);

    let app_handle = app.handle().clone();
    let agent_manager = crate::agent::AgentSessionManager::new_with_repositories(
        app_handle.clone(),
        proxy_manager_arc,
        repositories,
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
        let code =
            execute_hidden_webview_turn(&app_handle, &agent_manager, args, interrupt_rx).await;
        if let Ok(mut guard) = exit_code_holder.lock() {
            *guard = code;
        }
        app_handle.exit(code);
    });

    Ok(())
}

async fn execute_hidden_webview_turn(
    app_handle: &tauri::AppHandle,
    agent_manager: &crate::agent::AgentSessionManager,
    args: HeadlessArgs,
    mut interrupt_rx: watch::Receiver<Option<InterruptSignal>>,
) -> i32 {
    let _ = crate::services::assistant_init::ensure_default_assistants_hardcoded().await;

    let settings_repo = crate::state::get_settings_repository();
    let assistant_repo = crate::state::get_assistant_repository();
    let resolved = match resolve_headless_config(&args, settings_repo, assistant_repo).await {
        Ok(r) => r,
        Err(err) => {
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
        }
    };

    let session_id = format!("headless-{}", uuid::Uuid::new_v4());
    if let Ok(sm) = crate::session::get_session_manager() {
        let _ = sm
            .register_session_override(&session_id, resolved.workspace_path.clone())
            .await;
    }

    if let Err(err) = agent_manager
        .create_session_with_repo(
            agent_manager.repositories().session.clone(),
            session_id.clone(),
            None,
            Some(resolved.model.clone()),
            Some(resolved.provider.clone()),
            resolved.agent_config.clone(),
            args.isolation,
            None,
        )
        .await
    {
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        return emit_startup_error(&args, err, &mut stdout, &mut stderr).exit_code;
    }

    let _ = crate::agent::session_manager::execution_mode::set_execution_mode(
        agent_manager,
        &session_id,
        resolved.execution_mode,
    )
    .await;

    // Wait for the hidden WebView frontend (`useLLMListener`) to mark itself ready,
    // while still honoring `--timeout` and OS interrupt signals.
    let frontend_wait_timeout = args
        .timeout_secs
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(30));

    let workspace_str = resolved.workspace_path.to_string_lossy().to_string();

    tokio::select! {
        _ = crate::lifecycle::frontend_ready::wait_until_ready() => {}
        _ = tokio::time::sleep(frontend_wait_timeout) => {
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            let payload = if args.timeout_secs.is_some() {
                HeadlessResultPayload::timeout(workspace_str, resolved.execution_mode)
            } else {
                HeadlessResultPayload::error(
                    "Timed out waiting for hidden LLM runtime to initialize".to_string(),
                    workspace_str,
                    resolved.execution_mode,
                )
            };
            let _ = agent_manager.delete_session(session_id).await;
            let _ = write_final_output(args.output_format, &payload, &mut stdout, &mut stderr);
            return payload.exit_code;
        }
        _ = interrupt_rx.changed() => {
            let maybe_sig = *interrupt_rx.borrow();
            if let Some(sig) = maybe_sig {
                let mut stdout = std::io::stdout();
                let mut stderr = std::io::stderr();
                let payload = HeadlessResultPayload::interrupted(
                    sig.exit_code(),
                    workspace_str,
                    resolved.execution_mode,
                );
                let _ = agent_manager.delete_session(session_id).await;
                let _ = write_final_output(args.output_format, &payload, &mut stdout, &mut stderr);
                return payload.exit_code;
            }
        }
    }

    let user_message = Message::new_user_message(
        session_id.clone(),
        args.prompt.clone(),
        Some(MessageSource::Ui),
        resolved.assistant_id.clone(),
    );

    if let Err(err) = agent_manager
        .start_workflow(session_id.clone(), user_message)
        .await
    {
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        let payload = HeadlessResultPayload::error(err, workspace_str, resolved.execution_mode);
        let _ = agent_manager.delete_session(session_id).await;
        let _ = write_final_output(args.output_format, &payload, &mut stdout, &mut stderr);
        return payload.exit_code;
    }

    let active_sessions = agent_manager.active_sessions_arc();
    let session_repo = agent_manager.repositories().session.clone();

    let monitor_params = HeadlessMonitorParams {
        session_repo: &session_repo,
        active_sessions: &active_sessions,
        session_id: &session_id,
        workspace_path: workspace_str,
        execution_mode: resolved.execution_mode,
        model: resolved.model,
        provider: resolved.provider,
        assistant_id: resolved.assistant_id,
        output_format: args.output_format,
        timeout: args.timeout_secs.map(Duration::from_secs),
        poll_interval: Duration::from_millis(25),
    };

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let payload = monitor_headless_session(monitor_params, interrupt_rx, &mut stdout).await;

    let _ = agent_manager.terminate_session(session_id.clone()).await;
    let _ = agent_manager.delete_session(session_id).await;
    let _ = app_handle;

    let _ = write_final_output(args.output_format, &payload, &mut stdout, &mut stderr);
    payload.exit_code
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
