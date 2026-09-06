use crate::mcp::MCPServiceProxyManager;
use std::sync::Arc;

/// Collect available tools for a session from its configured proxy.
pub async fn collect_available_tools(
    session_id: &str,
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

        // 2. Collect external MCP tools from the session-isolated proxy state.
        let session_stdio_tools = proxy.get_session_stdio_tools().await;

        log::info!(
            "Collected {} SESSION-ISOLATED stdio tools for session {}",
            session_stdio_tools.len(),
            session_id
        );

        all_tools.extend(session_stdio_tools);

        let session_http_tools = proxy.get_session_http_tools().await;

        log::info!(
            "Collected {} SESSION-ISOLATED HTTP tools for session {}",
            session_http_tools.len(),
            session_id
        );

        all_tools.extend(session_http_tools);
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
