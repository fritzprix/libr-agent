//! JSON message types for the Chrome MV3 extension WebSocket bridge.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_BRIDGE_PORT: u16 = 3847;
pub const DEV_BRIDGE_TOKEN: &str = "libragent-dev";
pub const DEFAULT_RPC_TIMEOUT_SECS: u64 = 30;

/// Methods the desktop app may send to the extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionMethod {
    CreateSession,
    Navigate,
    CloseSession,
    GetState,
    Ping,
}

impl ExtensionMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateSession => "createSession",
            Self::Navigate => "navigate",
            Self::CloseSession => "closeSession",
            Self::GetState => "getState",
            Self::Ping => "ping",
        }
    }
}

/// Outbound RPC request (app → extension).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRequest {
    pub id: String,
    pub method: String,
    pub params: Value,
}

/// Inbound RPC reply (extension → app).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionReply {
    pub id: Option<String>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Tab / page state returned by createSession / navigate / getState.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionTabState {
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub tab_id: Option<i64>,
}

/// Status snapshot for Settings UI / Tauri commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionBridgeStatus {
    pub connected: bool,
    pub port: u16,
    /// Non-secret hint (e.g. `libragent-dev` or `set via env`).
    pub token_hint: String,
    pub backend_mode: String,
}

#[derive(Debug, Deserialize)]
pub struct BridgeQuery {
    pub token: Option<String>,
}

/// Parse a reply JSON payload from the extension.
pub fn decode_reply(raw: &str) -> Result<ExtensionReply, String> {
    serde_json::from_str(raw).map_err(|e| format!("Invalid extension bridge reply JSON: {e}"))
}

/// Parse an outbound request (used by unit/integration tests).
pub fn decode_request(raw: &str) -> Result<ExtensionRequest, String> {
    serde_json::from_str(raw).map_err(|e| format!("Invalid extension bridge request JSON: {e}"))
}

pub fn resolve_bridge_port() -> u16 {
    std::env::var("LIBRAGENT_EXTENSION_BRIDGE_PORT")
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .filter(|p| *p > 0)
        .unwrap_or(DEFAULT_BRIDGE_PORT)
}

/// Token the bridge expects. Unset env → fixed MVP dev token.
pub fn resolve_bridge_token() -> String {
    std::env::var("LIBRAGENT_EXTENSION_BRIDGE_TOKEN")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| DEV_BRIDGE_TOKEN.to_string())
}

pub fn token_hint(token: &str) -> String {
    if token == DEV_BRIDGE_TOKEN {
        DEV_BRIDGE_TOKEN.to_string()
    } else if std::env::var("LIBRAGENT_EXTENSION_BRIDGE_TOKEN").is_ok() {
        "set via env".to_string()
    } else {
        "custom".to_string()
    }
}

/// `auto` (default) | `extension` | `sidecar`
pub fn resolve_backend_mode() -> String {
    std::env::var("LIBRAGENT_BROWSER_BACKEND")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| matches!(v.as_str(), "auto" | "extension" | "sidecar"))
        .unwrap_or_else(|| "auto".to_string())
}
