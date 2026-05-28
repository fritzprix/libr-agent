use serde_json::Value;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use tracing::error;
use walkdir::WalkDir;
use zip::write::FileOptions;

use super::{ui_resources, utils::is_internal_workspace_artifact_path, WorkspaceServer};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPResult;

struct ExportUiResponse<'a> {
    title: &'a str,
    items: &'a [String],
    type_label: &'a str,
    relative_path: &'a str,
    filename: &'a str,
    tool_name: &'a str,
    text_response: &'a str,
}

impl WorkspaceServer {
    pub async fn handle_export(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter validation
        let paths_value = args.get("paths");
        let paths_array = match paths_value {
            None => {
                return Ok(missing_param_error("paths", ToolGroup::Workspace));
            }
            Some(v) => match v.as_array() {
                Some(paths) if !paths.is_empty() => paths,
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        "Invalid 'paths' parameter",
                        ToolGroup::Workspace,
                    )
                    .guidance(vec!["The 'paths' argument must be provided as a non-empty array of workspace-relative paths to export.".to_string()])
                    .to_mcp_result());
                }
            },
        };

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        let workspace_dir = self
            .session_manager
            .get_session_workspace_dir_by_id(&target_session_id);

        let workspace_dir_canon =
            std::fs::canonicalize(&workspace_dir).unwrap_or(workspace_dir.clone());

        // Layer 2: Determine mode (Single File vs ZIP)
        // If there's exactly 1 path and it's a regular file -> Single File Export
        // Otherwise (multiple paths, or single path is a directory) -> ZIP Export
        let mut is_single_file_mode = false;
        let mut single_file_path: Option<PathBuf> = None;
        let mut single_file_rel_path_str = String::new();

        if paths_array.len() == 1 {
            if let Some(path_str) = paths_array[0].as_str() {
                let check_path = workspace_dir_canon.join(path_str);
                // Canonicalize the candidate path and ensure it stays within the workspace
                if let Ok(canon_check) = std::fs::canonicalize(&check_path) {
                    if canon_check.starts_with(&workspace_dir_canon) && canon_check.is_file() {
                        is_single_file_mode = true;
                        single_file_path = Some(canon_check);
                        single_file_rel_path_str = path_str.to_string();
                    }
                }
            }
        }

        let name_param = args
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let exports_dir = match self.ensure_exports_directory(&target_session_id) {
            Ok(dir) => dir,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InternalError,
                    "Create exports directory failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the session workspace exists and is writable".to_string(),
                    "Ensure the exports directory can be created under the session workspace"
                        .to_string(),
                    format!("Underlying error: {}", e),
                ])
                .to_mcp_result());
            }
        };

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");

        if is_single_file_mode {
            // === SINGLE FILE EXPORT ===
            let source_path = single_file_path.unwrap();
            if is_internal_workspace_artifact_path(&workspace_dir_canon, &source_path) {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Internal LibrAgent temp/export artifacts cannot be exported".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Select workspace files or directories outside .libragent/tmp and .libragent/exports".to_string(),
                    "Use readProcessOutput or listProcesses instead of exporting raw temp outputs".to_string(),
                ])
                .to_mcp_result());
            }
            let display_name = name_param.unwrap_or_else(|| single_file_rel_path_str.clone());

            let file_stem = source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let file_ext = source_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let export_filename = if file_ext.is_empty() {
                format!("{file_stem}_{timestamp}")
            } else {
                format!("{file_stem}_{timestamp}.{file_ext}")
            };

            let export_path = exports_dir.join("files").join(&export_filename);
            if let Err(e) = std::fs::copy(&source_path, &export_path) {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    "Export file failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![format!("Error: {}", e)])
                .to_mcp_result());
            }

            let relative_path = PathBuf::from(".libragent")
                .join("exports")
                .join("files")
                .join(&export_filename);
            let response_items = vec![single_file_rel_path_str];
            let response_title = format!("File Export: {display_name}");
            let response_relative_path = relative_path.to_string_lossy().replace('\\', "/");
            let response_text = format!(
                "✓ File '{}' exported successfully\n\nSaved export: `{}`\nDownload link available below",
                display_name, response_relative_path
            );

            return Ok(self.build_ui_response(&ExportUiResponse {
                title: &response_title,
                items: &response_items,
                type_label: "Single File",
                relative_path: &response_relative_path,
                filename: &display_name,
                tool_name: "export",
                text_response: &response_text,
            }));
        }

        // === ZIP PACKAGE EXPORT ===
        let package_name = name_param.unwrap_or_else(|| "workspace_export".to_string());
        let safe_package_name = crate::services::FileExportService::sanitize_package_name(&package_name);
        let zip_filename = format!("{safe_package_name}_{timestamp}.zip");
        let zip_path = exports_dir.join("packages").join(&zip_filename);

        let mut missing_files = Vec::new();
        for file_value in paths_array {
            if let Some(path_str) = file_value.as_str() {
                let file_path = workspace_dir_canon.join(path_str);
                if !file_path.exists() {
                    missing_files.push(path_str.to_string());
                }
            }
        }

        if !missing_files.is_empty() {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!(
                    "The following {} file(s)/folder(s) were not found: {}",
                    missing_files.len(),
                    missing_files.join(", ")
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use listDirectory('.') to verify workspace-relative paths".to_string(),
                "Use search with filePattern to find the exact file or directory name".to_string(),
                "Export paths must be relative to the workspace root".to_string(),
            ])
            .to_mcp_result());
        }

        let zip_file = match std::fs::File::create(&zip_path) {
            Ok(file) => file,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    "Create ZIP file failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the .libragent/exports/packages directory is writable".to_string(),
                    "Ensure sufficient disk space is available".to_string(),
                    format!("Underlying error: {}", e),
                ])
                .to_mcp_result())
            }
        };

        let mut zip = zip::ZipWriter::new(zip_file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let mut processed_files = Vec::new();
        let mut added_archive_paths = HashSet::<String>::new();

        for file_value in paths_array {
            if let Some(path_str) = file_value.as_str() {
                let source_path = workspace_dir_canon.join(path_str);
                if !source_path.exists() {
                    continue;
                }

                let roots: Vec<PathBuf> = if source_path.is_file() {
                    vec![source_path]
                } else if source_path.is_dir() {
                    WalkDir::new(&source_path)
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|e| {
                            !is_internal_workspace_artifact_path(&workspace_dir_canon, e.path())
                        })
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
                    if is_internal_workspace_artifact_path(&workspace_dir_canon, &abs_canon) {
                        continue;
                    }
                    let rel_path = match abs_canon.strip_prefix(&workspace_dir_canon) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let archive_path = {
                        let p = rel_path.to_string_lossy().to_string();
                        #[cfg(target_os = "windows")]
                        let p = p.replace('\\', "/");
                        p
                    };

                    if !added_archive_paths.insert(archive_path.clone()) {
                        continue;
                    }
                    if zip.start_file(&archive_path, options).is_err() {
                        continue;
                    }

                    match std::fs::read(&abs_canon) {
                        Ok(content) => {
                            if zip.write_all(&content).is_ok() {
                                processed_files.push(archive_path);
                            }
                        }
                        Err(e) => error!("Failed to read file {}: {}", abs_canon.display(), e),
                    }
                }
            }
        }

        if let Err(e) = zip.finish() {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                "Finalize ZIP file failed".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Retry the export after verifying the selected files are readable".to_string(),
                "Ensure the target export path has enough free space".to_string(),
                format!("Underlying error: {}", e),
            ])
            .to_mcp_result());
        }

        if processed_files.is_empty() {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                "No files were successfully added to ZIP".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use listDirectory or search to verify the selected paths contain readable files"
                    .to_string(),
                "Directories are allowed, but only readable files inside them are added"
                    .to_string(),
                "Retry with a smaller, known-good set of workspace-relative paths".to_string(),
            ])
            .to_mcp_result());
        }

        let relative_path = PathBuf::from(".libragent")
            .join("exports")
            .join("packages")
            .join(&zip_filename);
        let response_title = format!("ZIP Package: {package_name}");
        let response_relative_path = relative_path.to_string_lossy().replace('\\', "/");
        let response_text = format!(
            "✓ ZIP package '{}' created successfully\n\nContains {} files\nSaved export: `{}`\nDownload link available below",
            package_name,
            processed_files.len(),
            response_relative_path
        );

        Ok(self.build_ui_response(&ExportUiResponse {
            title: &response_title,
            items: &processed_files,
            type_label: "ZIP Package",
            relative_path: &response_relative_path,
            filename: &zip_filename,
            tool_name: "export",
            text_response: &response_text,
        }))
    }

    fn build_ui_response(&self, response: &ExportUiResponse<'_>) -> MCPResult {
        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        let html_content = ui_resources::create_html_export_ui(
            response.title,
            response.items,
            response.type_label,
            response.relative_path,
            response.filename,
        );

        let ui_resource = ui_resources::create_export_ui_resource(
            ui_request_id,
            response.title,
            response.items,
            response.type_label,
            response.relative_path,
            html_content,
        );

        let resource_uri = match ui_resource.get("uri").and_then(|value| value.as_str()) {
            Some(uri) if !uri.is_empty() => uri,
            _ => {
                return guided_error(
                    ErrorCategory::InternalError,
                    "Export UI resource was created without a valid URI".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Retry the export request".to_string(),
                    "If this keeps happening, inspect the export UI resource builder".to_string(),
                ])
                .to_mcp_result();
            }
        };
        let resource_text = match ui_resource.get("text").and_then(|value| value.as_str()) {
            Some(text) => text,
            None => {
                return guided_error(
                    ErrorCategory::InternalError,
                    "Export UI resource was created without HTML content".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Retry the export request".to_string(),
                    "If this keeps happening, inspect the export UI resource builder".to_string(),
                ])
                .to_mcp_result();
            }
        };
        let full_text = format!(
            "{}\nUI resource: `{}`\nUse the UI resource to trigger the download workflow.",
            response.text_response, resource_uri
        );

        crate::mcp::builtin::utils::create_resource_response(
            resource_uri,
            "text/html",
            resource_text,
            "workspace",
            response.tool_name,
            Some(&full_text),
        )
    }

    fn ensure_exports_directory(&self, session_id: &str) -> Result<std::path::PathBuf, String> {
        let exports_dir = self
            .get_workspace_dir(session_id)
            .join(".libragent/exports");

        let files_dir = exports_dir.join("files");
        let packages_dir = exports_dir.join("packages");

        for dir in [&exports_dir, &files_dir, &packages_dir] {
            if !dir.exists() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("Failed to create directory {dir:?}: {e}"))?;
            }
        }

        Ok(exports_dir)
    }
}
