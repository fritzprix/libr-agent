/// File download commands
///
/// This module contains commands for downloading files and creating ZIP archives
/// from the workspace.
use crate::session::get_session_manager;
use std::io::Write;
use tauri_plugin_dialog::DialogExt;
use zip::{write::FileOptions, ZipWriter};

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
    file_path: String,
) -> Result<String, String> {
    // Get workspace directory via SessionManager
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager.get_session_workspace_dir();

    // Construct the full path of the requested file
    let full_path = workspace_dir.join(&file_path);

    // Verify file existence and security
    if !full_path.exists() {
        return Err(format!("File not found: {file_path}"));
    }

    if !full_path.starts_with(&workspace_dir) {
        return Err("Access denied: Path outside workspace".to_string());
    }

    // Extract filename
    let file_name = full_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");

    // Read file content
    let file_content = match tokio::fs::read(&full_path).await {
        Ok(content) => content,
        Err(e) => return Err(format!("Failed to read file: {e}")),
    };

    // Show save file dialog and save (using a callback)
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();

    app_handle
        .dialog()
        .file()
        .set_file_name(file_name)
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
/// * `files` - A vector of relative file paths within the workspace to include in the ZIP.
/// * `package_name` - A base name to use for the generated ZIP file.
#[tauri::command]
pub async fn export_and_download_zip(
    app_handle: tauri::AppHandle,
    files: Vec<String>,
    package_name: String,
) -> Result<String, String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager.get_session_workspace_dir();

    if files.is_empty() {
        return Err("Files array cannot be empty".to_string());
    }

    // Create a temporary ZIP file
    let temp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let zip_filename = format!("{package_name}_{timestamp}.zip");
    let temp_zip_path = temp_dir.path().join(&zip_filename);

    // Create the ZIP archive
    let zip_file = std::fs::File::create(&temp_zip_path)
        .map_err(|e| format!("Failed to create ZIP file: {e}"))?;

    let mut zip = ZipWriter::new(zip_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Add files to the ZIP
    let mut processed_files = Vec::new();
    for file_path in &files {
        let source_path = workspace_dir.join(file_path);

        if !source_path.exists() || !source_path.is_file() {
            continue; // Skip non-existent files
        }

        // Set the path inside the ZIP (preserving directory structure)
        let archive_path = file_path.replace("\\", "/");

        match zip.start_file(&archive_path, options) {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to start file in ZIP: {e}");
                continue;
            }
        }

        match std::fs::read(&source_path) {
            Ok(content) => {
                if let Err(e) = zip.write_all(&content) {
                    log::error!("Failed to write file content to ZIP: {e}");
                    continue;
                }
                processed_files.push(file_path.clone());
            }
            Err(e) => {
                log::error!("Failed to read file {file_path}: {e}");
                continue;
            }
        }
    }

    // Finalize the ZIP file
    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {e}"))?;

    if processed_files.is_empty() {
        return Err("No files were successfully added to ZIP".to_string());
    }

    // Read ZIP content to be used in the callback
    let zip_content = tokio::fs::read(&temp_zip_path)
        .await
        .map_err(|e| format!("Failed to read ZIP file: {e}"))?;

    // Show save file dialog and save (using a callback)
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let processed_files_count = processed_files.len();

    app_handle
        .dialog()
        .file()
        .set_file_name(&zip_filename)
        .save_file(move |file_path_opt| {
            let save_result = if let Some(save_path) = file_path_opt {
                match save_path.into_path() {
                    Ok(path_buf) => match std::fs::write(&path_buf, &zip_content) {
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
