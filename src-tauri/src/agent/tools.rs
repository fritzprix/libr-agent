use crate::agent::state::AgentSession;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use crate::services::WorkspaceService;
use base64::engine::general_purpose;
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use url::Url;

use crate::mcp::builtin::service_id::{BUILTIN_SERVICE_REGISTRY, CORE_BUILTIN_SERVICE_ALIASES};

pub const TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES: usize = 16 * 1024;
const TOOL_RESULT_SPILLOVER_DIR: &str = ".libragent/tool-results";
const TOOL_RESULT_MEDIA_DIR: &str = ".libragent/tool-results/media";
const MIN_TOOL_RESULT_INLINE_LIMIT_BYTES: usize = 4 * 1024;
const MAX_TOOL_RESULT_INLINE_LIMIT_BYTES: usize = 256 * 1024;

/// Resolve any alias string (including legacy pre-0.6.0 names) to the current
/// canonical service name.
///
/// Delegates to [`crate::mcp::builtin::service_id::BuiltinServiceId::from_alias`]
/// which is the single source of truth for all alias mappings.
pub fn canonicalize_builtin_service_alias(alias: &str) -> Option<&'static str> {
    crate::mcp::builtin::service_id::BuiltinServiceId::from_alias(alias).map(|id| id.name())
}

pub fn runtime_allowed_builtin_service_aliases(
    agent_config: &crate::agent::AgentConfig,
) -> Vec<String> {
    let mut allowed: HashSet<String> = CORE_BUILTIN_SERVICE_ALIASES
        .iter()
        .map(|alias| alias.to_string())
        .collect();

    if let Some(configured_ids) = &agent_config.allowed_built_in_service_aliases {
        for id in configured_ids {
            allowed.insert(id.name().to_string());
        }
    } else {
        // No explicit list → all optional services are implicitly enabled
        for entry in BUILTIN_SERVICE_REGISTRY.iter().filter(|e| e.optional) {
            allowed.insert(entry.canonical.to_string());
        }
    }

    // Preserve canonical ordering from the registry
    BUILTIN_SERVICE_REGISTRY
        .iter()
        .filter(|entry| allowed.contains(entry.canonical))
        .map(|entry| entry.canonical.to_string())
        .collect()
}

pub fn is_builtin_service_alias_enabled(
    agent_config: &crate::agent::AgentConfig,
    alias: &str,
) -> bool {
    let Some(target_alias) = canonicalize_builtin_service_alias(alias) else {
        return false;
    };

    runtime_allowed_builtin_service_aliases(agent_config)
        .iter()
        .any(|current| current == target_alias)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ToolResultAcceptance {
    Accept,
    Stale,
    Duplicate,
}

pub fn classify_tool_result(
    pending: &crate::agent::state::PendingToolExecution,
    tool_call_id: &str,
) -> ToolResultAcceptance {
    if !pending.expected_tool_call_ids.contains(tool_call_id) {
        return ToolResultAcceptance::Stale;
    }

    if pending.completed_tool_call_ids.contains(tool_call_id) {
        return ToolResultAcceptance::Duplicate;
    }

    ToolResultAcceptance::Accept
}

/// Collect available tools for a session based on agent configuration
pub async fn collect_available_tools(
    session_id: &str,
    agent_config: &crate::agent::AgentConfig,
    proxy_manager: &Arc<MCPServiceProxyManager>,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    let mut all_tools = Vec::new();

    // Get session proxy
    if let Some(proxy) = proxy_manager.get_proxy(session_id).await {
        // 1. Collect builtin tools (already filtered by extract_builtin_tool_ids during proxy creation)
        let builtin_tool_ids = proxy.builtin_tool_ids();

        log::debug!(
            "Session {} has {} builtin tool IDs configured",
            session_id,
            builtin_tool_ids.len()
        );

        // Get tools from each builtin server via the global MCP manager
        for tool_id in builtin_tool_ids {
            let server_tools = proxy.get_builtin_server_tools(&tool_id);
            log::debug!(
                "Builtin server '{}' provides {} tools",
                tool_id,
                server_tools.len()
            );
            all_tools.extend(server_tools);
        }

        log::info!(
            "Collected {} builtin tools for session {}",
            all_tools.len(),
            session_id
        );

        // 2. Collect external MCP tools
        if !agent_config.mcp_server_ids.is_empty() {
            log::debug!(
                "Agent config allows {} external MCP servers",
                agent_config.mcp_server_ids.len()
            );

            // 2a. Get SESSION-ISOLATED stdio server tools (spawned per-session)
            let session_stdio_tools = proxy.get_session_stdio_tools().await;

            log::info!(
                "Collected {} SESSION-ISOLATED stdio tools for session {}",
                session_stdio_tools.len(),
                session_id
            );

            all_tools.extend(session_stdio_tools);

            // 2b. Get SESSION-ISOLATED HTTP server tools (connected per-session)
            let session_http_tools = proxy.get_session_http_tools().await;

            log::info!(
                "Collected {} SESSION-ISOLATED HTTP tools for session {}",
                session_http_tools.len(),
                session_id
            );

            all_tools.extend(session_http_tools);
        }
    } else {
        log::warn!(
            "No proxy found for session {}, cannot collect tools",
            session_id
        );
    }

    log::info!(
        "Total tools available for session {}: {} tools",
        session_id,
        all_tools.len()
    );

    Ok(all_tools)
}

/// Extract builtin tool IDs from agent configuration
pub fn extract_builtin_tool_ids(agent_config: &crate::agent::AgentConfig) -> Vec<String> {
    runtime_allowed_builtin_service_aliases(agent_config)
}

/// Create a tool result message from successful tool execution
pub fn create_tool_result_message(
    session_id: &str,
    tool_call_id: &str,
    content: String,
    structured_content: Option<serde_json::Value>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text {
        text: content,
        is_error: None,
    }];

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content: content_array,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
        metadata: structured_content.map(|value| {
            serde_json::json!({
                "structuredContent": value,
            })
        }),
    }
}

