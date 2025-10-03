use crate::mcp::types::{ServiceContext, ServiceContextOptions};

/// Gets the service context for a given MCP server.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server.
///
/// # Returns
/// A `Result` containing the service context on success, or an error string on failure.
#[tauri::command]
pub async fn get_service_context(server_id: String) -> Result<ServiceContext, String> {
    crate::get_mcp_manager()
        .get_service_context(&server_id)
        .await
}

/// Switches the context for a given MCP server.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server.
/// * `options` - The context options to switch to.
///
/// # Returns
/// A `Result` indicating success or an error string on failure.
#[tauri::command]
#[allow(dead_code)]
pub async fn switch_context(
    server_id: String,
    options: ServiceContextOptions,
) -> Result<(), String> {
    crate::get_mcp_manager()
        .switch_context(&server_id, options)
        .await
}
