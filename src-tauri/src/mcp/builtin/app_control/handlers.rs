use super::text_result;
use crate::mcp::types::MCPResult;
use crate::state::get_app_handle;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use tauri::Emitter;

pub const APP_CONTROL_EVENT: &str = "libragent:app-control";

#[derive(Debug)]
pub struct AppControlError(String);

impl fmt::Display for AppControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for AppControlError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AppControlError {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum AppControlPayload {
    Navigate {
        path: String,
    },
    Highlight {
        target: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
    },
    InstallPreset {
        name: String,
    },
    FocusSession {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
}

fn emit_payload(payload: &AppControlPayload) -> Result<(), AppControlError> {
    let Some(app_handle) = get_app_handle() else {
        return Err(AppControlError(
            "AppHandle not registered — open the LibrAgent desktop UI before using app control"
                .to_string(),
        ));
    };
    app_handle
        .emit(APP_CONTROL_EVENT, payload)
        .map_err(|e| AppControlError(format!("Failed to emit {APP_CONTROL_EVENT}: {e}")))?;
    Ok(())
}

fn require_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, AppControlError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppControlError(format!("Missing required string parameter: {key}")))
}

fn optional_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(|v| v.as_u64())
}

pub fn navigate(args: Value) -> Result<MCPResult, AppControlError> {
    let path = require_string(&args, "path")?;
    if !path.starts_with('/') || path.starts_with("//") {
        return Err(AppControlError(
            "path must be an absolute in-app route starting with / (not //)".to_string(),
        ));
    }
    emit_payload(&AppControlPayload::Navigate {
        path: path.to_string(),
    })?;
    Ok(text_result(format!("Navigating to {path}")))
}

pub fn highlight(args: Value) -> Result<MCPResult, AppControlError> {
    let target = require_string(&args, "target")?;
    if target != "preset" {
        return Err(AppControlError(
            "target must be \"preset\" in v1".to_string(),
        ));
    }
    let name = require_string(&args, "name")?;
    let ms = optional_u64(&args, "ms");
    emit_payload(&AppControlPayload::Highlight {
        target: "preset".to_string(),
        name: name.to_string(),
        ms,
    })?;
    Ok(text_result(format!(
        "Highlighting preset \"{name}\"{}",
        ms.map(|m| format!(" for {m}ms")).unwrap_or_default()
    )))
}

pub fn install_preset(args: Value) -> Result<MCPResult, AppControlError> {
    let name = require_string(&args, "name")?;
    emit_payload(&AppControlPayload::InstallPreset {
        name: name.to_string(),
    })?;
    Ok(text_result(format!(
        "Requested one-click install for preset \"{name}\" (UI performs install; wait ~500–800ms)"
    )))
}

pub fn focus_session(args: Value) -> Result<MCPResult, AppControlError> {
    let session_id = args
        .get("sessionId")
        .or_else(|| args.get("session_id"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppControlError("Missing required string parameter: sessionId".into()))?;
    emit_payload(&AppControlPayload::FocusSession {
        session_id: session_id.to_string(),
    })?;
    Ok(text_result(format!(
        "Focusing session UI at /agent/{session_id}"
    )))
}

pub async fn wait_ui(args: Value) -> Result<MCPResult, AppControlError> {
    let mut ms = optional_u64(&args, "ms").unwrap_or(500);
    if ms > 30_000 {
        ms = 30_000;
    }
    let path_hint = args
        .get("ready")
        .and_then(|r| r.get("path"))
        .and_then(|p| p.as_str())
        .map(str::to_string);
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    Ok(text_result(format!(
        "Waited {ms}ms{}",
        path_hint
            .map(|p| format!(" (ready hint path={p})"))
            .unwrap_or_default()
    )))
}
