use crate::agent::state::AgentSession;
use crate::commands::messages_commands::Message;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

// ─── Single source of truth ──────────────────────────────────────────────────────────────────────────────
// To add, rename, or change a service: edit ONE row here.
// All other constants and functions below are derived automatically.

pub(crate) struct BuiltinServiceEntry {
    pub(crate) canonical: &'static str,
    /// Falls back to enabled when agent has no explicit alias list
    pub(crate) optional: bool,
}

pub(crate) const BUILTIN_SERVICE_REGISTRY: &[BuiltinServiceEntry] = &[
    BuiltinServiceEntry {
        canonical: "planning",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "workspace",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "knowledge",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "assistant",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "skills",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "playbook",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "content_store",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "swarm",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "ui",
        optional: false,
    },
    BuiltinServiceEntry {
        canonical: "browser",
        optional: true,
    },
    BuiltinServiceEntry {
        canonical: "bootstrap",
        optional: true,
    },
    BuiltinServiceEntry {
        canonical: "mcp_manager",
        optional: false,
    },
];

// ─── Derived helpers ────────────────────────────────────────────────────────────────────

pub const CORE_BUILTIN_SERVICE_ALIASES: [&str; 9] = [
    "planning",
    "workspace",
    "knowledge",
    "assistant",
    "skills",
    "playbook",
    "content_store",
    "swarm",
    "ui",
];

pub fn canonicalize_builtin_service_alias(alias: &str) -> Option<&'static str> {
    let normalized = alias.trim().to_lowercase();
    BUILTIN_SERVICE_REGISTRY
        .iter()
        .find(|entry| entry.canonical == normalized.as_str())
        .map(|entry| entry.canonical)
}

