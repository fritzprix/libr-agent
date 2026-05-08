use super::contracts::{AgentResponse, ExecuteUiTauriActionRequest};
use crate::agent::tools::{create_error_tool_result, create_tool_result_message};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::agent::AgentSessionManager;
use crate::models::chat::{Message, MessageSource};
use tauri::{command, AppHandle, State};

fn read_required_string(params: &serde_json::Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("UI action parameter '{}' must be a string", key))
}

fn read_optional_string(params: &serde_json::Value, key: &str) -> Result<Option<String>, String> {
    match params.get(key) {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => value
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| format!("UI action parameter '{}' must be a string", key)),
        None => Ok(None),
    }
}

fn read_required_string_array(
    params: &serde_json::Value,
    key: &str,
) -> Result<Vec<String>, String> {
    let values = params
        .get(key)
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("UI action parameter '{}' must be an array of strings", key))?;

    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("UI action parameter '{}' must contain only strings", key))
        })
        .collect()
}

fn create_ui_tool_call_message(
    session_id: &str,
    tool_name: &str,
    params: &serde_json::Value,
) -> Result<(String, Message), String> {
    let tool_call_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let arguments = serde_json::to_string(params)
        .map_err(|error| format!("Failed to serialize UI action parameters: {}", error))?;

    Ok((
        tool_call_id.clone(),
        Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: "assistant".to_string(),
            content: Vec::new(),
            tool_calls: Some(vec![ToolCall {
                id: tool_call_id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: tool_name.to_string(),
                    arguments,
                },
            }]),
            tool_call_id: None,
            is_streaming: Some(false),
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: now,
            updated_at: now,
            source: Some(MessageSource::Ui),
            error: None,
            metadata: None,
        },
    ))
}

async fn execute_ui_tauri_action(
    app_handle: AppHandle,
    request: &ExecuteUiTauriActionRequest,
) -> Result<String, String> {
    match request.tool_name.as_str() {
        "tauri:downloadWorkspaceFile" => {
            crate::commands::download_commands::download_workspace_file(
                app_handle,
                request.session_id.clone(),
                read_required_string(&request.params, "filePath")?,
            )
            .await
        }
        "tauri:downloadMediaFile" => {
            crate::commands::download_commands::download_media_file(
                app_handle,
                Some(request.session_id.clone()),
                read_optional_string(&request.params, "fileName")?,
                read_required_string(&request.params, "mimeType")?,
                read_optional_string(&request.params, "dataBase64")?,
                read_optional_string(&request.params, "fileUrl")?,
            )
            .await
        }
        "tauri:exportAndDownloadZip" => {
            crate::commands::download_commands::export_and_download_zip(
                app_handle,
                request.session_id.clone(),
                read_required_string_array(&request.params, "files")?,
                read_required_string(&request.params, "packageName")?,
            )
            .await
        }
        "tauri:openExternalUrl" => {
            crate::commands::url_commands::open_external_url(read_required_string(
                &request.params,
                "url",
            )?)
            .await?;
            Ok("External URL opened successfully".to_string())
        }
        unsupported => Err(format!("Unsupported UI Tauri action: {}", unsupported)),
    }
}

/// Execute a UI-triggered Tauri action via the backend-owned message lifecycle.
#[command]
pub async fn agent_execute_ui_tauri_action(
    manager: State<'_, AgentSessionManager>,
    app_handle: AppHandle,
    request: ExecuteUiTauriActionRequest,
) -> Result<AgentResponse, String> {
    let (tool_call_id, tool_call_message) =
        create_ui_tool_call_message(&request.session_id, &request.tool_name, &request.params)?;

    let action_result = execute_ui_tauri_action(app_handle, &request).await;

    let (success, result_text, tool_result_message) = match action_result {
        Ok(result_text) => {
            let tool_result_message = create_tool_result_message(
                &request.session_id,
                &tool_call_id,
                result_text.clone(),
                None,
            );
            (true, result_text, tool_result_message)
        }
        Err(error_text) => {
            let tool_result_message =
                create_error_tool_result(&request.session_id, &tool_call_id, &error_text, None);
            (false, error_text, tool_result_message)
        }
    };

    manager
        .inject_messages(
            request.session_id.clone(),
            vec![tool_call_message, tool_result_message],
        )
        .await?;

    Ok(AgentResponse {
        success,
        message: if success {
            format!("UI Tauri action executed: {}", request.tool_name)
        } else {
            format!("UI Tauri action failed: {}", request.tool_name)
        },
        data: Some(serde_json::json!({
            "toolCallId": tool_call_id,
            "result": result_text,
        })),
    })
}
