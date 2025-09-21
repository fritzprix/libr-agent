/// Gets the service context for a given MCP server.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server.
///
/// # Returns
/// A `Result` containing the service context string on success, or an error string on failure.
#[tauri::command]
pub async fn get_service_context(server_id: String) -> Result<String, String> {
    crate::get_mcp_manager()
        .get_service_context(&server_id)
        .await
}
