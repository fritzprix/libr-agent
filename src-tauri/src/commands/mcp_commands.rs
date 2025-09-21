#[tauri::command]
pub async fn get_service_context(server_id: String) -> Result<String, String> {
    crate::get_mcp_manager()
        .get_service_context(&server_id)
        .await
}
