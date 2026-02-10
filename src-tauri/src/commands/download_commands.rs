/// File download commands
///
/// This module contains commands for downloading files and creating ZIP archives
/// from the workspace.
use crate::session::get_session_manager;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use tauri_plugin_dialog::DialogExt;
use walkdir::WalkDir;
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
    session_id: String,
    file_path: String,
) -> Result<String, String> {
    // Get workspace directory via SessionManager
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(&session_id);

    // Resolve and validate path securely
    let full_path = crate::utils::security::resolve_secure_path(&workspace_dir, &file_path)
        .await
        .map_err(|e| format!("Access denied or file not found: {}", e))?;

    // Extract filename
    let file_name = full_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");

    // Read file content
    let max_size = crate::config::max_file_size() as u64;
    let file_content = crate::utils::fs::read_file_with_limit(&full_path, max_size)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

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
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
    // Canonicalize base for stripping prefixes later
    let workspace_dir_canon = tokio::fs::canonicalize(&workspace_dir)
        .await
        .map_err(|e| format!("Failed to canonicalize workspace: {}", e))?;

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
    let mut added_archive_paths = HashSet::<String>::new();
    for file_path in &files {
        // Resolve path securely
        let source_path =
            match crate::utils::security::resolve_secure_path(&workspace_dir, file_path).await {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("Skipping invalid path {}: {}", file_path, e);
                    continue;
                }
            };

        let roots: Vec<PathBuf> = if source_path.is_file() {
            vec![source_path]
        } else if source_path.is_dir() {
            WalkDir::new(&source_path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .map(|e| e.into_path())
                .collect()
        } else {
            continue;
        };

        for abs_path in roots {
            let abs_canon = match std::fs::canonicalize(&abs_path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if !abs_canon.starts_with(&workspace_dir_canon) {
                continue;
            }

            let rel_path = match abs_canon.strip_prefix(&workspace_dir_canon) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let archive_path = rel_path.to_string_lossy().replace("\\", "/");
            if !added_archive_paths.insert(archive_path.clone()) {
                continue;
            }

            if zip.start_file(&archive_path, options).is_err() {
                continue;
            }

            let max_size = crate::config::max_file_size() as u64;
            match crate::utils::fs::read_file_with_limit(&abs_canon, max_size).await {
                Ok(content) => {
                    if zip.write_all(&content).is_err() {
                        continue;
                    }
                    processed_files.push(archive_path);
                }
                Err(e) => {
                    log::error!("Failed to read file {}: {e}", abs_canon.display());
                    continue;
                }
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
    let max_size = crate::config::max_file_size() as u64;
    let zip_content = crate::utils::fs::read_file_with_limit(&temp_zip_path, max_size)
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
