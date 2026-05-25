use std::collections::HashMap;
use std::sync::Arc;

use chromiumoxide::cdp::browser_protocol::target::{
    CreateBrowserContextParams, CreateTargetParams,
};
use log::warn;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinSet;

use super::contracts::{
    CreateSessionParams, EvaluateParams, NavigateParams, SessionIdParams, SidecarRequest,
    SidecarResponse,
};
use super::page::{
    navigate_back, navigate_forward, serialize_evaluation_result, snapshot_page_state,
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

            shutdown_runtime(runtime).await;
        }
    }
}

fn extract_request_id_from_line(line: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    value.get("id")?.as_str().map(ToString::to_string)
}
