use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::browser::BrowserContextId;
use chromiumoxide::cdp::browser_protocol::target::{
    CreateBrowserContextParams, CreateTargetParams,
};
use chromiumoxide::detection::{default_executable, DetectionOptions};
use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};
use futures::StreamExt;
use log::{debug, warn};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex, Notify};
use tokio::task::{AbortHandle, JoinSet};
use uuid::Uuid;

pub const BROWSER_SIDECAR_FLAG: &str = "--browser-sidecar";
const SESSION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const BROWSER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const HISTORY_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(4);
const HISTORY_NAVIGATION_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageClassification {
    Normal,
    BlockedInterstitial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryNavigationStatus {
    Navigated,
    NoHistoryEntry,
    BlockedInterstitial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageState {
    pub url: String,
    pub title: Option<String>,
    pub classification: Option<PageClassification>,
    pub navigation_status: Option<HistoryNavigationStatus>,
    pub navigation_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SidecarRequest {
    id: String,
    method: String,
    params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct SidecarResponse {
    id: String,
    result: Option<Value>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSessionParams {
    session_id: String,
    url: String,
    title: Option<String>,
    visible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdParams {
    session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NavigateParams {
    session_id: String,
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluateParams {
    session_id: String,
    script: String,
}

struct SidecarProcess {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
}

struct BrowserAutomationClientState {
    process: Mutex<Option<SidecarProcess>>,
    pending: dashmap::DashMap<String, oneshot::Sender<Result<Value, String>>>,
    request_timeout: Duration,
}

#[derive(Clone)]
pub struct BrowserAutomationClient {
    state: Arc<BrowserAutomationClientState>,
}

impl BrowserAutomationClient {
    pub fn new(request_timeout: Duration) -> Self {
        Self {
            state: Arc::new(BrowserAutomationClientState {
                process: Mutex::new(None),
                pending: dashmap::DashMap::new(),
                request_timeout,
            }),
        }
    }

    pub async fn create_session(
        &self,
        session_id: &str,
        url: &str,
        title: Option<&str>,
        visible: bool,
    ) -> Result<PageState, String> {
        self.request(
            "createSession",
            CreateSessionParams {
                session_id: session_id.to_string(),
                url: url.to_string(),
                title: title.map(ToString::to_string),
                visible,
            },
        )
        .await
    }

    pub async fn close_session(&self, session_id: &str) -> Result<(), String> {
        self.request::<Value>(
            "closeSession",
            SessionIdParams {
                session_id: session_id.to_string(),
            },
        )
        .await
        .map(|_| ())
    }

    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<PageState, String> {
        self.request(
            "navigate",
            NavigateParams {
                session_id: session_id.to_string(),
                url: url.to_string(),
            },
        )
        .await
    }

    pub async fn go_back(&self, session_id: &str) -> Result<PageState, String> {
        self.request(
            "goBack",
            SessionIdParams {
                session_id: session_id.to_string(),
            },
        )
        .await
    }

    pub async fn go_forward(&self, session_id: &str) -> Result<PageState, String> {
        self.request(
            "goForward",
            SessionIdParams {
                session_id: session_id.to_string(),
            },
        )
        .await
    }

    pub async fn evaluate(&self, session_id: &str, script: &str) -> Result<String, String> {
        self.request(
            "evaluate",
            EvaluateParams {
                session_id: session_id.to_string(),
                script: script.to_string(),
            },
        )
        .await
    }

    pub async fn get_state(&self, session_id: &str) -> Result<PageState, String> {
        self.request(
            "getState",
            SessionIdParams {
                session_id: session_id.to_string(),
            },
        )
        .await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<T, String> {
        self.ensure_started().await?;

        let request_id = Uuid::new_v4().to_string();
        let payload = serde_json::to_value(params)
            .map_err(|e| format!("Failed to serialize sidecar request params: {e}"))?;
        let request = SidecarRequest {
            id: request_id.clone(),
            method: method.to_string(),
            params: payload,
        };
        let line = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize sidecar request: {e}"))?;

        let (tx, rx) = oneshot::channel();
        self.state.pending.insert(request_id.clone(), tx);

        let stdin = {
            let process_guard = self.state.process.lock().await;
            let process = process_guard
                .as_ref()
                .ok_or_else(|| "Browser sidecar is not running".to_string())?;
            let _child = process.child.clone();
            process.stdin.clone()
        };

        {
            let mut stdin_guard = stdin.lock().await;
            if let Err(error) = stdin_guard.write_all(line.as_bytes()).await {
                self.state.pending.remove(&request_id);
                return Err(format!(
                    "Failed to write request to browser sidecar: {error}"
                ));
            }
            if let Err(error) = stdin_guard.write_all(b"\n").await {
                self.state.pending.remove(&request_id);
                return Err(format!(
                    "Failed to frame request to browser sidecar: {error}"
                ));
            }
            if let Err(error) = stdin_guard.flush().await {
                self.state.pending.remove(&request_id);
                return Err(format!("Failed to flush browser sidecar request: {error}"));
            }
        }

        let response_value = match tokio::time::timeout(self.state.request_timeout, rx).await {
            Ok(Ok(result)) => result?,
            Ok(Err(_)) => {
                self.state.pending.remove(&request_id);
                return Err("Browser sidecar response channel closed unexpectedly".to_string());
            }
            Err(_) => {
                self.state.pending.remove(&request_id);
                return Err(format!(
                    "Browser sidecar did not respond within {}ms",
                    self.state.request_timeout.as_millis()
                ));
            }
        };

        serde_json::from_value(response_value)
            .map_err(|e| format!("Failed to decode browser sidecar response: {e}"))
    }

    async fn ensure_started(&self) -> Result<(), String> {
        let mut process_guard = self.state.process.lock().await;
        if process_guard.is_some() {
            return Ok(());
        }

        let current_exe = std::env::current_exe().map_err(|e| {
            format!("Failed to resolve current executable for browser sidecar: {e}")
        })?;
        let mut command = Command::new(current_exe);
        command
            .arg(BROWSER_SIDECAR_FLAG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to spawn browser sidecar: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Browser sidecar stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Browser sidecar stdout unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Browser sidecar stderr unavailable".to_string())?;

        let child = Arc::new(Mutex::new(child));
        let stdin = Arc::new(Mutex::new(stdin));

        spawn_sidecar_stdout_task(self.state.clone(), stdout);
        spawn_sidecar_stderr_task(stderr);
        spawn_sidecar_exit_task(self.state.clone(), child.clone());

        *process_guard = Some(SidecarProcess { child, stdin });
        Ok(())
    }
}

fn spawn_sidecar_stdout_task(
    state: Arc<BrowserAutomationClientState>,
    stdout: tokio::process::ChildStdout,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => match serde_json::from_str::<SidecarResponse>(&line) {
                    Ok(response) => {
                        if let Some((_, sender)) = state.pending.remove(&response.id) {
                            let result = match response.error {
                                Some(error) => Err(error),
                                None => Ok(response.result.unwrap_or(Value::Null)),
                            };
                            let _ = sender.send(result);
                        }
                    }
                    Err(error) => {
                        warn!("Failed to parse browser sidecar response: {error}");
                    }
                },
                Ok(None) => {
                    fail_all_pending(&state, "Browser sidecar closed its stdout".to_string());
                    let mut process_guard = state.process.lock().await;
                    *process_guard = None;
                    break;
                }
                Err(error) => {
                    fail_all_pending(
                        &state,
                        format!("Failed reading browser sidecar stdout: {error}"),
                    );
                    let mut process_guard = state.process.lock().await;
                    *process_guard = None;
                    break;
                }
            }
        }
    });
}

fn spawn_sidecar_stderr_task(stderr: tokio::process::ChildStderr) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            warn!("browser-sidecar: {line}");
        }
    });
}

fn spawn_sidecar_exit_task(state: Arc<BrowserAutomationClientState>, child: Arc<Mutex<Child>>) {
    tokio::spawn(async move {
        let wait_result = child.lock().await.wait().await;
        match wait_result {
            Ok(status) => {
                debug!("Browser sidecar exited with status: {status}");
                fail_all_pending(
                    &state,
                    format!("Browser sidecar exited unexpectedly: {status}"),
                );
            }
            Err(error) => {
                fail_all_pending(
                    &state,
                    format!("Failed waiting for browser sidecar exit: {error}"),
                );
            }
        }
        let mut process_guard = state.process.lock().await;
        *process_guard = None;
    });
}

fn fail_all_pending(state: &BrowserAutomationClientState, error: String) {
    let pending_ids: Vec<String> = state
        .pending
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for pending_id in pending_ids {
        if let Some((_, sender)) = state.pending.remove(&pending_id) {
            let _ = sender.send(Err(error.clone()));
        }
    }
}

pub fn run_sidecar_mode() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to start browser sidecar runtime: {e}"))?;
    runtime.block_on(async { BrowserSidecarServer::run().await })
}

struct BrowserSidecarServer {
    runtime: BrowserRuntimeManager,
    sessions: Mutex<HashMap<String, SidecarSession>>,
}

#[derive(Clone)]
struct SharedBrowserRuntime {
    browser: Arc<Mutex<Browser>>,
    handler_abort: AbortHandle,
    headed: bool,
}

#[derive(Clone)]
struct BrowserRuntimeManager {
    state: Arc<Mutex<RuntimeState>>,
}

enum RuntimeState {
    Uninitialized,
    Starting { visible: bool, notify: Arc<Notify> },
    Ready(SharedBrowserRuntime),
}

struct SidecarSession {
    context_id: BrowserContextId,
    page: Arc<chromiumoxide::Page>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NavigationSnapshot {
    url: String,
    title: String,
    ready_state: String,
    history_length: u64,
    history_state: Option<String>,
    body_text_snippet: String,
}

#[derive(Debug, Clone, Copy)]
enum HistoryDirection {
    Back,
    Forward,
}

impl BrowserRuntimeManager {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RuntimeState::Uninitialized)),
        }
    }

    async fn ensure_runtime(&self, visible: bool) -> Result<SharedBrowserRuntime, String> {
        loop {
            let maybe_notify = {
                let mut state = self.state.lock().await;
                match &*state {
                    RuntimeState::Ready(runtime) => {
                        if runtime.headed != visible {
                            return Err(format!(
                                "Browser runtime is already running in {} mode, but this session requested {} mode",
                                if runtime.headed { "visible" } else { "headless" },
                                if visible { "visible" } else { "headless" }
                            ));
                        }
                        return Ok(runtime.clone());
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

            let launch_result = launch_runtime(visible).await;
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

    async fn current_runtime(&self) -> Option<SharedBrowserRuntime> {
        let state = self.state.lock().await;
        match &*state {
            RuntimeState::Ready(runtime) => Some(runtime.clone()),
            RuntimeState::Uninitialized | RuntimeState::Starting { .. } => None,
        }
    }

    async fn take_runtime(&self) -> Option<SharedBrowserRuntime> {
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

impl BrowserSidecarServer {
    fn new() -> Self {
        Self {
            runtime: BrowserRuntimeManager::new(),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    async fn run() -> Result<(), String> {
        let server = Arc::new(Self::new());
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();
        let (response_tx, mut response_rx) = mpsc::unbounded_channel::<SidecarResponse>();
        let writer_task = tokio::spawn(async move {
            let mut writer = tokio::io::BufWriter::new(stdout);
            while let Some(response) = response_rx.recv().await {
                let encoded = serde_json::to_string(&response)
                    .map_err(|e| format!("Failed to encode browser sidecar response: {e}"))?;
                writer
                    .write_all(encoded.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write browser sidecar response: {e}"))?;
                writer
                    .write_all(b"\n")
                    .await
                    .map_err(|e| format!("Failed to frame browser sidecar response: {e}"))?;
                writer
                    .flush()
                    .await
                    .map_err(|e| format!("Failed to flush browser sidecar response: {e}"))?;
            }
            Ok::<(), String>(())
        });
        let mut request_tasks = JoinSet::new();

        while let Some(line) = reader
            .next_line()
            .await
            .map_err(|e| format!("Failed to read browser sidecar request: {e}"))?
        {
            let server = server.clone();
            let response_tx = response_tx.clone();
            request_tasks.spawn(async move {
                let response = match serde_json::from_str::<SidecarRequest>(&line) {
                    Ok(request) => server.handle_request(request).await,
                    Err(error) => SidecarResponse {
                        id: String::from("invalid"),
                        result: None,
                        error: Some(format!("Invalid browser sidecar request: {error}")),
                    },
                };

                if response_tx.send(response).is_err() {
                    warn!("Dropped browser sidecar response because writer task is unavailable");
                }
            });
        }

        drop(response_tx);
        while let Some(result) = request_tasks.join_next().await {
            if let Err(error) = result {
                warn!("Browser sidecar request task failed: {error}");
            }
        }
        writer_task
            .await
            .map_err(|e| format!("Browser sidecar writer task failed: {e}"))??;
        server.shutdown().await;
        Ok(())
    }

    async fn handle_request(&self, request: SidecarRequest) -> SidecarResponse {
        let response = match request.method.as_str() {
            "createSession" => self.create_session(request.params).await,
            "closeSession" => self
                .close_session(request.params)
                .await
                .map(|_| Value::Null),
            "navigate" => self.navigate(request.params).await,
            "goBack" => self.go_back(request.params).await,
            "goForward" => self.go_forward(request.params).await,
            "evaluate" => self.evaluate(request.params).await,
            "getState" => self.get_state(request.params).await,
            _ => Err(format!(
                "Unknown browser sidecar method: {}",
                request.method
            )),
        };

        match response {
            Ok(result) => SidecarResponse {
                id: request.id,
                result: Some(result),
                error: None,
            },
            Err(error) => SidecarResponse {
                id: request.id,
                result: None,
                error: Some(error),
            },
        }
    }

    async fn create_session(&self, params: Value) -> Result<Value, String> {
        let params: CreateSessionParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid createSession params: {e}"))?;

        self.close_existing_session_if_present(&params.session_id)
            .await?;

        let runtime = self.runtime.ensure_runtime(params.visible).await?;
        let context_id = runtime
            .browser
            .lock()
            .await
            .create_browser_context(CreateBrowserContextParams::default())
            .await
            .map_err(|e| format!("Failed to create isolated browser context: {e}"))?;

        let page = match runtime
            .browser
            .lock()
            .await
            .new_page(
                CreateTargetParams::builder()
                    .url(&params.url)
                    .browser_context_id(context_id.clone())
                    .build()
                    .map_err(|e| format!("Failed to build browser target params: {e}"))?,
            )
            .await
        {
            Ok(page) => page,
            Err(error) => {
                cleanup_failed_context_launch(runtime.browser.clone(), context_id.clone()).await;
                return Err(format!("Failed to open page '{}': {error}", params.url));
            }
        };
        let page = Arc::new(page);
        let state = match snapshot_page_state(&page).await {
            Ok(state) => state,
            Err(error) => {
                let _ = page.as_ref().clone().close().await;
                cleanup_failed_context_launch(runtime.browser.clone(), context_id.clone()).await;
                return Err(error);
            }
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(params.session_id, SidecarSession { context_id, page });

        serde_json::to_value(state)
            .map_err(|e| format!("Failed to serialize createSession result: {e}"))
    }

    async fn close_session(&self, params: Value) -> Result<(), String> {
        let params: SessionIdParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid closeSession params: {e}"))?;
        let session = {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(&params.session_id)
        }
        .ok_or_else(|| format!("Browser session not found: {}", params.session_id))?;
        let runtime = self
            .runtime
            .current_runtime()
            .await
            .ok_or_else(|| "Browser runtime is not running".to_string())?;

        cleanup_session_resources(runtime.browser.clone(), session, &params.session_id).await
    }

    async fn navigate(&self, params: Value) -> Result<Value, String> {
        let params: NavigateParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid navigate params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        page.goto(&params.url)
            .await
            .map_err(|e| format!("Failed to navigate to '{}': {e}", params.url))?;
        let state = snapshot_page_state(&page).await?;
        serde_json::to_value(state).map_err(|e| format!("Failed to serialize navigate result: {e}"))
    }

    async fn go_back(&self, params: Value) -> Result<Value, String> {
        let params: SessionIdParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid goBack params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        let state = perform_history_navigation(&page, HistoryDirection::Back).await?;
        serde_json::to_value(state).map_err(|e| format!("Failed to serialize goBack result: {e}"))
    }

    async fn go_forward(&self, params: Value) -> Result<Value, String> {
        let params: SessionIdParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid goForward params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        let state = perform_history_navigation(&page, HistoryDirection::Forward).await?;
        serde_json::to_value(state)
            .map_err(|e| format!("Failed to serialize goForward result: {e}"))
    }

    async fn evaluate(&self, params: Value) -> Result<Value, String> {
        let params: EvaluateParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid evaluate params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        let result = page
            .evaluate(params.script)
            .await
            .map_err(|e| format!("Failed to evaluate JavaScript: {e}"))?;
        let serialized = serialize_evaluation_result(result)?;
        serde_json::to_value(serialized)
            .map_err(|e| format!("Failed to serialize evaluate result: {e}"))
    }

    async fn get_state(&self, params: Value) -> Result<Value, String> {
        let params: SessionIdParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid getState params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        let state = snapshot_page_state(&page).await?;
        serde_json::to_value(state).map_err(|e| format!("Failed to serialize getState result: {e}"))
    }

    async fn close_existing_session_if_present(&self, session_id: &str) -> Result<(), String> {
        let existing_session = {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(session_id)
        };

        let Some(session) = existing_session else {
            return Ok(());
        };

        let runtime = self
            .runtime
            .current_runtime()
            .await
            .ok_or_else(|| "Browser runtime is not running".to_string())?;

        cleanup_session_resources(runtime.browser.clone(), session, session_id).await
    }

    async fn get_session_page(&self, session_id: &str) -> Result<Arc<chromiumoxide::Page>, String> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(|session| session.page.clone())
            .ok_or_else(|| format!("Browser session not found: {}", session_id))
    }

    async fn shutdown(&self) {
        let sessions = {
            let mut sessions = self.sessions.lock().await;
            std::mem::take(&mut *sessions)
        };

        if let Some(runtime) = self.runtime.take_runtime().await {
            for (session_id, session) in sessions {
                if let Err(error) =
                    cleanup_session_resources(runtime.browser.clone(), session, &session_id).await
                {
                    warn!(
                        "Failed to close browser session {} during sidecar shutdown: {}",
                        session_id, error
                    );
                }
            }

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
        }
    }
}

async fn launch_runtime(visible: bool) -> Result<SharedBrowserRuntime, String> {
    let executable = resolve_browser_executable().await?;
    let mut builder = BrowserConfig::builder().chrome_executable(executable);
    if visible {
        builder = builder.with_head();
    }
    let config = builder
        .build()
        .map_err(|e| format!("Failed to build browser config: {e}"))?;
    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("Failed to launch Chromium automation session: {e}"))?;
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
    })
}

async fn snapshot_page_state(page: &chromiumoxide::Page) -> Result<PageState, String> {
    let snapshot = snapshot_navigation_state(page).await?;
    Ok(page_state_from_snapshot(snapshot))
}

async fn snapshot_navigation_state(
    page: &chromiumoxide::Page,
) -> Result<NavigationSnapshot, String> {
    evaluate_json(
        page,
        r#"(function() {
            let historyState = null;
            try {
                historyState = history.state === undefined ? null : JSON.stringify(history.state);
            } catch (_error) {
                historyState = "__LIBRAGENT_UNSERIALIZABLE_HISTORY_STATE__";
            }

            const bodyText = document.body
                ? ((document.body.innerText || document.body.textContent || '').slice(0, 512))
                : '';

            return {
                url: window.location.href,
                title: document.title || '',
                readyState: document.readyState || '',
                historyLength: Math.max(history.length || 0, 0),
                historyState,
                bodyTextSnippet: bodyText
            };
        })()"#,
    )
    .await
}

fn page_state_from_snapshot(snapshot: NavigationSnapshot) -> PageState {
    let classification =
        classify_browser_page(&snapshot.url, &snapshot.title, &snapshot.body_text_snippet);
    PageState {
        url: snapshot.url,
        title: if snapshot.title.is_empty() {
            None
        } else {
            Some(snapshot.title)
        },
        classification: Some(classification),
        navigation_status: None,
        navigation_message: None,
    }
}

async fn evaluate_json<T: DeserializeOwned>(
    page: &chromiumoxide::Page,
    script: &str,
) -> Result<T, String> {
    page.evaluate(script)
        .await
        .map_err(|e| format!("Failed to evaluate browser state script: {e}"))?
        .into_value()
        .map_err(|e| format!("Failed to decode browser state value: {e}"))
}

async fn perform_history_navigation(
    page: &chromiumoxide::Page,
    direction: HistoryDirection,
) -> Result<PageState, String> {
    let mut before = match snapshot_navigation_state(page).await {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            warn!(
                "{} navigation pre-snapshot failed; continuing without baseline: {}",
                direction.label(),
                error
            );
            None
        }
    };
    let trigger_script = match direction {
        HistoryDirection::Back => "history.back(); 'Navigated back'",
        HistoryDirection::Forward => "history.forward(); 'Navigated forward'",
    };
    page.evaluate(trigger_script)
        .await
        .map_err(|e| format!("Failed to trigger {} navigation: {e}", direction.label()))?;

    let deadline = tokio::time::Instant::now() + HISTORY_NAVIGATION_TIMEOUT;
    let mut latest = before.clone();
    let mut last_snapshot_error: Option<String> = None;
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }

        tokio::time::sleep(HISTORY_NAVIGATION_POLL_INTERVAL).await;
        let current = match snapshot_navigation_state(page).await {
            Ok(current) => {
                last_snapshot_error = None;
                current
            }
            Err(error) => {
                last_snapshot_error = Some(error);
                continue;
            }
        };
        if let Some(previous) = before.as_ref() {
            if navigation_snapshot_changed(previous, &current) {
                let mut state = page_state_from_snapshot(current);
                state.navigation_status = Some(HistoryNavigationStatus::Navigated);
                return Ok(state);
            }
        } else {
            before = Some(current.clone());
        }
        latest = Some(current);
    }

    let Some(latest) = latest else {
        return Err(match last_snapshot_error {
            Some(error) => format!(
                "Failed to observe page state after {} navigation: {}",
                direction.label(),
                error
            ),
            None => format!(
                "Failed to observe page state after {} navigation",
                direction.label()
            ),
        });
    };

    let classification =
        classify_browser_page(&latest.url, &latest.title, &latest.body_text_snippet);
    let mut state = page_state_from_snapshot(latest);
    match classification {
        PageClassification::BlockedInterstitial => {
            state.navigation_status = Some(HistoryNavigationStatus::BlockedInterstitial);
            state.navigation_message = Some(format!(
                "{} navigation did not complete because the current page appears to be a CAPTCHA or blocking interstitial",
                direction.label()
            ));
        }
        PageClassification::Normal => {
            state.navigation_status = Some(HistoryNavigationStatus::NoHistoryEntry);
            state.navigation_message = Some(format!(
                "{} navigation produced no observable page change",
                direction.label()
            ));
        }
    }

    Ok(state)
}

fn navigation_snapshot_changed(before: &NavigationSnapshot, after: &NavigationSnapshot) -> bool {
    before.url != after.url
        || before.title != after.title
        || before.ready_state != after.ready_state
        || before.history_length != after.history_length
        || before.history_state != after.history_state
}

pub fn classify_browser_page(
    url: &str,
    title: &str,
    body_text_snippet: &str,
) -> PageClassification {
    let url_lower = url.to_ascii_lowercase();
    let title_lower = title.to_ascii_lowercase();
    let body_lower = body_text_snippet.to_ascii_lowercase();
    let combined = format!("{title_lower}\n{body_lower}");

    let is_google_sorry = url::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            let host = parsed.host_str()?.to_ascii_lowercase();
            let path = parsed.path().to_ascii_lowercase();
            Some(
                (host == "google.com"
                    || host.ends_with(".google.com")
                    || host.starts_with("google."))
                    && path.starts_with("/sorry"),
            )
        })
        .unwrap_or_else(|| url_lower.contains("google.com/sorry") || url_lower.contains("/sorry/"));

    if is_google_sorry
        || url_lower.contains("captcha")
        || combined.contains("captcha")
        || combined.contains("recaptcha")
        || combined.contains("unusual traffic")
        || combined.contains("verify you are human")
        || combined.contains("verify you're human")
        || combined.contains("checking your browser before accessing")
        || combined.contains("cf challenge")
        || combined.contains("__cf_chl")
    {
        return PageClassification::BlockedInterstitial;
    }

    PageClassification::Normal
}

