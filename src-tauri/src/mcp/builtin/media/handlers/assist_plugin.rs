//! MediaAssist host plugin status and deploy handlers.

use serde_json::Value;
use std::path::Path;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};

async fn host_plugin_blocked_for_session(session_id: &str) -> Option<String> {
    if crate::mcp::builtin::workspace::utils::is_session_docker_isolated(session_id).await {
        return Some(
            "MediaAssist host plugins are disabled for Docker/Harbor sessions to prevent container breakout. Use Host isolation, or convert media inside the container with ffmpeg/CLI tools.".to_string(),
        );
    }
    None
}

pub async fn handle_assist_plugin_status(
    base_data_dir: &Path,
    session_id: &str,
) -> Result<MCPResult, String> {
    if let Some(blocked) = host_plugin_blocked_for_session(session_id).await {
        return Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: blocked.clone(),
            }]),
            structured_content: Some(serde_json::json!({
                "installed": false,
                "hostExecutionAllowed": false,
                "error": "docker_isolation_blocked",
                "message": blocked,
            })),
            is_error: Some(false),
        });
    }

    let status = crate::media_assist::load_status(base_data_dir);
    let json = serde_json::to_value(&status).map_err(|e| e.to_string())?;
    let text = if status.installed {
        format!(
            "✓ MediaAssist plugin installed at {}\nmodalities={:?}\ntimeoutMs={}",
            status.path, status.modalities, status.timeout_ms
        )
    } else {
        let mut text = format!(
            "MediaAssist plugin not installed at {}.\nLoad @skill:libragent-plugin to implement, verify, and media__deployAssistPlugin.",
            status.path
        );
        if let Some(error) = status.error.as_ref().filter(|e| !e.is_empty()) {
            text.push_str(&format!("\nDiagnostic: {error}"));
        }
        text
    };
    Ok(MCPResult {
        content: Some(vec![MCPContent::Text { text }]),
        structured_content: Some(json),
        is_error: Some(false),
    })
}

pub async fn handle_deploy_assist_plugin(
    args: Value,
    base_data_dir: &Path,
    session_id: &str,
) -> Result<MCPResult, String> {
    if let Some(blocked) = host_plugin_blocked_for_session(session_id).await {
        return Ok(guided_error(
            ErrorCategory::PermissionDenied,
            blocked,
            ToolGroup::Media,
        )
        .with_guidance(vec![
            "MediaAssist plugins run on the host and are blocked under Docker/Harbor isolation."
                .to_string(),
            "Switch the session to Host isolation to deploy, or keep conversion inside the container."
                .to_string(),
        ])
        .to_mcp_result());
    }

    let Some(files_val) = args.get("files") else {
        return Ok(missing_param_error("files", ToolGroup::Media));
    };
    let files: Vec<crate::media_assist::DeployFile> =
        match serde_json::from_value(files_val.clone()) {
            Ok(files) => files,
            Err(error) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("invalid files: {error}"),
                    ToolGroup::Media,
                )
                .with_guidance(vec![
                    "files must be an array of { path, content, base64? } objects.".to_string(),
                    "Include at least manifest.json and run (or run.cmd on Windows).".to_string(),
                ])
                .to_mcp_result());
            }
        };
    match crate::media_assist::deploy_files(base_data_dir, &files) {
        Ok(status) => {
            let json = serde_json::to_value(&status).map_err(|e| e.to_string())?;
            Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!(
                        "✓ MediaAssist plugin deployed to {}\nmodalities={:?}",
                        status.path, status.modalities
                    ),
                }]),
                structured_content: Some(json),
                is_error: Some(false),
            })
        }
        Err(error) => Ok(guided_error(
            ErrorCategory::InvalidInput,
            error,
            ToolGroup::Media,
        )
        .with_guidance(vec![
            "Include both manifest.json (interfaceVersion=1) and an executable run script.".to_string(),
            "Allowed paths: manifest.json, run (or run.exe/run.cmd/run.bat on Windows), README.md, fixtures/* only.".to_string(),
            "Follow @skill:libragent-plugin verify before deploy. Deploy requires hard user approval (not YOLO-bypassable).".to_string(),
        ])
        .to_mcp_result()),
    }
}
