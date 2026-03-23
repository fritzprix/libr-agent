use crate::repositories::mcp_server_repository::MCPServerRepository;

async fn persist_tool_cache(
    repo: &dyn MCPServerRepository,
    server_name: &str,
    server_id: Option<&str>,
    server_type: &str,
    tools: &[super::super::types::MCPTool],
) {
    let tool_count = tools.len();
    let tools_json = crate::mcp::utils::serialize_mcp_tools(tools);

    let resolved_server_id = match server_id {
        Some(id) => Some(id.to_string()),
        None => match repo.get_by_name(server_name).await {
            Ok(Some(server)) => Some(server.id),
            Ok(None) => {
                log::warn!(
                    "Cannot refresh tool cache for {} server '{}': server not found in database",
                    server_type,
                    server_name
                );
                None
            }
            Err(e) => {
                log::warn!(
                    "Failed to lookup {} server '{}' for tool cache refresh: {}",
                    server_type,
                    server_name,
                    e
                );
                None
            }
        },
    };

    let Some(server_id) = resolved_server_id else {
        return;
    };

    if let Err(e) = repo
        .update_cached_tools(&server_id, tool_count as i32, tools_json)
        .await
    {
        log::warn!(
            "Failed to refresh tool cache for {} server '{}' (ID: {}): {}",
            server_type,
            server_name,
            server_id,
            e
        );
    } else {
        log::debug!(
            "Refreshed {} cached tools for {} server '{}' (ID: {})",
            tool_count,
            server_type,
            server_name,
            server_id
        );
    }
}

pub async fn persist_tool_cache_for_server(
    server_name: &str,
    server_id: Option<&str>,
    server_type: &str,
    tools: &[super::super::types::MCPTool],
) {
    let repo = crate::state::get_mcp_server_repository();
    persist_tool_cache(repo, server_name, server_id, server_type, tools).await;
}

pub fn spawn_tool_cache_update<F, Fut>(
    server_name: String,
    session_id: String,
    server_type: &'static str,
    fetch_tools: F,
) where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<Vec<super::super::types::MCPTool>, String>> + Send,
{
    tokio::spawn(async move {
        log::debug!(
            "Fetching tools to update cache for {} server '{}' (session: {})",
            server_type,
            server_name,
            session_id
        );

        match fetch_tools().await {
            Ok(tools) => {
                persist_tool_cache_for_server(&server_name, None, server_type, &tools).await;
            }
            Err(e) => {
                log::error!(
                    "Failed to fetch tools for cache update from {} server '{}': {}",
                    server_type,
                    server_name,
                    e
                );
            }
        }
    });
}