impl HistoryDirection {
    fn label(self) -> &'static str {
        match self {
            HistoryDirection::Back => "Back",
            HistoryDirection::Forward => "Forward",
        }
    }
}

fn serialize_evaluation_result(
    result: chromiumoxide::js::EvaluationResult,
) -> Result<String, String> {
    serialize_browser_result_value(result.value().cloned())
}

pub fn serialize_browser_result_value(value: Option<Value>) -> Result<String, String> {
    Ok(match value {
        None => "undefined".to_string(),
        Some(Value::String(text)) => text,
        Some(Value::Null) => "null".to_string(),
        Some(other) => serde_json::to_string(&other)
            .map_err(|e| format!("Failed to serialize JavaScript result: {e}"))?,
    })
}

async fn resolve_browser_executable() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("LIBRAGENT_BROWSER_EXECUTABLE") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!(
            "LIBRAGENT_BROWSER_EXECUTABLE points to a missing browser executable: {}",
            path.display()
        ));
    }

    if let Ok(path) = default_executable(DetectionOptions::default()) {
        return Ok(path);
    }

    let base_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.fritzprix.libragent")
        .join("browser-runtime");
    tokio::fs::create_dir_all(&base_dir)
        .await
        .map_err(|e| format!("Failed to create browser runtime cache directory: {e}"))?;

    let fetcher = BrowserFetcher::new(
        BrowserFetcherOptions::builder()
            .with_path(&base_dir)
            .build()
            .map_err(|e| format!("Failed to configure Chromium fetcher: {e}"))?,
    );
    let info = fetcher
        .fetch()
        .await
        .map_err(|e| format!("Failed to download bundled Chromium runtime: {e}"))?;
    Ok(info.executable_path)
}

async fn cleanup_failed_context_launch(browser: Arc<Mutex<Browser>>, context_id: BrowserContextId) {
    if let Err(error) = browser
        .lock()
        .await
        .dispose_browser_context(context_id)
        .await
    {
        warn!("Failed to dispose partially initialized browser context: {error}");
    }
}

async fn cleanup_session_resources(
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

    let context_close = tokio::time::timeout(SESSION_CLEANUP_TIMEOUT, async {
        browser
            .lock()
            .await
            .dispose_browser_context(session.context_id)
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

    if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(cleanup_errors.join("; "))
    }
}
