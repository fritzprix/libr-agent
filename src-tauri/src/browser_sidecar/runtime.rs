use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::browser::BrowserContextId;
use chromiumoxide::detection::{default_executable, DetectionOptions};
use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};
use futures::StreamExt;
use log::{debug, warn};
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

const SESSION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const BROWSER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound Chromium cold-start so a stuck `Browser::launch` cannot outlive the
/// parent client's createSession bootstrap timeout without a sidecar error.
const BROWSER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(45);

fn emit_sidecar_diagnostic(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

#[derive(Clone)]
pub(crate) struct SharedBrowserRuntime {
    pub(crate) browser: Arc<Mutex<Browser>>,
    pub(crate) handler_abort: tokio::task::AbortHandle,
    pub(crate) headed: bool,
    pub(crate) user_data_dir: PathBuf,
    /// When true, `user_data_dir` is disposable and may be deleted on shutdown.
    pub(crate) ephemeral: bool,
    pub(crate) console_logs: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, Vec<super::contracts::ConsoleEntry>>>,
    >,
}

#[derive(Clone)]
pub(crate) struct BrowserRuntimeManager {
    state: Arc<Mutex<RuntimeState>>,
}

enum RuntimeState {
    Uninitialized,
    Starting {
        visible: bool,
        use_imported_profile: bool,
        notify: Arc<Notify>,
    },
    Ready(SharedBrowserRuntime),
}

pub(crate) struct SidecarSession {
    /// Isolated CDP context for clean ephemeral sessions. `None` when using the
    /// imported Default profile so cookies/logins from User Data are shared.
    pub(crate) context_id: Option<BrowserContextId>,
    pub(crate) page: Arc<chromiumoxide::Page>,
}

impl BrowserRuntimeManager {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RuntimeState::Uninitialized)),
        }
    }

    pub(crate) async fn ensure_runtime(
        &self,
        visible: bool,
        imported_user_data_dir: Option<PathBuf>,
    ) -> Result<SharedBrowserRuntime, String> {
        let use_imported_profile = imported_user_data_dir.is_some();
        loop {
            let maybe_notify = {
                let mut state = self.state.lock().await;
                enum ReadyDecision {
                    Use(SharedBrowserRuntime),
                    Reject(String),
                }

                let ready_decision = match &*state {
                    RuntimeState::Ready(runtime) => {
                        if runtime.headed != visible {
                            Some(ReadyDecision::Reject(format!(
                "Browser runtime is already running in {} mode, but this session requested {} mode",
                if runtime.headed { "visible" } else { "headless" },
                if visible { "visible" } else { "headless" }
              )))
                        } else if runtime.ephemeral == use_imported_profile {
                            Some(ReadyDecision::Reject(
                                "Browser runtime is already running with a different profile mode. Close active browser sessions before switching between clean and imported profiles.".to_string(),
                            ))
                        } else {
                            Some(ReadyDecision::Use(runtime.clone()))
                        }
                    }
                    _ => None,
                };

                if let Some(decision) = ready_decision {
                    match decision {
                        ReadyDecision::Use(runtime) => return Ok(runtime),
                        ReadyDecision::Reject(error) => return Err(error),
                    }
                }

                match &*state {
                    RuntimeState::Ready(_) => {
                        return Err("Browser runtime ready-state race".to_string());
                    }
                    RuntimeState::Starting {
                        visible: current_visible,
                        use_imported_profile: current_imported,
                        notify,
                    } => {
                        if *current_visible != visible {
                            return Err(format!(
                "Browser runtime is already starting in {} mode, but this session requested {} mode",
                if *current_visible { "visible" } else { "headless" },
                if visible { "visible" } else { "headless" }
              ));
                        }
                        if *current_imported != use_imported_profile {
                            return Err(
                                "Browser runtime is already starting with a different profile mode"
                                    .to_string(),
                            );
                        }
                        Some(notify.clone())
                    }
                    RuntimeState::Uninitialized => {
                        let notify = Arc::new(Notify::new());
                        *state = RuntimeState::Starting {
                            visible,
                            use_imported_profile,
                            notify: notify.clone(),
                        };
                        None
                    }
                }
            };

            if let Some(notify) = maybe_notify {
                notify.notified().await;
                continue;
            }

            let launch_result = launch_runtime(visible, imported_user_data_dir.clone()).await;
            let mut state = self.state.lock().await;
            let notify = match std::mem::replace(&mut *state, RuntimeState::Uninitialized) {
                RuntimeState::Starting { notify, .. } => notify,
                RuntimeState::Ready(runtime) => {
                    *state = RuntimeState::Ready(runtime.clone());
                    return Ok(runtime);
                }
                RuntimeState::Uninitialized => {
                    return Err("Browser runtime initialization state was lost".to_string());
                }
            };

            match launch_result {
                Ok(runtime) => {
                    *state = RuntimeState::Ready(runtime.clone());
                    notify.notify_waiters();
                    return Ok(runtime);
                }
                Err(error) => {
                    notify.notify_waiters();
                    return Err(error);
                }
            }
        }
    }

    pub(crate) async fn current_runtime(&self) -> Option<SharedBrowserRuntime> {
        let state = self.state.lock().await;
        match &*state {
            RuntimeState::Ready(runtime) => Some(runtime.clone()),
            RuntimeState::Uninitialized | RuntimeState::Starting { .. } => None,
        }
    }

    pub(crate) async fn take_runtime(&self) -> Option<SharedBrowserRuntime> {
        let mut state = self.state.lock().await;
        match std::mem::replace(&mut *state, RuntimeState::Uninitialized) {
            RuntimeState::Ready(runtime) => Some(runtime),
            RuntimeState::Starting { notify, .. } => {
                notify.notify_waiters();
                None
            }
            RuntimeState::Uninitialized => None,
        }
    }
}