pub fn runtime_allowed_builtin_service_aliases(
    agent_config: &crate::agent::AgentConfig,
) -> Vec<String> {
    let mut allowed: HashSet<String> = CORE_BUILTIN_SERVICE_ALIASES
        .iter()
        .map(|alias| alias.to_string())
        .collect();

    if let Some(configured_aliases) = &agent_config.allowed_built_in_service_aliases {
        for alias in configured_aliases {
            match canonicalize_builtin_service_alias(alias) {
                Some(canonical) => {
                    allowed.insert(canonical.to_string());
                }
                None => {
                    log::warn!("Unknown builtin service alias: {}", alias);
                }
            }
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
enum ToolResultAcceptance {
    Accept,
    Stale,
    Duplicate,
}

fn classify_tool_result(
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
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
        metadata: None,
    }
}

/// Create an error tool result message from failed tool execution
pub fn create_error_tool_result(
    session_id: &str,
    tool_call_id: &str,
    error_message: &str,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text {
        text: format!("Error: {}", error_message),
        is_error: Some(true),
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
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
        metadata: Some(serde_json::json!({
            "toolError": true,
        })),
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

/// Create a tool result message from strict MCP content
pub fn create_tool_result_message_with_content(
    session_id: &str,
    tool_call_id: &str,
    content: Vec<MCPContent>,
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
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
        metadata: if tool_error {
            Some(serde_json::json!({
                "toolError": true,
            }))
        } else {
            None
        },
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
                let message = if result.is_error {
                    create_error_tool_result(
                        &session_id,
                        &tool_call_id,
                        result.error.as_deref().unwrap_or("Unknown error"),
                    )
                } else if let Some(mcp_content) = result.mcp_content {
                    // ✅ ALWAYS use structured content for successful tool calls
                    create_tool_result_message_with_content(&session_id, &tool_call_id, mcp_content)
                } else {
                    // ⚠️ This branch should never happen for successful tool calls
                    log::warn!(
                        "Tool result has no mcp_content for session {}, tool_call_id {}. Using stringified fallback.",
                        session_id,
                        tool_call_id
                    );
                    create_tool_result_message(&session_id, &tool_call_id, result.content.clone())
                };

                pending.results.push(message);
                pending.completed_tool_call_ids.insert(tool_call_id.clone());

                // Emit ToolExecutionCompleted event for external tools (progress tracking)
                if let Some(tool_name) = pending.tool_names.get(&tool_call_id) {
                    let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                        session_id: session_id.clone(),
                        tool_name: tool_name.clone(),
                        success: !result.is_error,
                    };
                    let _ = crate::agent::events::emit_agent_event(app_handle, event);
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
    use std::collections::HashMap;

    fn mock_agent_config(aliases: Option<Vec<&str>>) -> crate::agent::AgentConfig {
        crate::agent::AgentConfig {
            id: Some("assistant-test".to_string()),
            name: "Test Assistant".to_string(),
            description: None,
            system_prompt: "You are helpful".to_string(),
            mcp_server_ids: Vec::new(),
            local_services: Vec::new(),
            allowed_built_in_service_aliases: aliases
                .map(|values| values.into_iter().map(|value| value.to_string()).collect()),
            temperature: 1.0,
            max_tokens: None,
            max_depth: None,
            max_fanout: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
        }
    }

    fn mock_pending_execution(
        expected: &[&str],
        completed: &[&str],
    ) -> crate::agent::state::PendingToolExecution {
        crate::agent::state::PendingToolExecution {
            message_id: "msg-1".to_string(),
            total_expected: expected.len(),
            results: Vec::new(),
            tool_names: HashMap::new(),
            expected_tool_call_ids: expected.iter().map(|id| (*id).to_string()).collect(),
            completed_tool_call_ids: completed.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    #[test]
    fn test_classify_tool_result_accepts_expected_unseen_id() {
        let pending = mock_pending_execution(&["call-1", "call-2"], &["call-1"]);
        let result = classify_tool_result(&pending, "call-2");
        assert_eq!(result, ToolResultAcceptance::Accept);
    }

    #[test]
    fn test_classify_tool_result_rejects_stale_id() {
        let pending = mock_pending_execution(&["call-1", "call-2"], &["call-1"]);
        let result = classify_tool_result(&pending, "call-999");
        assert_eq!(result, ToolResultAcceptance::Stale);
    }

    #[test]
    fn test_classify_tool_result_rejects_duplicate_id() {
        let pending = mock_pending_execution(&["call-1", "call-2"], &["call-1"]);
        let result = classify_tool_result(&pending, "call-1");
        assert_eq!(result, ToolResultAcceptance::Duplicate);
    }

    #[test]
    fn test_tool_result_with_structured_content() {
        let session_id = "test-session";
        let tool_call_id = "call-123";
        let content = vec![MCPContent::Text {
            text: "Test result".to_string(),
            is_error: None,
        }];

        let message =
            create_tool_result_message_with_content(session_id, tool_call_id, content.clone());

        // Assert: No double wrapping
        assert_eq!(message.content.len(), 1);
        assert_eq!(message.role, "tool");
        assert_eq!(message.tool_call_id, Some(tool_call_id.to_string()));

        match &message.content[0] {
            MCPContent::Text { text, .. } => {
                assert_eq!(text, "Test result");
                // Should NOT contain JSON string with "content" field
                assert!(!text.contains("\"content\""));
                assert!(!text.starts_with("{"));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_tool_result_fallback_to_string() {
        let session_id = "test-session";
        let tool_call_id = "call-123";
        let content_str = "Plain text result";

        let message = create_tool_result_message(session_id, tool_call_id, content_str.to_string());

        // Assert: Single text wrapper
        assert_eq!(message.content.len(), 1);
        assert_eq!(message.role, "tool");
        assert_eq!(message.tool_call_id, Some(tool_call_id.to_string()));

        match &message.content[0] {
            MCPContent::Text { text, .. } => {
                assert_eq!(text, content_str);
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_error_tool_result() {
        let session_id = "test-session";
        let tool_call_id = "call-123";
        let error_msg = "Tool execution failed";

        let message = create_error_tool_result(session_id, tool_call_id, error_msg);

        assert_eq!(message.role, "tool");
        assert_eq!(message.tool_call_id, Some(tool_call_id.to_string()));
        assert_eq!(message.content.len(), 1);

        match &message.content[0] {
            MCPContent::Text { text, .. } => {
                assert!(text.contains(error_msg));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_multiple_content_items() {
        let session_id = "test-session";
        let tool_call_id = "call-123";
        let content = vec![
            MCPContent::Text {
                text: "First item".to_string(),
                is_error: None,
            },
            MCPContent::Text {
                text: "Second item".to_string(),
                is_error: None,
            },
        ];

        let message =
            create_tool_result_message_with_content(session_id, tool_call_id, content.clone());

        assert_eq!(message.content.len(), 2);

        match &message.content[0] {
            MCPContent::Text { text, .. } => {
                assert_eq!(text, "First item");
            }
            _ => panic!("Expected text content"),
        }

        match &message.content[1] {
            MCPContent::Text { text, .. } => {
                assert_eq!(text, "Second item");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn extract_builtin_tool_ids_normalizes_legacy_aliases() {
        let config = mock_agent_config(Some(vec!["session_api", "contentstore", "browser"]));
        let tool_ids = extract_builtin_tool_ids(&config);

        assert!(tool_ids.contains(&"swarm".to_string()));
        assert!(tool_ids.contains(&"content_store".to_string()));
        assert!(tool_ids.contains(&"browser".to_string()));
    }

    #[test]
    fn extract_builtin_tool_ids_always_includes_core_aliases() {
        let config = mock_agent_config(Some(vec!["browser"]));
        let tool_ids = extract_builtin_tool_ids(&config);

        for alias in CORE_BUILTIN_SERVICE_ALIASES {
            assert!(tool_ids.contains(&alias.to_string()));
        }
        assert!(tool_ids.contains(&"browser".to_string()));
    }

    // ─── Server name / registry regression tests ─────────────────────────────
    // Original bug: ContentStoreServer::name() returned "contentstore" while the
    // registry had "content_store". All four tests below would have caught it.

    /// Every concrete server NAME must be a recognised canonical in the registry.
    #[test]
    fn each_builtin_server_name_is_in_registry() {
        use crate::mcp::builtin;
        let all_names: &[&str] = &[
            builtin::planning::NAME,
            builtin::workspace::NAME,
            builtin::knowledge::NAME,
            builtin::assistant::NAME,
            builtin::skills::NAME,
            builtin::playbook::NAME,
            builtin::content_store::NAME,
            builtin::session_api::NAME,
            builtin::ui::NAME,
            builtin::browser::NAME,
            builtin::bootstrap::NAME,
            builtin::mcp_manager::NAME,
        ];
        for name in all_names {
            assert!(
                canonicalize_builtin_service_alias(name).is_some(),
                "server NAME {name:?} is not in BUILTIN_SERVICE_REGISTRY – \
                 fix the typo or add it to the registry",
            );
        }
    }

    /// No two servers may share the same canonical name.
    #[test]
    fn builtin_server_names_are_unique() {
        use crate::mcp::builtin;
        let all_names: &[&str] = &[
            builtin::planning::NAME,
            builtin::workspace::NAME,
            builtin::knowledge::NAME,
            builtin::assistant::NAME,
            builtin::skills::NAME,
            builtin::playbook::NAME,
            builtin::content_store::NAME,
            builtin::session_api::NAME,
            builtin::ui::NAME,
            builtin::browser::NAME,
            builtin::bootstrap::NAME,
            builtin::mcp_manager::NAME,
        ];
        let mut seen = std::collections::HashSet::new();
        for name in all_names {
            assert!(seen.insert(*name), "duplicate server NAME {name:?}");
        }
    }

    /// BUILTIN_SERVICE_REGISTRY must not contain duplicate canonical entries.
    #[test]
    fn registry_has_no_duplicate_canonicals() {
        let mut seen = std::collections::HashSet::new();
        for entry in BUILTIN_SERVICE_REGISTRY {
            assert!(
                seen.insert(entry.canonical),
                "duplicate canonical {:?} in BUILTIN_SERVICE_REGISTRY",
                entry.canonical,
            );
        }
    }

    /// Server list and registry must stay in sync.
    /// Catches: registry entry added but no server implements it (or vice-versa).
    #[test]
    fn registry_and_server_list_are_in_sync() {
        use crate::mcp::builtin;
        let server_names: std::collections::HashSet<&str> = [
            builtin::planning::NAME,
            builtin::workspace::NAME,
            builtin::knowledge::NAME,
            builtin::assistant::NAME,
            builtin::skills::NAME,
            builtin::playbook::NAME,
            builtin::content_store::NAME,
            builtin::session_api::NAME,
            builtin::ui::NAME,
            builtin::browser::NAME,
            builtin::bootstrap::NAME,
            builtin::mcp_manager::NAME,
        ]
        .iter()
        .copied()
        .collect();

        assert_eq!(
            server_names.len(),
            BUILTIN_SERVICE_REGISTRY.len(),
            "server list ({}) and registry ({}) diverged – update both together",
            server_names.len(),
            BUILTIN_SERVICE_REGISTRY.len(),
        );

        for entry in BUILTIN_SERVICE_REGISTRY {
            assert!(
                server_names.contains(entry.canonical),
                "registry canonical {:?} has no server NAME",
                entry.canonical,
            );
        }
    }
}
