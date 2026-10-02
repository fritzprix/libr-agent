//! In-process Chrome MV3 extension WebSocket bridge (loopback only).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use log::{info, warn};
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex, RwLock};
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Filter;

use super::messages::{
    decode_reply, resolve_backend_mode, resolve_bridge_port, resolve_bridge_token, token_hint,
    BridgeQuery, ExtensionBridgeStatus, ExtensionMethod, ExtensionReply, ExtensionRequest,
    ExtensionTabState, DEFAULT_RPC_TIMEOUT_SECS,
};
use crate::browser_sidecar::PageState;

static GLOBAL_BRIDGE: OnceLock<ExtensionBridge> = OnceLock::new();

struct PendingSlot {
    sender: oneshot::Sender<Result<Value, String>>,
}

struct BridgeInner {
    connected: AtomicBool,
    port: u16,
    token: String,
    /// Active outbound sink to the single connected extension.
    outbound: Mutex<Option<futures::stream::SplitSink<WebSocket, Message>>>,
    pending: dashmap::DashMap<String, PendingSlot>,
    /// session_id → last known tab id (informational; tabs owned by extension).
    session_tabs: RwLock<HashMap<String, i64>>,
    started: AtomicBool,
}

/// Cloneable handle to the process-wide extension bridge.
#[derive(Clone)]
pub struct ExtensionBridge {
    inner: Arc<BridgeInner>,
}

impl ExtensionBridge {
    fn new(port: u16, token: String) -> Self {
        Self {
            inner: Arc::new(BridgeInner {
                connected: AtomicBool::new(false),
                port,
                token,
                outbound: Mutex::new(None),
                pending: dashmap::DashMap::new(),
                session_tabs: RwLock::new(HashMap::new()),
                started: AtomicBool::new(false),
            }),
        }
    }

    pub fn global() -> ExtensionBridge {
        GLOBAL_BRIDGE
            .get_or_init(|| {
                let port = resolve_bridge_port();
                let token = resolve_bridge_token();
                ExtensionBridge::new(port, token)
            })
            .clone()
    }

    /// Idempotent: spawn the loopback WS server once.
    pub fn ensure_started(&self) {
        if self
            .inner
            .started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let bridge = self.clone();
        let port = self.inner.port;
        tauri::async_runtime::spawn(async move {
            let bridge_for_reset = bridge.clone();
            if let Err(error) = bridge.serve(port).await {
                warn!("Extension bridge server exited: {error}");
                bridge_for_reset
                    .inner
                    .started
                    .store(false, Ordering::SeqCst);
                bridge_for_reset
                    .inner
                    .connected
                    .store(false, Ordering::SeqCst);
            }
        });
        info!(
            "Chrome extension bridge listening on ws://127.0.0.1:{port}/extension-bridge (token hint: {})",
            token_hint(&self.inner.token)
        );
    }

    pub fn is_connected(&self) -> bool {
        self.inner.connected.load(Ordering::SeqCst)
    }

    pub fn status(&self) -> ExtensionBridgeStatus {
        ExtensionBridgeStatus {
            connected: self.is_connected(),
            port: self.inner.port,
            token_hint: token_hint(&self.inner.token),
            backend_mode: resolve_backend_mode(),
        }
    }

    /// Whether new sessions should prefer the extension backend.
    pub fn should_route_new_sessions_to_extension(&self) -> bool {
        match resolve_backend_mode().as_str() {
            "sidecar" => false,
            "extension" => true,
            _ => self.is_connected(),
        }
    }

