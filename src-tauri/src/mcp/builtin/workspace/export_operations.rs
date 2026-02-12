use serde_json::Value;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use tracing::error;
use walkdir::WalkDir;
use zip::write::FileOptions;

use super::{ui_resources, WorkspaceServer};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPResult;

impl WorkspaceServer {
    pub async fn handle_export_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter existence validation
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };
        let display_name = args
            .get("displayName")
            .or_else(|| args.get("display_name"))
            .and_then(|v| v.as_str())
            .unwrap_or(path)
            .to_string();

        // Layer 3: Business logic - file existence validation
        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());
        let source_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&target_session_id)
            .join(path);
        if !source_path.exists() || !source_path.is_file() {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "File not found or is not a regular file".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use listDirectory to verify the file exists".to_string(),
                "Ensure the path points to a file, not a directory".to_string(),
                "Check the file path is correct".to_string(),
            ])
            .to_mcp_result());
        }

        let exports_dir = match self.ensure_exports_directory(&target_session_id) {
            Ok(dir) => dir,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Create exports directory failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check workspace directory permissions".to_string(),
                    "Ensure sufficient disk space".to_string(),
                    "Verify workspace path is accessible".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
        };

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
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
                ErrorCategory::InvalidState,
                "Export file failed".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Check source file permissions".to_string(),
                "Ensure sufficient disk space in exports directory".to_string(),
                "Verify the file is not locked by another process".to_string(),
                format!("Error: {}", e),
            ])
            .to_mcp_result());
        }

        let relative_path = format!("exports/files/{export_filename}");
        let source_path_str = path;

        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        let html_content = ui_resources::create_html_export_ui(
            &format!("File Export: {display_name}"),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            &display_name,
        );

        let ui_resource = ui_resources::create_export_ui_resource(
            ui_request_id,
            &format!("File Export: {display_name}"),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            html_content,
        );

        Ok(crate::mcp::builtin::utils::create_resource_response(
            ui_resource["uri"].as_str().unwrap(),
            "text/html",
            ui_resource["text"].as_str().unwrap(),
            "workspace",
            "exportFile",
            Some(&format!(
                "✓ File '{}' exported successfully\n\nDownload link available below\n\n💡 Next: Use exportZip to export multiple files at once",
                display_name
            )),
        ))
    }

    pub async fn handle_export_zip(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter existence validation
        let files_array = match args.get("files").and_then(|v| v.as_array()) {
            Some(files) => files,
            None => {
                return Ok(missing_param_error("files", ToolGroup::Workspace));
            }
        };
        let package_name = args
            .get("packageName")
            .or_else(|| args.get("package_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("workspace_export")
            .to_string();

        // Layer 2: Value constraints validation
        if files_array.is_empty() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Files array cannot be empty",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Include at least one file path in the files array".to_string(),
                "Use listDirectory to find files to export".to_string(),
                "Example: {\"files\": [\"file1.txt\", \"folder/file2.txt\"]}".to_string(),
            ])
            .to_mcp_result());
        }

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        let workspace_dir = self
            .session_manager
            .get_session_workspace_dir_by_id(&target_session_id);

        let workspace_dir_canon =
            std::fs::canonicalize(&workspace_dir).unwrap_or(workspace_dir.clone());

        // Validate all files exist before proceeding
        let mut missing_files = Vec::new();
        for file_value in files_array {
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
                    "The following {} file(s) were not found: {}",
                    missing_files.len(),
                    missing_files.join(", ")
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use listDirectory to verify file paths".to_string(),
                "Check for typos in filenames".to_string(),
                "Ensure files are within the workspace".to_string(),
            ])
            .to_mcp_result());
        }

        let exports_dir = match self.ensure_exports_directory(&target_session_id) {
            Ok(dir) => dir,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Create exports directory failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check workspace directory permissions".to_string(),
                    "Ensure sufficient disk space".to_string(),
                    "Verify workspace path is accessible".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
        };

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let zip_filename = format!("{package_name}_{timestamp}.zip");
        let zip_path = exports_dir.join("packages").join(&zip_filename);

        let zip_file = match std::fs::File::create(&zip_path) {
            Ok(file) => file,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Create ZIP file failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check exports directory permissions".to_string(),
                    "Ensure sufficient disk space".to_string(),
                    "Verify the path is accessible".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
        };

        let mut zip = zip::ZipWriter::new(zip_file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let mut processed_files = Vec::new();
        let mut added_archive_paths = HashSet::<String>::new();
        for file_value in files_array {
            let file_path = match file_value.as_str() {
                Some(path) => path,
                None => continue,
            };

            let source_path = workspace_dir_canon.join(file_path);
            if !source_path.exists() {
                continue;
            }

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

                match std::fs::read(&abs_canon) {
                    Ok(content) => {
                        if zip.write_all(&content).is_err() {
                            continue;
                        }
                        processed_files.push(archive_path);
                    }
                    Err(e) => {
                        error!("Failed to read file {}: {}", abs_canon.display(), e);
                        continue;
                    }
                }
            }
        }

        if let Err(e) = zip.finish() {
            return Ok(guided_error(
                ErrorCategory::InvalidState,
                "Finalize ZIP file failed".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Check if the ZIP writer encountered an error".to_string(),
                "Verify disk space is sufficient".to_string(),
                "Try exporting fewer files".to_string(),
                format!("Error: {}", e),
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
                "Verify the file paths are correct with listDirectory".to_string(),
                "Check that the files exist and are readable".to_string(),
                "If you provided a directory, ensure it contains readable files".to_string(),
            ])
            .to_mcp_result());
        }

        let relative_path = format!("exports/packages/{zip_filename}");

        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        let html_content = ui_resources::create_html_export_ui(
            &format!("ZIP Package: {package_name}"),
            &processed_files,
            "ZIP Package",
            &relative_path,
            &zip_filename,
        );

        let ui_resource = ui_resources::create_export_ui_resource(
            ui_request_id,
            &format!("ZIP Package: {package_name}"),
            &processed_files,
            "ZIP Package",
            &relative_path,
            html_content,
        );

        Ok(crate::mcp::builtin::utils::create_resource_response(
            ui_resource["uri"].as_str().unwrap(),
            "text/html",
            ui_resource["text"].as_str().unwrap(),
            "workspace",
            "exportZip",
            Some(&format!(
                "✓ ZIP package '{}' created successfully\n\nContains {} files\nDownload link available below\n\n💡 Next: Use exportFile to export individual files",
                package_name,
                processed_files.len()
            )),
        ))
    }

    fn ensure_exports_directory(&self, session_id: &str) -> Result<std::path::PathBuf, String> {
        let exports_dir = self.get_workspace_dir(session_id).join("exports");

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
