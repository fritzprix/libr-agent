//! Tauri commands for MediaAssist host plugins (#1926).
//!
//! Deploy is intentionally MCP-only (`media__deployAssistPlugin`) so it stays
//! behind the sensitive-tool approval gate. Status/run are used by the frontend
//! fallback path during multimodal 400 recovery.
//!
//! When `sessionId` is provided and the session is Docker/Harbor-isolated, status
//! reports not installed and run is rejected — matching MCP tool policy (no host
//! plugin ACE from container sessions).

use crate::mcp::builtin::workspace::utils::is_session_docker_isolated;
use crate::media_assist::{self, PluginStatus, RunRequest, RunResponse};
use crate::session::get_session_manager;

fn base_data_dir() -> Result<std::path::PathBuf, String> {
    Ok(get_session_manager()?.get_base_data_dir().clone())
}

fn docker_isolation_block_message() -> String {
    "MediaAssist host plugins are disabled for Docker/Harbor sessions to prevent container breakout. Use Host isolation, or convert media inside the container with ffmpeg/CLI tools.".to_string()
}

async fn reject_if_docker_isolated(session_id: Option<&str>) -> Result<(), String> {
    let Some(session_id) = session_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    if is_session_docker_isolated(session_id).await {
        return Err(docker_isolation_block_message());
    }
    Ok(())
}

#[tauri::command]
pub async fn media_assist_plugin_status(
    session_id: Option<String>,
) -> Result<PluginStatus, String> {
    if let Err(message) = reject_if_docker_isolated(session_id.as_deref()).await {
        let base = base_data_dir()?;
        let path = media_assist::plugin_dir(&base).display().to_string();
        return Ok(PluginStatus {
            installed: false,
            path,
            modalities: vec![],
            timeout_ms: 120_000,
            error: Some(message),
        });
    }
    let base = base_data_dir()?;
    Ok(media_assist::load_status(&base))
}

#[tauri::command]
pub async fn media_assist_run_plugin(
    request: RunRequest,
    session_id: Option<String>,
) -> Result<RunResponse, String> {
    reject_if_docker_isolated(session_id.as_deref()).await?;
    let base = base_data_dir()?;
    media_assist::run_plugin(&base, request).await
}
