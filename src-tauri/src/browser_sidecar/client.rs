use std::collections::HashSet;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, warn};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use super::contracts::{
    CreateSessionParams, EvaluateParams, NavigateParams, PageState, SessionIdParams,
    SidecarRequest, SidecarResponse,
};
use super::BROWSER_SIDECAR_FLAG;

const MIN_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(180);
const SIDECAR_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(100);

struct SidecarProcess {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
}

struct BrowserAutomationClientState {
    process: Mutex<Option<SidecarProcess>>,
    pending: dashmap::DashMap<String, oneshot::Sender<Result<Value, String>>>,
    request_timeout: Duration,
    bootstrap_timeout: Duration,
    reset_guard: Mutex<()>,
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
                bootstrap_timeout: derive_bootstrap_timeout(request_timeout),
                reset_guard: Mutex::new(()),
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
        debug!(
            "Creating browser sidecar session {} with bootstrap timeout {:?}",
            session_id, self.state.bootstrap_timeout
        );
        self.request_with_timeout(
            "createSession",
            CreateSessionParams {
                session_id: session_id.to_string(),
                url: url.to_string(),
                title: title.map(ToString::to_string),
                visible,
            },
            self.state.bootstrap_timeout,
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

    pub fn request_timeout(&self) -> Duration {
        self.state.request_timeout
    }

    pub fn bootstrap_timeout(&self) -> Duration {
        self.state.bootstrap_timeout
    }

    pub async fn shutdown(&self) {
        let process = {
            let mut process_guard = self.state.process.lock().await;
            process_guard.take()
        };

        if let Some(process) = process {
            fail_all_pending_with_exclusions(
                &self.state,
                "Browser automation client shut down".to_string(),
                HashSet::new(),
            );

            let stdin = {
                let mut stdin_guard = process.stdin.lock().await;
                stdin_guard.take()
            };
            drop(stdin);

            let mut child = process.child.lock().await;
            match tokio::time::timeout(self.state.request_timeout, child.wait()).await {
                Ok(Ok(_)) => debug!("Browser sidecar exited gracefully during shutdown"),
                Ok(Err(error)) => warn!("Failed waiting for browser sidecar shutdown: {error}"),
                Err(_) => {
                    warn!(
            "Browser sidecar did not exit gracefully during shutdown within {:?}; forcing kill",
            self.state.request_timeout
          );
                    if let Err(error) = child.kill().await {
                        warn!("Failed to kill browser sidecar during shutdown: {error}");
                    }
                    let _ = child.wait().await;
                }
            }
        }
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<T, String> {
        self.request_with_timeout(method, params, self.state.request_timeout)
            .await
    }

    async fn request_with_timeout<T: DeserializeOwned>(
        &self,
        method: &str,
        params: impl Serialize,
        timeout: Duration,
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
            let process = process_guard.as_ref().ok_or_else(|| {
                self.state.pending.remove(&request_id);
                "Browser sidecar is not running".to_string()
            })?;
            process.stdin.clone()
        };

        {
            let mut stdin_guard = stdin.lock().await;
            let stdin = match stdin_guard.as_mut() {
                Some(stdin) => stdin,
                None => {
                    self.state.pending.remove(&request_id);
                    return Err("Browser sidecar stdin is closed".to_string());
                }
            };
            if let Err(error) = stdin.write_all(line.as_bytes()).await {
                self.state.pending.remove(&request_id);
                self.reset_process(format!(
                    "Browser sidecar stdin write failed for {method}: {error}"
                ))
                .await;
                return Err(format!(
                    "Failed to write request to browser sidecar: {error}"
                ));
            }
            if let Err(error) = stdin.write_all(b"\n").await {
                self.state.pending.remove(&request_id);
                self.reset_process(format!(
                    "Browser sidecar request framing failed for {method}: {error}"
                ))
                .await;
                return Err(format!(
                    "Failed to frame request to browser sidecar: {error}"
                ));
            }
            if let Err(error) = stdin.flush().await {
                self.state.pending.remove(&request_id);
                self.reset_process(format!(
                    "Browser sidecar flush failed for {method}: {error}"
                ))
                .await;
                return Err(format!("Failed to flush browser sidecar request: {error}"));
            }
        }

        let response_value = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result?,
            Ok(Err(_)) => {
                self.state.pending.remove(&request_id);
                self.reset_process(format!(
                    "Browser sidecar response channel closed while waiting for {method}"
                ))
                .await;
                return Err("Browser sidecar response channel closed unexpectedly".to_string());
            }
            Err(_) => {
                self.state.pending.remove(&request_id);
                self.reset_process(format!("Browser sidecar timed out while handling {method}"))
                    .await;
                return Err(format!(
                    "Browser sidecar did not respond within {}ms",
                    timeout.as_millis()
                ));
            }
        };

        serde_json::from_value(response_value)
            .map_err(|e| format!("Failed to decode browser sidecar response: {e}"))
    }

