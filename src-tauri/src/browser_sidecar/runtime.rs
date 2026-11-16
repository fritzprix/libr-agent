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

use super::profile_lock::chrome_profile_appears_in_use;
use crate::session::get_session_manager;

/// App-local sticky Chromium user-data dir (agent logins only — not everyday Chrome).
const AGENT_STICKY_PROFILE_DIR: &str = "browser_agent_profile";

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
    Starting { visible: bool, notify: Arc<Notify> },
    Ready(SharedBrowserRuntime),
}

pub(crate) struct SidecarSession {
    /// Isolated CDP context when used. Sticky agent profile uses `None`
    /// (default cookie jar) so logins persist across createSession.
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
    ) -> Result<SharedBrowserRuntime, String> {
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
                        notify,
                    } => {
                        if *current_visible != visible {
                            return Err(format!(
                "Browser runtime is already starting in {} mode, but this session requested {} mode",
                if *current_visible { "visible" } else { "headless" },
                if visible { "visible" } else { "headless" }
              ));
                        }
                        Some(notify.clone())
                    }
                    RuntimeState::Uninitialized => {
                        let notify = Arc::new(Notify::new());
                        *state = RuntimeState::Starting {
                            visible,
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

            let launch_result = launch_agent_sticky_runtime(visible).await;
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

/// App-data base dir that holds the agent sticky Chromium profiles.
fn agent_sticky_base_dir() -> PathBuf {
    if let Ok(manager) = get_session_manager() {
        return manager.get_base_data_dir().clone();
    }
    crate::profile::data_dir(crate::profile::resolve_profile())
}

/// Name of the agent sticky Chromium profile directory.
///
/// The first backend on a profile keeps the un-suffixed name, so agent logins
/// survive a restart (#1984). A backend that finds another live one takes
/// `browser_agent_profile.<pid>`, so two Chromium instances never share a
/// SingletonLock; that suffix is a collision guard, not a permanent identity
/// (#2858 G1.2).
pub fn agent_sticky_profile_dir_name(has_other_backend: bool, pid: u32) -> String {
    if has_other_backend {
        format!("{AGENT_STICKY_PROFILE_DIR}.{pid}")
    } else {
        AGENT_STICKY_PROFILE_DIR.to_string()
    }
}

/// Remove `browser_agent_profile.<pid>` dirs whose process is gone. A suffixed
/// dir is only ever used by a second backend, so it is safe to drop once that
/// process is dead (#2858 G1.2).
fn sweep_dead_agent_sticky_profiles(
    base_dir: &Path,
    own_pid: u32,
    pid_alive: impl Fn(u32) -> bool,
) -> usize {
    let prefix = format!("{AGENT_STICKY_PROFILE_DIR}.");
    let Ok(entries) = std::fs::read_dir(base_dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own_pid || pid_alive(pid) {
            continue;
        }
        if std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Per-profile app-data Chromium profile for agent sticky logins (not everyday
/// Chrome User Data).
pub fn agent_sticky_user_data_dir() -> Result<PathBuf, String> {
    let dir_name = agent_sticky_profile_dir_name(
        crate::server::backend_descriptor::profile_has_other_backend(),
        std::process::id(),
    );
    Ok(agent_sticky_base_dir().join(dir_name))
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
    let removed =
        sweep_dead_agent_sticky_profiles(&agent_sticky_base_dir(), std::process::id(), |pid| {
            crate::utils::process::process_is_alive(pid).ok() == Some(true)
        });
    if removed > 0 {
        emit_sidecar_diagnostic(format!(
            "Removed {removed} stale agent browser profile dir(s) left by dead processes"
        ));
    }
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
    // Best-effort CDP Browser.close.
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

    // Sticky agent profile keeps its user-data-dir.
    // Wipe only via Settings → Clear agent browser data.
    debug!(
        "Preserving sticky agent browser profile directory: {}",
        runtime.user_data_dir.display()
    );
}

#[cfg(test)]
mod sticky_profile_tests {
    use super::*;

    /// #2858 G1.2: a dir left by a dead second backend is dropped at the next
    /// start, while the first backend's un-suffixed profile and live ones stay.
    #[test]
    fn sweep_removes_only_dead_pid_sticky_profiles() {
        let base = std::env::temp_dir().join(format!("sticky-sweep-{}", std::process::id()));
        let own = base.join("browser_agent_profile.1000");
        let dead = base.join("browser_agent_profile.2000");
        let live = base.join("browser_agent_profile.3000");
        let first_backend = base.join("browser_agent_profile");
        for dir in [&own, &dead, &live, &first_backend] {
            std::fs::create_dir_all(dir).expect("create sticky dir");
        }
        std::fs::write(dead.join("SingletonLock"), b"x").expect("write lock file");

        let removed = sweep_dead_agent_sticky_profiles(&base, 1000, |pid| pid == 3000);

        assert_eq!(removed, 1);
        assert!(own.exists(), "own dir must survive");
        assert!(!dead.exists(), "dead backend dir must be removed");
        assert!(live.exists(), "live backend dir must survive");
        assert!(
            first_backend.exists(),
            "the un-suffixed first-backend profile must never be swept"
        );

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn profile_dir_name_is_un_suffixed_for_the_first_backend() {
        assert_eq!(
            agent_sticky_profile_dir_name(false, 7),
            "browser_agent_profile"
        );
    }
}