    pub async fn create_session(&self, session_id: &str, url: &str) -> Result<PageState, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::CreateSession,
                json!({
                    "sessionId": session_id,
                    "url": url,
                }),
            )
            .await?;
        let state = parse_tab_state(result)?;
        if let Some(tab_id) = state.tab_id {
            let mut map = self.inner.session_tabs.write().await;
            map.insert(session_id.to_string(), tab_id);
        }
        Ok(page_state_from_tab(state))
    }

    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<PageState, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::Navigate,
                json!({
                    "sessionId": session_id,
                    "url": url,
                }),
            )
            .await?;
        let state = parse_tab_state(result)?;
        if let Some(tab_id) = state.tab_id {
            let mut map = self.inner.session_tabs.write().await;
            map.insert(session_id.to_string(), tab_id);
        }
        Ok(page_state_from_tab(state))
    }

    pub async fn close_session(&self, session_id: &str) -> Result<(), String> {
        if !self.is_connected() {
            // Extension already gone — treat as closed.
            let mut map = self.inner.session_tabs.write().await;
            map.remove(session_id);
            return Ok(());
        }
        let _ = self
            .rpc(
                ExtensionMethod::CloseSession,
                json!({ "sessionId": session_id }),
            )
            .await?;
        let mut map = self.inner.session_tabs.write().await;
        map.remove(session_id);
        Ok(())
    }

    pub async fn get_state(&self, session_id: &str) -> Result<PageState, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::GetState,
                json!({ "sessionId": session_id }),
            )
            .await?;
        Ok(page_state_from_tab(parse_tab_state(result)?))
    }

    /// Run arbitrary page JS in the extension-owned tab (MAIN world).
    pub async fn evaluate(&self, session_id: &str, script: &str) -> Result<String, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::Evaluate,
                json!({
                    "sessionId": session_id,
                    "script": script,
                }),
            )
            .await?;
        Ok(stringify_evaluate_result(result))
    }

    pub async fn go_back(&self, session_id: &str) -> Result<PageState, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::GoBack,
                json!({ "sessionId": session_id }),
            )
            .await?;
        Ok(page_state_from_tab(parse_tab_state(result)?))
    }

    pub async fn go_forward(&self, session_id: &str) -> Result<PageState, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::GoForward,
                json!({ "sessionId": session_id }),
            )
            .await?;
        Ok(page_state_from_tab(parse_tab_state(result)?))
    }

    /// Capture the visible tab as standard base64 PNG (no data-URL prefix).
    /// `full_page` is accepted for API parity; the MV3 bridge currently captures
    /// the visible viewport only.
    pub async fn take_screenshot(
        &self,
        session_id: &str,
        full_page: bool,
    ) -> Result<String, String> {
        if !self.is_connected() {
            return Err("Chrome extension bridge is not connected".to_string());
        }
        let result = self
            .rpc(
                ExtensionMethod::TakeScreenshot,
                json!({
                    "sessionId": session_id,
                    "fullPage": full_page,
                }),
            )
            .await?;
        match result {
            Value::String(data) => Ok(strip_data_url_base64(&data)),
            other => other
                .get("base64")
                .and_then(|v| v.as_str())
                .map(|s| strip_data_url_base64(s))
                .ok_or_else(|| {
                    format!("Invalid extension screenshot payload: expected string, got {other}")
                }),
        }
    }

    async fn rpc(&self, method: ExtensionMethod, params: Value) -> Result<Value, String> {
        let id = Uuid::new_v4().to_string();
        let request = ExtensionRequest {
            id: id.clone(),
            method: method.as_str().to_string(),
            params,
        };
        let payload = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to encode extension bridge request: {e}"))?;

        let (tx, rx) = oneshot::channel();
        self.inner
            .pending
            .insert(id.clone(), PendingSlot { sender: tx });

        {
            let mut outbound = self.inner.outbound.lock().await;
            let Some(sink) = outbound.as_mut() else {
                self.inner.pending.remove(&id);
                return Err("Chrome extension bridge is not connected".to_string());
            };
            if let Err(error) = sink.send(Message::text(payload)).await {
                self.inner.pending.remove(&id);
                self.mark_disconnected().await;
                return Err(format!("Failed to send to Chrome extension: {error}"));
            }
        }

        let timeout = Duration::from_secs(DEFAULT_RPC_TIMEOUT_SECS);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.inner.pending.remove(&id);
                Err("Extension bridge response channel closed unexpectedly".to_string())
            }
            Err(_) => {
                self.inner.pending.remove(&id);
                Err(format!(
                    "Chrome extension did not respond within {}s for {}",
                    DEFAULT_RPC_TIMEOUT_SECS,
                    method.as_str()
                ))
            }
        }
    }

    async fn serve(self, port: u16) -> Result<(), String> {
        let bridge = self.clone();
        let expected_token = self.inner.token.clone();

        let ws_route = warp::path("extension-bridge")
            .and(warp::ws())
            .and(warp::query::<BridgeQuery>())
            .and(warp::any().map(move || bridge.clone()))
            .and(warp::any().map(move || expected_token.clone()))
            .map(
                |ws: warp::ws::Ws,
                 query: BridgeQuery,
                 bridge: ExtensionBridge,
                 expected_token: String| {
                    ws.on_upgrade(move |socket| async move {
                        handle_socket(socket, query, bridge, expected_token).await;
                    })
                },
            );

        let health = warp::path("extension-bridge")
            .and(warp::path("health"))
            .and(warp::get())
            .map(|| warp::reply::with_status("ok", warp::http::StatusCode::OK));

        let routes = ws_route.or(health);

        let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
        info!("Binding Chrome extension bridge on {addr}");
        warp::serve(routes).try_bind(addr).await;
        Err(format!("Chrome extension bridge server on {addr} stopped"))
    }

    async fn mark_disconnected(&self) {
        self.inner.connected.store(false, Ordering::SeqCst);
        {
            let mut outbound = self.inner.outbound.lock().await;
            *outbound = None;
        }
        // Fail any in-flight RPCs.
        let keys: Vec<String> = self
            .inner
            .pending
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        for key in keys {
            if let Some((_, slot)) = self.inner.pending.remove(&key) {
                let _ = slot
                    .sender
                    .send(Err("Chrome extension disconnected".to_string()));
            }
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    query: BridgeQuery,
    bridge: ExtensionBridge,
    expected_token: String,
) {
    let provided = query.token.unwrap_or_default();
    // MVP: accept empty token only when expecting the fixed dev token (first-connection ease),
    // otherwise require an exact match.
    let token_ok = provided == expected_token
        || (provided.is_empty() && expected_token == super::messages::DEV_BRIDGE_TOKEN);
    if !token_ok {
        warn!("Rejected Chrome extension bridge connection: invalid token");
        let mut socket = socket;
        let _ = socket
            .send(Message::close_with(4001u16, "invalid token"))
            .await;
        return;
    }

    // Single-client: replace any existing connection.
    {
        let mut outbound = bridge.inner.outbound.lock().await;
        if outbound.is_some() {
            warn!("Replacing existing Chrome extension bridge connection");
        }
        *outbound = None;
    }

    let (sink, mut stream) = socket.split();
    {
        let mut outbound = bridge.inner.outbound.lock().await;
        *outbound = Some(sink);
    }
    bridge.inner.connected.store(true, Ordering::SeqCst);
    info!("Chrome extension connected to bridge");

    while let Some(message_result) = stream.next().await {
        let message = match message_result {
            Ok(m) => m,
            Err(error) => {
                warn!("Chrome extension WebSocket read error: {error}");
                break;
            }
        };

        if message.is_close() {
            break;
        }
        if !message.is_text() {
            continue;
        }

        let text = match message.to_str() {
            Ok(t) => t,
            Err(_) => continue,
        };

        match decode_reply(text) {
            Ok(reply) => complete_pending(&bridge, reply),
            Err(error) => warn!("Ignoring invalid extension reply: {error}"),
        }
    }

    bridge.mark_disconnected().await;
    info!("Chrome extension disconnected from bridge");
}

fn complete_pending(bridge: &ExtensionBridge, reply: ExtensionReply) {
    let Some(id) = reply.id.clone() else {
        warn!("Extension reply missing id; ignoring");
        return;
    };
    let Some((_, slot)) = bridge.inner.pending.remove(&id) else {
        warn!("No pending RPC for extension reply id {id}");
        return;
    };

    let result = if reply.ok {
        Ok(reply.result.unwrap_or(Value::Null))
    } else {
        Err(reply
            .error
            .unwrap_or_else(|| "Chrome extension reported an error".to_string()))
    };
    let _ = slot.sender.send(result);
}

fn parse_tab_state(value: Value) -> Result<ExtensionTabState, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid extension tab state payload: {e}"))
}

fn page_state_from_tab(state: ExtensionTabState) -> PageState {
    PageState {
        url: state.url,
        title: state.title,
        classification: None,
        navigation_status: None,
        navigation_message: Some("Opened via Chrome extension bridge".to_string()),
    }
}

fn stringify_evaluate_result(value: Value) -> String {
    match value {
        Value::String(s) => s,
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn strip_data_url_base64(raw: &str) -> String {
    if let Some(idx) = raw.find("base64,") {
        raw[idx + "base64,".len()..].to_string()
    } else {
        raw.to_string()
    }
}