/// Create an error tool result message from failed tool execution
pub fn create_error_tool_result(
    session_id: &str,
    tool_call_id: &str,
    error_message: &str,
    structured_content: Option<serde_json::Value>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text {
        text: format!("Error: {}", error_message),
        is_error: Some(true),
    }];

    let metadata = match structured_content {
        Some(value) => Some(serde_json::json!({
            "toolError": true,
            "structuredContent": value,
        })),
        None => Some(serde_json::json!({
            "toolError": true,
        })),
    };

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content: content_array,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
        metadata,
    }
}

fn build_tool_message_metadata(
    tool_error: bool,
    structured_content: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut metadata = serde_json::Map::new();

    if tool_error {
        metadata.insert("toolError".to_string(), serde_json::Value::Bool(true));
    }

    if let Some(value) = structured_content {
        metadata.insert("structuredContent".to_string(), value);
    }

    if metadata.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(metadata))
    }
}

/// Convert MCP response result to agent MCPContent
pub fn convert_mcp_response_content(
    result: Option<crate::mcp::types::MCPResponseResult>,
) -> Option<Vec<crate::mcp::types::MCPContent>> {
    match result {
        Some(crate::mcp::types::MCPResponseResult::ToolCall(tool_result)) => tool_result.content,
        _ => None,
    }
}

fn sanitize_spillover_identifier(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "tool-result".to_string()
    } else {
        trimmed.to_string()
    }
}

fn clamp_tool_result_inline_limit_bytes(limit_bytes: usize) -> usize {
    limit_bytes.clamp(
        MIN_TOOL_RESULT_INLINE_LIMIT_BYTES,
        MAX_TOOL_RESULT_INLINE_LIMIT_BYTES,
    )
}

pub fn tool_result_preview_headroom_bytes(limit_bytes: usize) -> usize {
    if limit_bytes <= 4 * 1024 {
        return 512.min(limit_bytes.saturating_sub(1));
    }

    (limit_bytes / 8).clamp(1024, 8 * 1024)
}

pub fn tool_result_preview_content_limit_bytes(limit_bytes: usize) -> usize {
    limit_bytes.saturating_sub(tool_result_preview_headroom_bytes(limit_bytes))
}

pub async fn tool_result_inline_limit_bytes() -> usize {
    let Some(settings_repo) = crate::state::try_get_settings_repository() else {
        return TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES;
    };

    match settings_repo.get("advancedSettings").await {
        Ok(Some(model)) => match serde_json::from_str::<Value>(&model.value) {
            Ok(json) => json
                .get("toolResultInlineLimitBytes")
                .and_then(|value| value.as_u64())
                .map(|value| clamp_tool_result_inline_limit_bytes(value as usize))
                .unwrap_or(TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES),
            Err(_) => TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        },
        _ => TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
    }
}

fn truncate_to_complete_lines(text: &str, limit: usize) -> (&str, bool) {
    if text.len() <= limit {
        return (text, false);
    }

    let mut boundary = limit.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }

    let truncated = &text[..boundary];
    match truncated.rfind('\n') {
        Some(pos) if pos > 0 => (&truncated[..pos], true),
        _ => (truncated, true),
    }
}

