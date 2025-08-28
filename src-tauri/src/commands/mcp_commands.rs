#[tauri::command]
pub async fn get_service_context(server_id: String) -> Result<String, String> {
    // This is a placeholder implementation.
    // In the future, this could query the actual MCP server process.
    Ok(format!(
        "# MCP Server Context\nServer ID: {server_id}\nStatus: Active"
    ))
}