    async fn reset_process(&self, reason: String) {
        let _reset_guard = self.state.reset_guard.lock().await;
        let process = {
            let mut process_guard = self.state.process.lock().await;
            process_guard.take()
        };

        if let Some(process) = process {
            warn!("Resetting browser sidecar process: {reason}");
            fail_all_pending_with_exclusions(
                &self.state,
                format!("Browser sidecar was reset: {reason}"),
                HashSet::new(),
            );

            let mut child = process.child.lock().await;
            if let Err(error) = child.kill().await {
                warn!("Failed to kill browser sidecar during reset: {error}");
            }
            let _ = child.wait().await;
        }
    }

    async fn ensure_started(&self) -> Result<(), String> {
        let mut process_guard = self.state.process.lock().await;
        if process_guard.is_some() {
            return Ok(());
        }

        let current_exe = std::env::current_exe().map_err(|e| {
            format!("Failed to resolve current executable for browser sidecar: {e}")
        })?;
        debug!(
            "Spawning browser sidecar process from executable {}",
            current_exe.display()
        );
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
        let stdin = Arc::new(Mutex::new(Some(stdin)));

        spawn_sidecar_stdout_task(self.state.clone(), child.clone(), stdout);
        spawn_sidecar_stderr_task(stderr);
        spawn_sidecar_exit_task(self.state.clone(), child.clone());

        *process_guard = Some(SidecarProcess { child, stdin });
        Ok(())
    }
}

fn derive_bootstrap_timeout(request_timeout: Duration) -> Duration {
    request_timeout.max(MIN_BOOTSTRAP_TIMEOUT)
}

async fn clear_process_if_matches(state: &BrowserAutomationClientState, child: &Arc<Mutex<Child>>) {
    let mut process_guard = state.process.lock().await;
    let matches_current_process = process_guard
        .as_ref()
        .map(|process| Arc::ptr_eq(&process.child, child))
        .unwrap_or(false);
    if matches_current_process {
        *process_guard = None;
    }
}

async fn process_matches_current(
    state: &BrowserAutomationClientState,
    child: &Arc<Mutex<Child>>,
) -> bool {
    state
        .process
        .lock()
        .await
        .as_ref()
        .map(|process| Arc::ptr_eq(&process.child, child))
        .unwrap_or(false)
}

async fn fail_pending_if_process_matches(
    state: &BrowserAutomationClientState,
    child: &Arc<Mutex<Child>>,
    error: String,
) -> bool {
    let process_guard = state.process.lock().await;
    let matches_current_process = process_guard
        .as_ref()
        .map(|process| Arc::ptr_eq(&process.child, child))
        .unwrap_or(false);
    if matches_current_process {
        fail_all_pending_with_exclusions(state, error, HashSet::new());
    }
    matches_current_process
}

fn spawn_sidecar_stdout_task(
    state: Arc<BrowserAutomationClientState>,
    child: Arc<Mutex<Child>>,
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
                    fail_pending_if_process_matches(
                        &state,
                        &child,
                        "Browser sidecar closed its stdout".to_string(),
                    )
                    .await;
                    clear_process_if_matches(&state, &child).await;
                    break;
                }
                Err(error) => {
                    fail_pending_if_process_matches(
                        &state,
                        &child,
                        format!("Failed reading browser sidecar stdout: {error}"),
                    )
                    .await;
                    clear_process_if_matches(&state, &child).await;
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
        loop {
            let wait_result = {
                let mut child_guard = child.lock().await;
                child_guard.try_wait()
            };

            match wait_result {
                Ok(Some(status)) => {
                    warn!("Browser sidecar exited with status: {status}");
                    fail_pending_if_process_matches(
                        &state,
                        &child,
                        format!("Browser sidecar exited unexpectedly: {status}"),
                    )
                    .await;
                    break;
                }
                Ok(None) => {
                    if !process_matches_current(&state, &child).await {
                        break;
                    }
                    tokio::time::sleep(SIDECAR_EXIT_POLL_INTERVAL).await;
                }
                Err(error) => {
                    fail_pending_if_process_matches(
                        &state,
                        &child,
                        format!("Failed waiting for browser sidecar exit: {error}"),
                    )
                    .await;
                    break;
                }
            }
        }
        clear_process_if_matches(&state, &child).await;
    });
}

fn fail_all_pending_with_exclusions(
    state: &BrowserAutomationClientState,
    error: String,
    excluded_request_ids: HashSet<String>,
) {
    let pending_ids: Vec<String> = state
        .pending
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for pending_id in pending_ids {
        if excluded_request_ids.contains(&pending_id) {
            continue;
        }
        if let Some((_, sender)) = state.pending.remove(&pending_id) {
            let _ = sender.send(Err(error.clone()));
        }
    }
}