fn count_preview_lines(text: &str) -> usize {
    text.lines().count()
}

fn build_tool_result_spillover_notice(
    relative_path: &str,
    original_size_bytes: usize,
    preview_line_count: usize,
) -> String {
    let mut notice = format!(
        "\n\n... [output truncated: total size {} bytes] ...\n\nFull output saved to workspace file: `{}`\nRead it in chunks with `readFile({{\"path\": \"{}\", \"startLine\": 1, \"endLine\": 200}})`.\nDo not call `readFile({{\"path\": \"{}\"}})` on the saved file without `startLine` and `endLine`; that will just truncate again.",
        original_size_bytes, relative_path, relative_path, relative_path
    );

    if preview_line_count > 0 {
        let next_start_line = preview_line_count + 1;
        let next_end_line = next_start_line + 199;
        notice.push_str(&format!(
            "\nTo continue after the inline preview, call `readFile({{\"path\": \"{}\", \"startLine\": {}, \"endLine\": {}}})`.",
            relative_path, next_start_line, next_end_line
        ));
    } else {
        notice.push_str(&format!(
            "\nStart with a narrow range such as `readFile({{\"path\": \"{}\", \"startLine\": 1, \"endLine\": 50}})` and keep narrowing if needed.",
            relative_path
        ));
    }

    notice
}

pub async fn spill_oversized_tool_result_messages(
    session_id: &str,
    messages: Vec<Message>,
) -> Result<Vec<Message>, String> {
    let inline_limit_bytes = tool_result_inline_limit_bytes().await;
    let preview_limit_bytes = tool_result_preview_content_limit_bytes(inline_limit_bytes);
    let mut processed_messages = Vec::with_capacity(messages.len());

    for mut message in messages {
        if message.role != "tool" {
            processed_messages.push(message);
            continue;
        }

        let tool_call_id =
            sanitize_spillover_identifier(message.tool_call_id.as_deref().unwrap_or("tool-result"));
        let message_id = sanitize_spillover_identifier(&message.id);
        let mut next_content = Vec::with_capacity(message.content.len());
        for (content_index, content) in message.content.into_iter().enumerate() {
            match content {
                MCPContent::Text { text, is_error } if text.len() > inline_limit_bytes => {
                    let relative_path = format!(
                        "{}/{}-{}-{}.txt",
                        TOOL_RESULT_SPILLOVER_DIR,
                        tool_call_id,
                        message_id,
                        content_index + 1
                    );
                    WorkspaceService::workspace_write_file(
                        &relative_path,
                        text.as_bytes(),
                        Some(session_id.to_string()),
                    )
                    .await
                    .map_err(|error| {
                        format!(
                            "Failed to spill oversized tool output to '{}': {}",
                            relative_path, error
                        )
                    })?;

                    log::info!(
                        "Spilled oversized tool output for session {} to workspace file '{}' ({} bytes)",
                        session_id,
                        relative_path,
                        text.len()
                    );

                    let (preview, _) = truncate_to_complete_lines(&text, preview_limit_bytes);
                    let preview_line_count = count_preview_lines(preview);

                    next_content.push(MCPContent::Text {
                        text: format!(
                            "{}{}",
                            preview,
                            build_tool_result_spillover_notice(
                                &relative_path,
                                text.len(),
                                preview_line_count
                            )
                        ),
                        is_error,
                    });
                }
                other => next_content.push(other),
            }
        }

        message.content = next_content;
        processed_messages.push(message);
    }

    Ok(processed_messages)
}

fn media_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/ogg" => "ogg",
        "audio/aac" => "aac",
        "audio/flac" => "flac",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

fn media_relative_path_from_bytes(bytes: &[u8], mime_type: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = format!("{:x}", hasher.finalize());
    format!(
        "{}/{}.{}",
        TOOL_RESULT_MEDIA_DIR,
        digest,
        media_extension_for_mime(mime_type)
    )
}

async fn persist_tool_result_media(
    session_id: &str,
    bytes: &[u8],
    mime_type: &str,
) -> Result<String, String> {
    let relative_path = media_relative_path_from_bytes(bytes, mime_type);
    WorkspaceService::workspace_write_file(&relative_path, bytes, Some(session_id.to_string()))
        .await
        .map_err(|error| {
            format!(
                "Failed to persist tool-result media '{}' for session {}: {}",
                relative_path, session_id, error
            )
        })?;

    let session_manager = crate::session::get_session_manager()?;
    let absolute_path = session_manager
        .get_session_workspace_dir_by_id(session_id)
        .join(&relative_path);

    Url::from_file_path(&absolute_path)
        .map(|url| url.to_string())
        .map_err(|_| {
            format!(
                "Failed to convert persisted media path '{}' into file URL",
                absolute_path.display()
            )
        })
}

