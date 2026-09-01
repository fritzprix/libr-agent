use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use chromiumoxide::cdp::browser_protocol::target::{
    CreateBrowserContextParams, CreateTargetParams,
};
use log::warn;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinSet;

use super::contracts::{
    ConsoleEntry, CreateSessionParams, EvaluateParams, GetConsoleLogsParams, NavigateParams,
    SessionIdParams, SidecarRequest, SidecarResponse, TakeScreenshotParams,
};
use super::page::{
    capture_screenshot, navigate_back, navigate_forward, serialize_evaluation_result,
    snapshot_page_state,
};
use super::runtime::{
    cleanup_failed_context_launch, cleanup_session_resources, shutdown_runtime,
    BrowserRuntimeManager, SidecarSession,
};

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
    console_listeners: Mutex<HashMap<String, tokio::task::AbortHandle>>,
}

impl BrowserSidecarServer {
    fn new() -> Self {
        Self {
            runtime: BrowserRuntimeManager::new(),
            sessions: Mutex::new(HashMap::new()),
            console_listeners: Mutex::new(HashMap::new()),
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
                        id: extract_request_id_from_line(&line)
                            .unwrap_or_else(|| String::from("invalid")),
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
            "takeScreenshot" => self.take_screenshot(request.params).await,
            "getState" => self.get_state(request.params).await,
            "getConsoleLogs" => self.get_console_logs(request.params).await,
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

        let runtime = self.runtime.ensure_runtime(params.visible).await?;
        self.close_existing_session_if_present(&params.session_id)
            .await?;
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

        // Attach console event listener
        use futures::StreamExt;
        if let Err(e) = page.enable_runtime().await {
            warn!("Failed to enable runtime domain for console event listener: {e}");
        }

        // Abort existing console listener task if any
        {
            let mut listeners = self.console_listeners.lock().await;
            if let Some(handle) = listeners.remove(&params.session_id) {
                handle.abort();
            }
        }

        let console_logs = runtime.console_logs.clone();
        let session_id = params.session_id.clone();
        match page
            .event_listener::<chromiumoxide::cdp::js_protocol::runtime::EventConsoleApiCalled>()
            .await
        {
            Ok(mut events) => {
                let handle = tokio::spawn(async move {
                    while let Some(event) = events.next().await {
                        let level = format!("{:?}", event.r#type).to_lowercase();
                        let text = event
                            .args
                            .iter()
                            .map(|arg| {
                                if let Some(val) = &arg.value {
                                    if let Some(s) = val.as_str() {
                                        s.to_string()
                                    } else {
                                        val.to_string()
                                    }
                                } else if let Some(desc) = &arg.description {
                                    desc.clone()
                                } else {
                                    format!("{:?}", arg.r#type)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" ");

                        let timestamp = serde_json::to_value(&event.timestamp)
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);

                        let entry = ConsoleEntry {
                            level,
                            text,
                            timestamp,
                        };

                        let mut logs = console_logs.write().await;
                        let entries = logs.entry(session_id.clone()).or_insert_with(Vec::new);
                        entries.push(entry);
                        if entries.len() > 1000 {
                            entries.remove(0);
                        }
                    }
                });
                let mut listeners = self.console_listeners.lock().await;
                listeners.insert(params.session_id.clone(), handle.abort_handle());
            }
            Err(e) => {
                warn!("Failed to subscribe to console events: {e}");
            }
        }

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

        // Clean up console logs
        {
            let mut logs = runtime.console_logs.write().await;
            logs.remove(&params.session_id);
        }

        // Clean up console listener task
        {
            let mut listeners = self.console_listeners.lock().await;
            if let Some(handle) = listeners.remove(&params.session_id) {
                handle.abort();
            }
        }

        let cleanup_res =
            cleanup_session_resources(runtime.browser.clone(), session, &params.session_id).await;

        // Cascade shutdown: if no other sessions exist, shutdown the browser runtime
        let is_empty = {
            let sessions = self.sessions.lock().await;
            sessions.is_empty()
        };
        if is_empty {
            if let Some(runtime) = self.runtime.take_runtime().await {
                // Abort all active console listener tasks
                {
                    let mut listeners = self.console_listeners.lock().await;
                    for handle in listeners.values() {
                        handle.abort();
                    }
                    listeners.clear();
                }
                shutdown_runtime(runtime).await;
            }
        }

        cleanup_res
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
        let state = navigate_back(&page).await?;
        serde_json::to_value(state).map_err(|e| format!("Failed to serialize goBack result: {e}"))
    }

    async fn go_forward(&self, params: Value) -> Result<Value, String> {
        let params: SessionIdParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid goForward params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        let state = navigate_forward(&page).await?;
        serde_json::to_value(state)
            .map_err(|e| format!("Failed to serialize goForward result: {e}"))
    }

    async fn evaluate(&self, params: Value) -> Result<Value, String> {
        let params: EvaluateParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid evaluate params: {e}"))?;

        const MAX_SCRIPT_LENGTH: usize = 65_536; // 64KB
        if params.script.len() > MAX_SCRIPT_LENGTH {
            return Err("Script length exceeds maximum limit of 64KB".to_string());
        }

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

    async fn take_screenshot(&self, params: Value) -> Result<Value, String> {
        let params: TakeScreenshotParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid takeScreenshot params: {e}"))?;
        let page = self.get_session_page(&params.session_id).await?;
        let screenshot = capture_screenshot(&page, params.full_page).await?;
        Ok(Value::String(
            base64::engine::general_purpose::STANDARD.encode(screenshot),
        ))
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

        // Clean up console logs
        {
            let mut logs = runtime.console_logs.write().await;
            logs.remove(session_id);
        }

        // Clean up console listener task
        {
            let mut listeners = self.console_listeners.lock().await;
            if let Some(handle) = listeners.remove(session_id) {
                handle.abort();
            }
        }

        cleanup_session_resources(runtime.browser.clone(), session, session_id).await
    }

    async fn get_session_page(&self, session_id: &str) -> Result<Arc<chromiumoxide::Page>, String> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(|session| session.page.clone())
            .ok_or_else(|| format!("Browser session not found: {}", session_id))
    }

    async fn get_console_logs(&self, params: Value) -> Result<Value, String> {
        let params: GetConsoleLogsParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid getConsoleLogs params: {e}"))?;
        let runtime = self
            .runtime
            .current_runtime()
            .await
            .ok_or_else(|| "Browser runtime is not running".to_string())?;

        let logs = {
            let logs_guard = runtime.console_logs.read().await;
            let entries = logs_guard.get(&params.session_id);
            let limit = params.max_entries.unwrap_or(100) as usize;
            entries
                .map(|list| {
                    let len = list.len();
                    let skip = len.saturating_sub(limit);
                    list[skip..].to_vec()
                })
                .unwrap_or_default()
        };

        serde_json::to_value(logs).map_err(|e| format!("Failed to serialize console logs: {e}"))
    }

    async fn shutdown(&self) {
        let sessions = {
            let mut sessions = self.sessions.lock().await;
            std::mem::take(&mut *sessions)
        };

        if let Some(runtime) = self.runtime.take_runtime().await {
            // Clean up all console logs
            {
                let mut logs = runtime.console_logs.write().await;
                logs.clear();
            }

            // Abort all active console listener tasks
            {
                let mut listeners = self.console_listeners.lock().await;
                for handle in listeners.values() {
                    handle.abort();
                }
                listeners.clear();
            }

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

            shutdown_runtime(runtime).await;
        }
    }
}

fn extract_request_id_from_line(line: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    value.get("id")?.as_str().map(ToString::to_string)
}
