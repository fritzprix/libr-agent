
use serde_json::Value;
use std::io::Write;
use tracing::error;
use zip::write::FileOptions;

use super::{ui_resources, WorkspaceServer};
use crate::mcp::MCPResponse;

impl WorkspaceServer {
    pub async fn handle_export_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: path",
                );
            }
        };
        let display_name = args
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or(path)
            .to_string();

        let source_path = self.get_workspace_dir().join(path);
        if !source_path.exists() || !source_path.is_file() {
            return Self::error_response(
                request_id,
                -32603,
                "File not found or is not a regular file",
            );
        }

        let exports_dir = match self.ensure_exports_directory() {
            Ok(dir) => dir,
            Err(e) => return Self::error_response(request_id, -32603, &e),
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
            format!("{}_{}", file_stem, timestamp)
        } else {
            format!("{}_{}.{}", file_stem, timestamp, file_ext)
        };

        let export_path = exports_dir.join("files").join(&export_filename);
        if let Err(e) = std::fs::copy(&source_path, &export_path) {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to copy file: {}", e),
            );
        }

        let relative_path = format!("exports/files/{}", export_filename);
        let source_path_str = path;

        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        let html_content = ui_resources::create_html_export_ui(
            &format!("File Export: {}", display_name),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            &display_name,
        );

        let ui_resource = ui_resources::create_export_ui_resource(
            ui_request_id,
            &format!("File Export: {}", display_name),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            html_content,
        );

        let success_message = format!(
            "파일 '{}'이(가) 성공적으로 export되었습니다. 아래 링크에서 다운로드할 수 있습니다.",
            display_name
        );

        ui_resources::success_response_with_text_and_resource(
            request_id,
            &success_message,
            ui_resource,
        )
    }

    pub async fn handle_export_zip(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let files_array = match args.get("files").and_then(|v| v.as_array()) {
            Some(files) => files,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: files (array)",
                );
            }
        };
        let package_name = args
            .get("package_name")
            .and_then(|v| v.as_str())
            .unwrap_or("workspace_export")
            .to_string();

        if files_array.is_empty() {
            return Self::error_response(request_id, -32602, "Files array cannot be empty");
        }

        let exports_dir = match self.ensure_exports_directory() {
            Ok(dir) => dir,
            Err(e) => return Self::error_response(request_id, -32603, &e),
        };

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let zip_filename = format!("{}_{}.zip", package_name, timestamp);
        let zip_path = exports_dir.join("packages").join(&zip_filename);

        let zip_file = match std::fs::File::create(&zip_path) {
            Ok(file) => file,
            Err(e) => {
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create ZIP file: {}", e),
                )
            }
        };

        let mut zip = zip::ZipWriter::new(zip_file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let mut processed_files = Vec::new();
        for file_value in files_array {
            let file_path = match file_value.as_str() {
                Some(path) => path,
                None => continue,
            };

            let source_path = self.get_workspace_dir().join(file_path);
            if !source_path.exists() || !source_path.is_file() {
                continue;
            }

            let archive_path = file_path.replace("\\", "/");

            match zip.start_file(&archive_path, options) {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to start file in ZIP: {}", e);
                    continue;
                }
            }

            match std::fs::read(&source_path) {
                Ok(content) => {
                    if let Err(e) = zip.write_all(&content) {
                        error!("Failed to write file content to ZIP: {}", e);
                        continue;
                    }
                    processed_files.push(file_path.to_string());
                }
                Err(e) => {
                    error!("Failed to read file {}: {}", file_path, e);
                    continue;
                }
            }
        }

        if let Err(e) = zip.finish() {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to finalize ZIP: {}", e),
            );
        }

        if processed_files.is_empty() {
            return Self::error_response(
                request_id,
                -32603,
                "No files were successfully added to ZIP",
            );
        }

        let relative_path = format!("exports/packages/{}", zip_filename);

        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        let html_content = ui_resources::create_html_export_ui(
            &format!("ZIP Package: {}", package_name),
            &processed_files,
            "ZIP Package",
            &relative_path,
            &zip_filename,
        );

        let ui_resource = ui_resources::create_export_ui_resource(
            ui_request_id,
            &format!("ZIP Package: {}", package_name),
            &processed_files,
            "ZIP Package",
            &relative_path,
            html_content,
        );

        let success_message = format!(
            "ZIP 패키지 '{}'이(가) 성공적으로 생성되었습니다. {}개 파일이 포함되어 있으며, 아래 링크에서 다운로드할 수 있습니다.",
            package_name,
            processed_files.len()
        );

        ui_resources::success_response_with_text_and_resource(
            request_id,
            &success_message,
            ui_resource,
        )
    }

    fn ensure_exports_directory(&self) -> Result<std::path::PathBuf, String> {
        let exports_dir = self.get_workspace_dir().join("exports");

        let files_dir = exports_dir.join("files");
        let packages_dir = exports_dir.join("packages");

        for dir in [&exports_dir, &files_dir, &packages_dir] {
            if !dir.exists() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("Failed to create directory {:?}: {}", dir, e))?;
            }
        }

        Ok(exports_dir)
    }
}