pub async fn externalize_media_content_for_storage(
    session_id: &str,
    content: Vec<MCPContent>,
) -> Result<Vec<MCPContent>, String> {
    let mut next_content = Vec::with_capacity(content.len());

    for item in content {
        match item {
            MCPContent::Image {
                data: Some(data),
                uri: _uri,
                mime_type,
            } => {
                let bytes = general_purpose::STANDARD.decode(&data).map_err(|error| {
                    format!(
                        "Failed to decode image payload for session {}: {}",
                        session_id, error
                    )
                })?;
                let file_url = persist_tool_result_media(session_id, &bytes, &mime_type).await?;
                next_content.push(MCPContent::Image {
                    data: None,
                    uri: Some(file_url),
                    mime_type,
                });
            }
            MCPContent::Audio {
                data: Some(data),
                uri: _uri,
                mime_type,
            } => {
                let bytes = general_purpose::STANDARD.decode(&data).map_err(|error| {
                    format!(
                        "Failed to decode audio payload for session {}: {}",
                        session_id, error
                    )
                })?;
                let file_url = persist_tool_result_media(session_id, &bytes, &mime_type).await?;
                next_content.push(MCPContent::Audio {
                    data: None,
                    uri: Some(file_url),
                    mime_type,
                });
            }
            other => next_content.push(other),
        }
    }

    Ok(next_content)
}

/// Create a tool result message from strict MCP content
pub fn create_tool_result_message_with_content(
    session_id: &str,
    tool_call_id: &str,
    content: Vec<MCPContent>,
    structured_content: Option<serde_json::Value>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();

    // Some servers may return error semantics via MCPContent (e.g., Text { is_error: Some(true) })
    // without flipping the outer ToolExecutionResult.is_error flag.
    // We propagate that signal into Message.metadata.toolError so the UI can group failed tool
    // results deterministically without parsing text.
    let tool_error = content.iter().any(|c| {
        matches!(
            c,
            MCPContent::Text {
                is_error: Some(true),
                ..
            }
        )
    });

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
        metadata: build_tool_message_metadata(tool_error, structured_content),
    }
}

