/// File download commands
///
/// This module contains commands for downloading files and creating ZIP archives
/// from the workspace.
use crate::commands::markdown_pdf::build_markdown_pdf;
use crate::commands::workspace_commands::resolve_workspace_scoped_file_path;
use crate::services::FileExportService;
use crate::session::get_session_manager;
use base64::{engine::general_purpose, Engine as _};
use std::path::Path;
use tauri_plugin_dialog::DialogExt;

fn infer_extension_from_mime_type(mime_type: &str) -> &str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        _ => "bin",
    }
}

async fn save_bytes_via_dialog(
    app_handle: tauri::AppHandle,
    file_name: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();

    app_handle
        .dialog()
        .file()
        .set_file_name(&file_name)
        .save_file(move |file_path_opt| {
            let save_result = if let Some(save_path) = file_path_opt {
                match save_path.into_path() {
                    Ok(path_buf) => match std::fs::write(&path_buf, &bytes) {
                        Ok(_) => {
                            log::info!("File downloaded successfully to: {path_buf:?}");
                            Ok(path_buf.to_string_lossy().into_owned())
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

    // Save dialogs are user-paced; allow several minutes before timing out.
    match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("Internal communication error".to_string()),
        Err(_) => Err("Dialog timeout - please try again".to_string()),
    }
}

/// Downloads a single file from the current session's workspace.
#[tauri::command]
pub async fn download_workspace_file(
    app_handle: tauri::AppHandle,
    session_id: String,
    file_path: String,
) -> Result<String, String> {
    let exported_file = FileExportService::read_file_content(&session_id, &file_path).await?;
    save_bytes_via_dialog(app_handle, exported_file.filename, exported_file.content).await
}

/// Exports a selection of workspace files as a single ZIP archive and prompts for download.
#[tauri::command]
pub async fn export_and_download_zip(
    app_handle: tauri::AppHandle,
    session_id: String,
    files: Vec<String>,
    package_name: String,
) -> Result<String, String> {
    let exported_file =
        FileExportService::create_zip_export(&session_id, files, &package_name).await?;
    save_bytes_via_dialog(app_handle, exported_file.filename, exported_file.content).await
}

#[tauri::command]
pub async fn download_media_file(
    app_handle: tauri::AppHandle,
    session_id: Option<String>,
    file_name: Option<String>,
    mime_type: String,
    data_base64: Option<String>,
    file_url: Option<String>,
) -> Result<String, String> {
    let bytes = match (data_base64, file_url) {
        (Some(base64), None) => general_purpose::STANDARD
            .decode(base64)
            .map_err(|e| format!("Invalid media base64 payload: {e}"))?,
        (None, Some(url)) => {
            let parsed =
                url::Url::parse(&url).map_err(|e| format!("Invalid file URL format: {e}"))?;

            if parsed.scheme() != "file" {
                return Err(format!(
                    "download_media_file only supports file:// URLs, got: {}",
                    parsed.scheme()
                ));
            }

            let session_id = session_id.ok_or_else(|| {
                "sessionId is required when downloading media from file:// URLs".to_string()
            })?;
            let file_path = parsed
                .to_file_path()
                .map_err(|_| "URL cannot be converted to a local file path".to_string())?;
            let session_manager = get_session_manager()?;
            let workspace_dir =
                crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;
            let scoped_path =
                resolve_workspace_scoped_file_path(&file_path, &workspace_dir).await?;
            tokio::fs::read(&scoped_path).await.map_err(|e| {
                format!(
                    "Failed to read local media '{}': {e}",
                    scoped_path.display()
                )
            })?
        }
        (Some(_), Some(_)) => {
            return Err("Provide either dataBase64 or fileUrl, not both".to_string())
        }
        (None, None) => return Err("Either dataBase64 or fileUrl is required".to_string()),
    };

    let resolved_file_name = file_name.unwrap_or_else(|| {
        format!(
            "image-{}.{}",
            chrono::Utc::now().timestamp_millis(),
            infer_extension_from_mime_type(&mime_type)
        )
    });

    save_bytes_via_dialog(app_handle, resolved_file_name, bytes).await
}

/// Saves arbitrary binary bytes via the native Save File dialog.
#[tauri::command]
pub async fn download_binary_file(
    app_handle: tauri::AppHandle,
    file_name: String,
    data_base64: String,
) -> Result<String, String> {
    if file_name.trim().is_empty() {
        return Err("fileName is required".to_string());
    }

    let bytes = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|e| format!("Invalid base64 payload: {e}"))?;

    save_bytes_via_dialog(app_handle, file_name, bytes).await
}

/// Saves UTF-8 text content via the native Save File dialog.
#[tauri::command]
pub async fn download_text_file(
    app_handle: tauri::AppHandle,
    file_name: String,
    content: String,
) -> Result<String, String> {
    if file_name.trim().is_empty() {
        return Err("fileName is required".to_string());
    }

    save_bytes_via_dialog(app_handle, file_name, content.into_bytes()).await
}

/// Renders Markdown to PDF via `markdown2pdf` (github theme) and saves via dialog.
#[tauri::command]
pub async fn download_text_pdf(
    app_handle: tauri::AppHandle,
    file_name: String,
    content: String,
    title: Option<String>,
) -> Result<String, String> {
    if file_name.trim().is_empty() {
        return Err("fileName is required".to_string());
    }

    // Title is unused — export is body-only Markdown (no role/document chrome).
    let _ = title;

    let bytes = tokio::task::spawn_blocking(move || build_markdown_pdf(&content))
        .await
        .map_err(|e| format!("PDF generation task failed: {e}"))??;

    let resolved_name = if Path::new(&file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
    {
        file_name
    } else {
        format!("{file_name}.pdf")
    };

    save_bytes_via_dialog(app_handle, resolved_name, bytes).await
}
