use crate::session_export::{
    export_session_file as export_session_file_inner, SessionExportFormat,
};

/// Export one session's persisted history as Markdown or ATIF-v1.7 JSON.
///
/// Reads the current SQLite snapshot (including busy sessions) and opens the
/// native Save File dialog. Compaction and recovery scaffolding is omitted.
#[tauri::command]
pub async fn export_session_file(
    app_handle: tauri::AppHandle,
    session_id: String,
    format: SessionExportFormat,
) -> Result<String, String> {
    export_session_file_inner(app_handle, session_id, format).await
}
