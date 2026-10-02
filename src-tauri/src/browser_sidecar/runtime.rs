use std::path::PathBuf;
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

use crate::browser_profiles::{
    chrome_profile_appears_in_use, pick_loopback_debug_port, spawn_system_chrome_for_profile_async,
    wait_for_cdp_ready, ChromeProfileSpawnOptions,
};
use crate::session::get_session_manager;

/// App-local sticky Chromium user-data dir (agent logins only — not everyday Chrome).
const AGENT_STICKY_PROFILE_DIR: &str = "browser_agent_profile";

/// Which cookie jar / user-data-dir the shared runtime is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserProfileMode {
    /// Fixed LibrAgent agent profile; cookies survive createSession / runtime recycle.
    AgentSticky,
    /// Imported saved-login copy from Settings (`use_profile: true`).
    Imported,
}

impl BrowserProfileMode {
    pub(crate) fn from_imported(use_imported_profile: bool) -> Self {
        if use_imported_profile {
            Self::Imported
        } else {
            Self::AgentSticky
        }
    }
}

const SESSION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const BROWSER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound Chromium cold-start so a stuck `Browser::launch` cannot outlive the
/// parent client's createSession bootstrap timeout without a sidecar error.
const BROWSER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(45);
const CDP_READY_TIMEOUT: Duration = Duration::from_secs(30);

