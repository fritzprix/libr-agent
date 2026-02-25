/// File download commands
///
/// This module contains commands for downloading files and creating ZIP archives
/// from the workspace.
use crate::services::FileExportService;
use tauri_plugin_dialog::DialogExt;

/// Downloads a single file from the current session's workspace.
///
/// This command reads a specified file from the workspace, then opens a native
/// "Save File" dialog for the user to choose a download location.
///
/// # Arguments
/// * `app_handle` - The Tauri application handle.
/// * `file_path` - The relative path of the file within the workspace to download.
#[tauri::command]
pub async fn download_workspace_file(
    app_handle: tauri::AppHandle,
    session_id: String,
    file_path: String,
) -> Result<String, String> {
    // Delegate to Service
    let exported_file = FileExportService::read_file_content(&session_id, &file_path).await?;

    // Show save file dialog and save (using a callback)
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let file_content = exported_file.content;

    app_handle
        .dialog()
        .file()
        .set_file_name(&exported_file.filename)
        .save_file(move |file_path_opt| {
            let save_result = if let Some(save_path) = file_path_opt {
                match save_path.into_path() {
                    Ok(path_buf) => match std::fs::write(&path_buf, &file_content) {
                        Ok(_) => {
                            log::info!("File downloaded successfully to: {path_buf:?}");
                            Ok("File downloaded successfully".to_string())
                        }
                        Err(e) => Err(format!("Failed to save file: {e}")),
                    },
                    Err(e) => Err(format!("Failed to convert file path: {e}")),
                }
            } else {
                Ok("Download cancelled by user".to_string())
            };

            let _ = tx.send(save_result);
        });

    // Wait for the callback to complete with a reasonable timeout
    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("Internal communication error".to_string()),
        Err(_) => Err("Dialog timeout - please try again".to_string()),
    }
}

/// Exports a selection of workspace files as a single ZIP archive and prompts for download.
///
/// This command creates a temporary ZIP file, adds the specified workspace files to it
/// while preserving their directory structure, and then uses a "Save File" dialog to
/// allow the user to download the archive.
///
/// # Arguments
/// * `app_handle` - The Tauri application handle.
/// * `session_id` - The ID of the session to export from.
/// * `files` - A vector of relative file paths within the workspace to include in the ZIP.
/// * `package_name` - A base name to use for the generated ZIP file.
#[tauri::command]
pub async fn export_and_download_zip(
    app_handle: tauri::AppHandle,
    session_id: String,
    files: Vec<String>,
    package_name: String,
) -> Result<String, String> {
    // Delegate to Service
    let exported_file =
        FileExportService::create_zip_export(&session_id, files, &package_name).await?;

    // Show save file dialog and save (using a callback)
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let processed_files_count = exported_file.file_count.unwrap_or(0);
    let file_content = exported_file.content;

    app_handle
        .dialog()
        .file()
        .set_file_name(&exported_file.filename)
        .save_file(move |file_path_opt| {
            let save_result = if let Some(save_path) = file_path_opt {
                match save_path.into_path() {
                    Ok(path_buf) => match std::fs::write(&path_buf, &file_content) {
                        Ok(_) => {
                            log::info!("ZIP file downloaded successfully to: {path_buf:?}");
                            Ok(format!(
                                "ZIP file with {processed_files_count} files downloaded successfully"
                            ))
                        }
                        Err(e) => Err(format!("Failed to save ZIP file: {e}")),
                    },
                    Err(e) => Err(format!("Failed to convert file path: {e}")),
                }
            } else {
                Ok("Download cancelled by user".to_string())
            };

            let _ = tx.send(save_result);
        });

    // Wait for the callback to complete with a reasonable timeout
    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("Internal communication error".to_string()),
        Err(_) => Err("Dialog timeout - please try again".to_string()),
    }
}