/// Handle tool execution result from frontend or internal execution
///
/// Returns `Ok(Some(messages))` if all pending tools for this turn have completed,
/// containing the accumulated tool results to be processed.
/// Returns `Ok(None)` if we are still waiting for other tools to complete.
pub async fn handle_tool_result(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    tool_call_id: String,
    result: crate::commands::agent_commands::ToolExecutionResult,
) -> Result<Option<Vec<Message>>, String> {
    let crate::commands::agent_commands::ToolExecutionResult {
        success,
        content,
        mcp_content,
        structured_content,
        error,
        is_error,
    } = result;
    let mcp_content = match mcp_content {
        Some(content) => Some(externalize_media_content_for_storage(&session_id, content).await?),
        None => None,
    };

    log::debug!(
        "Tool result received for session {}, tool_call_id: {}",
        session_id,
        tool_call_id
    );

    // Scope to hold the write lock
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            if let Some(pending) = &mut session.pending_execution {
                match classify_tool_result(pending, &tool_call_id) {
                    ToolResultAcceptance::Stale => {
                        log::warn!(
                            "Ignoring stale tool result for session {}: tool_call_id {} does not belong to message {}",
                            session_id,
                            tool_call_id,
                            pending.message_id
                        );
                        return Ok(None);
                    }
                    ToolResultAcceptance::Duplicate => {
                        log::warn!(
                            "Ignoring duplicate tool result for session {}: tool_call_id {} already handled for message {}",
                            session_id,
                            tool_call_id,
                            pending.message_id
                        );
                        return Ok(None);
                    }
                    ToolResultAcceptance::Accept => {}
                }

                // Create Tool Message using helper methods
                let message = if is_error {
                    if let Some(mcp_content) = mcp_content {
                        // Prefer structured content (guided_error) over bare error string —
                        // the content array carries the full diagnosis the agent needs.
                        create_tool_result_message_with_content(
                            &session_id,
                            &tool_call_id,
                            mcp_content,
                            structured_content.clone(),
                        )
                    } else {
                        create_error_tool_result(
                            &session_id,
                            &tool_call_id,
                            error
                                .as_deref()
                                .unwrap_or("Tool execution failed without error message"),
                            structured_content.clone(),
                        )
                    }
                } else if let Some(mcp_content) = mcp_content {
                    // ✅ ALWAYS use structured content for successful tool calls
                    create_tool_result_message_with_content(
                        &session_id,
                        &tool_call_id,
                        mcp_content,
                        structured_content.clone(),
                    )
                } else {
                    // ⚠️ This branch should never happen for successful tool calls
                    log::warn!(
                        "Tool result has no mcp_content for session {}, tool_call_id {}. Using stringified fallback.",
                        session_id,
                        tool_call_id
                    );
                    create_tool_result_message(
                        &session_id,
                        &tool_call_id,
                        content.clone(),
                        structured_content.clone(),
                    )
                };

                pending.results.push(message);
                pending.completed_tool_call_ids.insert(tool_call_id.clone());

                // Emit ToolExecutionCompleted event for external tools (progress tracking)
                if let Some(tool_name) = pending.tool_names.get(&tool_call_id) {
                    let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                        session_id: session_id.clone(),
                        tool_name: tool_name.clone(),
                        success: !is_error && success,
                    };
                    let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event);
                }

                log::debug!(
                    "Accumulated result {}/{} for session {}",
                    pending.completed_tool_call_ids.len(),
                    pending.total_expected,
                    session_id
                );

                // Check if all results are in
                if pending.completed_tool_call_ids.len() >= pending.total_expected {
                    // Move results out of pending state
                    let accumulated_messages: Vec<Message> = pending.results.drain(..).collect();
                    // Clear pending state
                    session.pending_execution = None;

                    // Return the accumulated messages
                    return Ok(Some(accumulated_messages));
                }
            } else {
                log::warn!(
                    "Received tool result for session {} but no pending execution state found",
                    session_id
                );
                return Ok(None); // Ignore or error? Safe to ignore to prevent crashes
            }
        } else {
            return Err(format!("Session not found: {}", session_id));
        }
    }

    // If we're here, it means we haven't finished collecting all results yet
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::MCPContent;

    /// Regression: builtin tools that return is_error=true WITH mcp_content
    /// (e.g. guided_error from editFiles) must surface that content to the
    /// agent — NOT collapse it to the bare "Unknown error" fallback.
    ///
    /// Root cause was that handle_tool_result only checked `result.is_error` and
    /// always called create_error_tool_result, discarding mcp_content entirely.
    #[test]
    fn test_error_with_mcp_content_preserves_guided_error_text() {
        let guided_text =
            "STALE ANCHOR on line 28 — retry with anchor: 'ab12cd'\n  → refresh anchors and retry NOW";
        let content = vec![MCPContent::Text {
            text: guided_text.to_string(),
            is_error: Some(true),
        }];

        // This is the branch now taken when is_error=true AND mcp_content is Some
        let msg = create_tool_result_message_with_content("sess1", "tc1", content, None);

        // The agent must see the full guided error, not a bare "Unknown error"
        assert!(
            msg.content.iter().any(|c| matches!(c,
                MCPContent::Text { text, .. } if text.contains("STALE HASH")
            )),
            "guided_error text must be preserved in the tool message"
        );
        // toolError metadata must still be set so the UI marks it as failed
        assert_eq!(
            msg.metadata
                .as_ref()
                .and_then(|m| m.get("toolError"))
                .and_then(|v| v.as_bool()),
            Some(true),
            "toolError metadata must be set when content carries is_error:true"
        );
    }

    /// When is_error=true but mcp_content is None (e.g. JSON-RPC protocol error
    /// or arg-parse failure), the fallback path must still produce a message.
    /// "Unknown error" is acceptable here because there is literally no content.
    #[test]
    fn test_error_without_mcp_content_falls_back_to_error_string() {
        // Explicit error message
        let msg = create_error_tool_result("sess1", "tc1", "Failed to parse args: EOF", None);
        assert!(
            msg.content.iter().any(|c| matches!(c,
                MCPContent::Text { text, .. } if text.contains("Failed to parse args")
            )),
            "explicit error string must appear in the message"
        );

        // No error message → "Unknown error" fallback
        let fallback = create_error_tool_result("sess1", "tc1", "Unknown error", None);
        assert!(
            fallback.content.iter().any(|c| matches!(c,
                MCPContent::Text { text, .. } if text.contains("Unknown error")
            )),
            "Unknown error fallback must appear when no message is available"
        );
    }
}