fn emit_sidecar_diagnostic(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

#[derive(Clone)]
pub(crate) struct SharedBrowserRuntime {
    pub(crate) browser: Arc<Mutex<Browser>>,
    pub(crate) handler_abort: tokio::task::AbortHandle,
    pub(crate) headed: bool,
    pub(crate) user_data_dir: PathBuf,
    pub(crate) mode: BrowserProfileMode,
    /// Process we spawned for imported-profile attach (`Browser::connect` has no child).
    pub(crate) owned_child: Option<Arc<Mutex<tokio::process::Child>>>,
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
    /// Isolated CDP context when used. Sticky agent + imported profiles use
    /// `None` (default cookie jar) so logins persist across createSession.
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

                let requested_mode = BrowserProfileMode::from_imported(use_imported_profile);
                let ready_decision = match &*state {
                    RuntimeState::Ready(runtime) => {
                        if runtime.headed != visible {
                            Some(ReadyDecision::Reject(format!(
                "Browser runtime is already running in {} mode, but this session requested {} mode",
                if runtime.headed { "visible" } else { "headless" },
                if visible { "visible" } else { "headless" }
              )))
                        } else if runtime.mode != requested_mode {
                            Some(ReadyDecision::Reject(
                                "Browser runtime is already running with a different profile mode. Close active browser sessions before switching between the agent sticky profile and imported saved logins.".to_string(),
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
    if let Some(path) = imported_user_data_dir {
        return connect_imported_profile_runtime(visible, path).await;
    }
    launch_agent_sticky_runtime(visible).await
}

/// Imported / saved-login mode: spawn system Chrome (no automation DEFAULT_ARGS), then CDP connect.
///
/// Google rejects chromiumoxide `Browser::launch` for account login. Cookies for Google must be
/// created inside this app-local profile (Settings → Open to sign in), not only copied from Chrome.
async fn connect_imported_profile_runtime(
    visible: bool,
    imported_user_data_dir: PathBuf,
) -> Result<SharedBrowserRuntime, String> {
    let user_data_dir =
        crate::browser_profiles::ensure_under_profiles_storage(&imported_user_data_dir)?;
    if !user_data_dir.is_dir() {
        return Err(
            "Imported browser profile directory is missing. Re-import from Settings.".to_string(),
        );
    }

    if chrome_profile_appears_in_use(&user_data_dir) {
        return Err(
            "This saved browser login is already open in another Chrome window. Close the LibrAgent Chrome window from Settings → Open to sign in (or any other Chrome using this saved login), then retry."
                .to_string(),
        );
    }

    let debug_port = pick_loopback_debug_port()?;
    emit_sidecar_diagnostic(format!(
        "Spawning system Chrome for imported profile attach (headed={}, user_data_dir={}, debug_port={})",
        visible,
        user_data_dir.display(),
        debug_port
    ));

    let mut child = spawn_system_chrome_for_profile_async(&ChromeProfileSpawnOptions {
        user_data_dir: user_data_dir.clone(),
        debug_port: Some(debug_port),
        headless: !visible,
        start_url: Some("about:blank".to_string()),
    })
    .await?;

    if let Err(error) =
        wait_for_cdp_ready(debug_port, CDP_READY_TIMEOUT, &mut child, &user_data_dir).await
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error);
    }

    let connect_url = format!("http://127.0.0.1:{debug_port}");
    let (browser, mut handler) =
        match tokio::time::timeout(BROWSER_LAUNCH_TIMEOUT, Browser::connect(connect_url)).await {
            Ok(Ok(pair)) => pair,
            Ok(Err(error)) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(format!(
                    "Failed to attach to system Chrome for imported profile: {error}"
                ));
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(format!(
                    "Timed out attaching to system Chrome after {}s",
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
        mode: BrowserProfileMode::Imported,
        owned_child: Some(Arc::new(Mutex::new(child))),
        console_logs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    })
}

/// Sticky agent profile: fixed app-data user-data-dir, default cookie jar, not deleted on shutdown.
async fn launch_agent_sticky_runtime(visible: bool) -> Result<SharedBrowserRuntime, String> {
    let executable = resolve_browser_executable().await?;
    let user_data_dir = ensure_agent_sticky_user_data_dir().await?;
    if chrome_profile_appears_in_use(&user_data_dir) {
        return Err(
            "The LibrAgent agent browser profile is already open in another Chromium window. Close that window (or clear agent browser data from Settings after closing sessions), then retry."
                .to_string(),
        );
    }
    emit_sidecar_diagnostic(format!(
        "Launching Chromium automation runtime in {} mode with executable: {} (sticky profile: {})",
        if visible { "visible" } else { "headless" },
        executable.display(),
        user_data_dir.display(),
    ));
    // chromiumoxide defaults viewport to 800x600 DeviceMetricsOverride. That locks CSS
    // layout even when the OS window is resized. Headed sessions: disable emulation so
    // content fills the window (same idea as Playwright viewport: null).
    let mut builder = BrowserConfig::builder()
        .chrome_executable(executable)
        .user_data_dir(&user_data_dir);
    if visible {
        builder = builder.with_head().viewport(None);
    }
    let config = builder
        .build()
        .map_err(|error| format!("Failed to build browser config: {error}"))?;
    let (browser, mut handler) =
        match tokio::time::timeout(BROWSER_LAUNCH_TIMEOUT, Browser::launch(config)).await {
            Ok(Ok(browser)) => browser,
            Ok(Err(error)) => {
                emit_sidecar_diagnostic(format!(
                    "Chromium automation launch failed in {} mode: {}",
                    if visible { "visible" } else { "headless" },
                    error
                ));
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
        mode: BrowserProfileMode::AgentSticky,
        owned_child: None,
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

/// Fixed app-data Chromium profile for agent sticky logins (not everyday Chrome User Data).
pub fn agent_sticky_user_data_dir() -> Result<PathBuf, String> {
    if let Ok(manager) = get_session_manager() {
        return Ok(manager.get_base_data_dir().join(AGENT_STICKY_PROFILE_DIR));
    }
    Ok(crate::profile::data_dir(crate::profile::resolve_profile()).join(AGENT_STICKY_PROFILE_DIR))
}

async fn ensure_agent_sticky_user_data_dir() -> Result<PathBuf, String> {
    let user_data_dir = agent_sticky_user_data_dir()?;
    tokio::fs::create_dir_all(&user_data_dir)
        .await
        .map_err(|error| {
            format!(
                "Failed to create agent sticky browser profile directory '{}': {error}",
                user_data_dir.display()
            )
        })?;
    emit_sidecar_diagnostic(format!(
        "Using sticky agent browser profile directory: {}",
        user_data_dir.display()
    ));
    Ok(user_data_dir)
}

/// Delete the sticky agent profile (cookies/logins). Caller must ensure no runtime is using it.
pub fn clear_agent_sticky_profile_dir() -> Result<(), String> {
    let user_data_dir = agent_sticky_user_data_dir()?;
    if chrome_profile_appears_in_use(&user_data_dir) {
        return Err(
            "Agent browser profile is in use. Close all browser sessions first, then try again."
                .to_string(),
        );
    }
    if !user_data_dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&user_data_dir).map_err(|error| {
        format!(
            "Failed to clear agent browser profile '{}': {error}",
            user_data_dir.display()
        )
    })?;
    Ok(())
}

/// Legacy cache UUID profile helpers kept for existing tests only.
/// Production createSession uses [`agent_sticky_user_data_dir`] since #1984.
#[deprecated(note = "UUID cache profiles are no longer used; prefer agent_sticky_user_data_dir")]
pub fn browser_runtime_profile_root() -> PathBuf {
    browser_runtime_cache_root().join("profiles")
}

#[deprecated(note = "UUID cache profiles are no longer used; prefer agent_sticky_user_data_dir")]
pub fn browser_runtime_profile_dir(runtime_id: Uuid) -> PathBuf {
    #[allow(deprecated)]
    browser_runtime_profile_root().join(runtime_id.to_string())
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
    // Best-effort CDP Browser.close for both launch and attach modes.
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

    runtime.handler_abort.abort();

    if let Some(owned_child) = runtime.owned_child {
        // Attach mode: `Browser::connect` has no managed child, so `browser.wait()` /
        // `browser.kill()` cannot reap Chrome and would burn the full shutdown timeout.
        // Kill the process we spawned promptly instead.
        let mut child = owned_child.lock().await;
        match child.kill().await {
            Ok(()) => debug!("Killed owned Chrome process for imported profile runtime"),
            Err(error) => warn!("Failed to kill owned Chrome process: {error}"),
        }
        let _ = tokio::time::timeout(BROWSER_SHUTDOWN_TIMEOUT, child.wait()).await;
    } else {
        // Launch mode: chromiumoxide owns the child via Browser::launch.
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
    }

    // Sticky agent + imported profiles always keep their user-data-dir.
    // Wipe only via Settings → Clear agent browser data (sticky) or remove imported profile.
    debug!(
        "Preserving browser profile directory ({:?}): {}",
        runtime.mode,
        runtime.user_data_dir.display()
    );
}