async fn launch_runtime(
    visible: bool,
    imported_user_data_dir: Option<PathBuf>,
) -> Result<SharedBrowserRuntime, String> {
    let executable = resolve_browser_executable().await?;
    let ephemeral = imported_user_data_dir.is_none();
    let user_data_dir = if let Some(path) = imported_user_data_dir {
        let path = crate::browser_profiles::ensure_under_profiles_storage(&path)?;
        if !path.is_dir() {
            return Err(
                "Imported browser profile directory is missing. Re-import from Settings."
                    .to_string(),
            );
        }
        path
    } else {
        create_browser_runtime_profile_dir().await?
    };
    emit_sidecar_diagnostic(format!(
        "Launching Chromium automation runtime in {} mode with executable: {} (profile: {}, ephemeral={})",
        if visible { "visible" } else { "headless" },
        executable.display(),
        user_data_dir.display(),
        ephemeral
    ));
    let mut builder = BrowserConfig::builder()
        .chrome_executable(executable)
        .user_data_dir(&user_data_dir);
    if visible {
        builder = builder.with_head();
    }
    let config = match builder.build() {
        Ok(config) => config,
        Err(error) => {
            if ephemeral {
                cleanup_browser_runtime_profile_dir(&user_data_dir).await;
            }
            return Err(format!("Failed to build browser config: {error}"));
        }
    };
    let (browser, mut handler) =
        match tokio::time::timeout(BROWSER_LAUNCH_TIMEOUT, Browser::launch(config)).await {
            Ok(Ok(browser)) => browser,
            Ok(Err(error)) => {
                emit_sidecar_diagnostic(format!(
                    "Chromium automation launch failed in {} mode: {}",
                    if visible { "visible" } else { "headless" },
                    error
                ));
                if ephemeral {
                    cleanup_browser_runtime_profile_dir(&user_data_dir).await;
                }
                return Err(format!(
                    "Failed to launch Chromium automation session: {error}"
                ));
            }
            Err(_) => {
                emit_sidecar_diagnostic(format!(
                    "Chromium automation launch timed out after {:?} in {} mode",
                    BROWSER_LAUNCH_TIMEOUT,
                    if visible { "visible" } else { "headless" }
                ));
                if ephemeral {
                    cleanup_browser_runtime_profile_dir(&user_data_dir).await;
                }
                return Err(format!(
                    "Chromium automation launch timed out after {}s",
                    BROWSER_LAUNCH_TIMEOUT.as_secs()
                ));
            }
        };
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if let Err(error) = event {
                warn!("Browser sidecar handler error: {error}");
            }
        }
        debug!("Browser sidecar handler loop exited");
    });

    Ok(SharedBrowserRuntime {
        browser: Arc::new(Mutex::new(browser)),
        handler_abort: handler_task.abort_handle(),
        headed: visible,
        user_data_dir,
        ephemeral,
        console_logs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    })
}

