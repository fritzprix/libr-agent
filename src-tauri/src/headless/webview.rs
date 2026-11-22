//! Hidden-WebView headless driver (#2887).
//!
//! Boots the Tauri runtime without a visible window and runs one turn through
//! the same runner/monitor pair the test path uses.

use crate::headless::cli::{exit_codes, HeadlessArgs};
use crate::headless::ephemeral_db::{
    resolve_headless_config, resolve_workspace_path, ResolvedHeadlessConfig,
};
use crate::headless::inspect::emit_startup_error;
use crate::headless::runner::spawn_interrupt_watcher;
use crate::headless::runtime::{
    cleanup_headless_run, remove_temp_dir_with_retry_async, EphemeralHeadlessRuntime,
};
use crate::models::chat::{Message, MessageSource};
use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;

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
        // Same NVIDIA DMA-BUF hang applies to this hidden WebView (#2964).
        crate::utils::platform::apply_webkit_dmabuf_workaround();
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
        if let Some(id) = args.session.as_deref() {
            // Re-apply the continued session's own configuration: re-resolution
            // above reads profile defaults, which must not replace the session.
            let existing = runtime
                .session_repo
                .get_session(id)
                .await
                .map_err(|e| format!("Failed to load session {id}: {e}"))?
                .ok_or_else(|| format!("Session not found: {id}"))?;
            EphemeralHeadlessRuntime::merge_resolved_with_session(
                &args,
                &mut runtime.resolved,
                &existing,
            )?;
        }
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

    // Same retry as the async teardown paths (gatekeeper G1.1): the runtime
    // is a global (not owned by the just-exited event loop), so `block_on`
    // here drives the yielding retry for handles the WebView released late.
    tauri::async_runtime::block_on(remove_temp_dir_with_retry_async(&temp_dir_path));

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

/// Listen for the hidden WebView's readiness signal.
///
/// Split from [`setup_hidden_headless_app`] so the wiring can be checked
/// without a full app: the handler runs on the frontend's `frontend-ready`
/// emit and on nothing else.
fn register_hidden_frontend_ready<R: tauri::Runtime, F>(app: &tauri::AppHandle<R>, on_ready: F)
where
    F: Fn() + Send + 'static,
{
    use tauri::Listener;

    app.listen("frontend-ready", move |_| on_ready());
}

fn setup_hidden_headless_app(
    app: &mut tauri::App,
    args: HeadlessArgs,
    runtime: EphemeralHeadlessRuntime,
    exit_code_holder: Arc<std::sync::Mutex<i32>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ephemeral_data_dir = runtime.env.data_dir.clone();
    let log_dir = ephemeral_data_dir.join("logs");
    // #2885: headless stdout carries only the `--output-format` document;
    // logs go to stderr and the ephemeral log file.
    let _ = crate::logger::setup_headless_file_logger(log_dir, 1);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        let _ = window.set_skip_taskbar(true);
    }

    // The GUI marks readiness from `lifecycle::app_setup`, and the TUI reaches
    // that setup too. A hidden headless app never runs it, so without this
    // listener the frontend's `frontend-ready` emit is observed by nobody,
    // `run_hidden_webview_turn` waits out its whole timeout, and the prompt is
    // never sent (#2884).
    register_hidden_frontend_ready(
        app.handle(),
        crate::lifecycle::frontend_ready::mark_as_ready,
    );

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

    if args.session.is_some() {
        // Continue the persisted session (#2859): resume loads its metadata,
        // MCP bindings, and compact context from the snapshot instead of
        // re-creating the row, which would reset name/lineage/created_at.
        agent_manager.resume_session(session_id).await?;
    } else {
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
    }

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

#[cfg(test)]
mod tests {
    use super::register_hidden_frontend_ready;

    /// #2884: the hidden-WebView setup must observe `frontend-ready`.
    ///
    /// The turn driver waits for that mark, and no other setup runs in this
    /// process, so a missing listener makes every headless run time out before
    /// the prompt is sent. The handler is watched through a spy instead of the
    /// global readiness flag, which is one-way process state.
    #[test]
    fn hidden_setup_listens_for_frontend_ready() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tauri::Emitter;

        let app = tauri::test::mock_app();
        let marks = Arc::new(AtomicUsize::new(0));
        let spy = marks.clone();
        register_hidden_frontend_ready(app.handle(), move || {
            spy.fetch_add(1, Ordering::SeqCst);
        });

        // Another event must not read as readiness.
        app.emit("frontend-ready-typo", ()).unwrap();
        assert_eq!(marks.load(Ordering::SeqCst), 0);

        app.emit("frontend-ready", ()).unwrap();
        assert_eq!(marks.load(Ordering::SeqCst), 1);
    }
}
