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
                let tool_count = tools.len();

                // Update database cache
                use crate::repositories::mcp_server_repository::MCPServerRepository;
                let repo = crate::state::get_mcp_server_repository();

                // Lookup server ID by name first
                match repo.get_by_name(&server_name).await {
                    Ok(Some(server)) => {
                        if let Err(e) = repo.update_tool_count(&server.id, tool_count as i32).await
                        {
                            log::warn!(
                                "Failed to cache tool count for {} server '{}' (ID: {}): {}",
                                server_type,
                                server_name,
                                server.id,
                                e
                            );
                        } else {
                            log::debug!(
                                "Cached {} tools for {} server '{}' (ID: {})",
                                tool_count,
                                server_type,
                                server_name,
                                server.id
                            );
                        }
                    }
                    Ok(None) => {
                        log::warn!(
                            "Cannot cache tool count for {} server '{}': server not found in database",
                            server_type,
                            server_name
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to lookup {} server '{}' for tool count caching: {}",
                            server_type,
                            server_name,
                            e
                        );
                    }
                }
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