async fn resolve_browser_executable() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("LIBRAGENT_BROWSER_EXECUTABLE") {
        let path = PathBuf::from(path);
        if path.exists() {
            emit_sidecar_diagnostic(format!(
                "Using browser executable from LIBRAGENT_BROWSER_EXECUTABLE: {}",
                path.display()
            ));
            return Ok(path);
        }
        emit_sidecar_diagnostic(format!(
            "LIBRAGENT_BROWSER_EXECUTABLE points to a missing path: {}",
            path.display()
        ));
        return Err(format!(
            "LIBRAGENT_BROWSER_EXECUTABLE points to a missing browser executable: {}",
            path.display()
        ));
    }

    match default_executable(DetectionOptions::default()) {
        Ok(path) => {
            emit_sidecar_diagnostic(format!(
                "Resolved system browser executable: {}",
                path.display()
            ));
            return Ok(path);
        }
        Err(error) => {
            emit_sidecar_diagnostic(format!(
        "System browser executable auto-detection failed; falling back to bundled Chromium download: {}",
        error
      ));
        }
    }

    let base_dir = browser_runtime_cache_root();
    emit_sidecar_diagnostic(format!(
        "Preparing bundled Chromium runtime cache directory: {}",
        base_dir.display()
    ));
    tokio::fs::create_dir_all(&base_dir)
        .await
        .map_err(|e| format!("Failed to create browser runtime cache directory: {e}"))?;

    let fetcher = BrowserFetcher::new(
        BrowserFetcherOptions::builder()
            .with_path(&base_dir)
            .build()
            .map_err(|e| format!("Failed to configure Chromium fetcher: {e}"))?,
    );
    emit_sidecar_diagnostic(format!(
        "Downloading or locating bundled Chromium runtime in: {}",
        base_dir.display()
    ));
    let info = fetcher
        .fetch()
        .await
        .map_err(|e| format!("Failed to download bundled Chromium runtime: {e}"))?;
    emit_sidecar_diagnostic(format!(
        "Using bundled Chromium runtime executable: {}",
        info.executable_path.display()
    ));
    Ok(info.executable_path)
}

pub fn browser_runtime_cache_root() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.fritzprix.libragent")
        .join("browser-runtime")
}

pub fn browser_runtime_profile_root() -> PathBuf {
    browser_runtime_cache_root().join("profiles")
}

pub fn browser_runtime_profile_dir(runtime_id: Uuid) -> PathBuf {
    browser_runtime_profile_root().join(runtime_id.to_string())
}

async fn create_browser_runtime_profile_dir() -> Result<PathBuf, String> {
    let user_data_dir = browser_runtime_profile_dir(Uuid::new_v4());
    tokio::fs::create_dir_all(&user_data_dir)
        .await
        .map_err(|error| {
            format!(
                "Failed to create browser runtime profile directory '{}': {error}",
                user_data_dir.display()
            )
        })?;
    emit_sidecar_diagnostic(format!(
        "Using isolated browser runtime profile directory: {}",
        user_data_dir.display()
    ));
    Ok(user_data_dir)
}

pub(crate) async fn cleanup_browser_runtime_profile_dir(user_data_dir: &Path) {
    match tokio::fs::remove_dir_all(user_data_dir).await {
        Ok(()) => debug!(
            "Cleaned up browser runtime profile directory: {}",
            user_data_dir.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => warn!(
            "Failed to clean up browser runtime profile directory {}: {}",
            user_data_dir.display(),
            error
        ),
    }
}

pub(crate) async fn cleanup_failed_context_launch(
    browser: Arc<Mutex<Browser>>,
    context_id: Option<BrowserContextId>,
) {
    let Some(context_id) = context_id else {
        return;
    };
    if let Err(error) = browser
        .lock()
        .await
        .dispose_browser_context(context_id)
        .await
    {
        warn!("Failed to dispose partially initialized browser context: {error}");
    }
}

pub(crate) async fn cleanup_session_resources(
    browser: Arc<Mutex<Browser>>,
    session: SidecarSession,
    session_id: &str,
) -> Result<(), String> {
    let mut cleanup_errors = Vec::new();

    let page_close = tokio::time::timeout(
        SESSION_CLEANUP_TIMEOUT,
        session.page.as_ref().clone().close(),
    )
    .await;
    match page_close {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            cleanup_errors.push(format!(
                "Failed to close browser page {}: {}",
                session_id, error
            ));
        }
        Err(_) => {
            cleanup_errors.push(format!(
                "Timed out after {:?} while closing browser page {}",
                SESSION_CLEANUP_TIMEOUT, session_id
            ));
        }
    }

    if let Some(context_id) = session.context_id {
        let context_close = tokio::time::timeout(SESSION_CLEANUP_TIMEOUT, async {
            browser
                .lock()
                .await
                .dispose_browser_context(context_id)
                .await
        })
        .await;
        match context_close {
            Ok(Ok(())) => {}
            Ok(Err(error)) => cleanup_errors.push(format!(
                "Failed to dispose browser context for session {}: {}",
                session_id, error
            )),
            Err(_) => cleanup_errors.push(format!(
                "Timed out after {:?} while disposing browser context for session {}",
                SESSION_CLEANUP_TIMEOUT, session_id
            )),
        }
    }

    if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(cleanup_errors.join("; "))
    }
}

pub(crate) async fn shutdown_runtime(runtime: SharedBrowserRuntime) {
    let close_result = {
        let mut browser = runtime.browser.lock().await;
        tokio::time::timeout(BROWSER_SHUTDOWN_TIMEOUT, browser.close()).await
    };
    match close_result {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => warn!("Failed to close shared browser runtime: {error}"),
        Err(_) => warn!(
            "Timed out while requesting shared browser runtime shutdown after {:?}",
            BROWSER_SHUTDOWN_TIMEOUT
        ),
    }

    let wait_result = {
        let mut browser = runtime.browser.lock().await;
        tokio::time::timeout(BROWSER_SHUTDOWN_TIMEOUT, browser.wait()).await
    };
    match wait_result {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => warn!("Failed waiting for shared browser runtime exit: {error}"),
        Err(_) => {
            warn!(
                "Shared browser runtime did not exit after {:?}; forcing kill",
                BROWSER_SHUTDOWN_TIMEOUT
            );
            let kill_result = {
                let mut browser = runtime.browser.lock().await;
                browser.kill().await
            };
            match kill_result {
                Some(Ok(())) => debug!("Forced shared browser runtime kill completed"),
                Some(Err(error)) => {
                    warn!("Failed to kill shared browser runtime: {error}");
                }
                None => warn!("Shared browser runtime kill unavailable for this browser"),
            }
        }
    }

    runtime.handler_abort.abort();
    if runtime.ephemeral {
        cleanup_browser_runtime_profile_dir(&runtime.user_data_dir).await;
    } else {
        debug!(
            "Preserving imported browser profile directory: {}",
            runtime.user_data_dir.display()
        );
    }
}
