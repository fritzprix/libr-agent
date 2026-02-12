use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::Value;
use tokio::fs;
use tracing::{error, info};

impl WorkspaceServer {
    pub async fn handle_import_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // ✅ ENHANCED: Replace legacy MCPResult::error() with guided_error for better context

        // Parameter validation 1: srcAbsPath
        let src_path_str = match args
            .get("srcAbsPath")
            .or_else(|| args.get("src_abs_path"))
            .and_then(|v| v.as_str())
        {
            Some(path) => path,
            None => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Missing required parameter: srcAbsPath",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide the absolute path to the file you want to import".to_string(),
                    "Example: {\"srcAbsPath\": \"/home/user/file.txt\", \"destRelPath\": \"imports/file.txt\"}".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Parameter validation 2: destRelPath
        let dest_rel_path = match args
            .get("destRelPath")
            .or_else(|| args.get("dest_rel_path"))
            .and_then(|v| v.as_str())
        {
            Some(path) => path,
            None => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Missing required parameter: destRelPath",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide the destination path relative to workspace root".to_string(),
                    "Example: \"imports/filename.ext\" or \"src/data/file.txt\"".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Log import attempt for debugging
        info!(
            "importFile called: src='{}', dest='{}'",
            src_path_str, dest_rel_path
        );

        // Validate source path exists and is readable
        let src_path = match std::path::Path::new(src_path_str).canonicalize() {
            Ok(path) => path,
            Err(e) => {
                error!(
                    "Failed to canonicalize source path '{}': {}",
                    src_path_str, e
                );
                return Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    format!("Source file not found or cannot be accessed: {}", src_path_str),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file path is correct and the file exists".to_string(),
                    "Check file permissions and ensure you have read access".to_string(),
                    format!("On Windows, use absolute paths like 'C:\\Users\\...', on Unix like '/home/user/...'"),
                    "Use an absolute path, not a relative path".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Ensure source is a file, not a directory
        if !src_path.is_file() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Source path is a directory, not a file: {}", src_path_str),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide the path to a specific file, not a directory".to_string(),
                "To import multiple files, call importFile multiple times".to_string(),
                "To import directory contents, use shell commands (e.g., runShell('cp -r src dest'))".to_string(),
            ])
            .to_mcp_result());
        }

        // Use file manager to handle destination path validation and copying
        let file_manager = self.get_file_manager(session_id);
        match file_manager
            .copy_file_from_external(&src_path, dest_rel_path)
            .await
        {
            Ok(dest_path) => {
                info!(
                    "Successfully imported file from {} to {}",
                    src_path.display(),
                    dest_path.display()
                );

                // Get file size for reporting
                let file_size = match fs::metadata(&dest_path).await {
                    Ok(metadata) => metadata.len(),
                    Err(_) => 0,
                };

                let hint = SuccessHint::new(
                    format!(
                        "✅ Successfully imported {} ({} bytes) to {}",
                        src_path.display(),
                        file_size,
                        dest_rel_path
                    ),
                    vec![
                        format!(
                            "Use readFile(\"{}\") to view imported content",
                            dest_rel_path
                        ),
                        "Use writeFile to modify the imported file".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result())
            }
            Err(e) => {
                error!(
                    "Failed to import file from {} to {}: {}",
                    src_path.display(),
                    dest_rel_path,
                    e
                );

                // Provide context-specific error guidance
                let (category, guidance) = if e.contains("already exists")
                    || e.contains("duplicate")
                {
                    (
                        ErrorCategory::InvalidInput,
                        vec![
                            format!("File already exists at: {}", dest_rel_path),
                            "Use writeFile to overwrite the existing file".to_string(),
                            "Or specify a different destination path with a unique name"
                                .to_string(),
                        ],
                    )
                } else if e.contains("permission") || e.contains("denied") {
                    (
                        ErrorCategory::PermissionDenied,
                        vec![
                            "Insufficient permissions to write to destination".to_string(),
                            "Check workspace permissions and destination directory access"
                                .to_string(),
                            "Ensure you have write access to the destination directory".to_string(),
                        ],
                    )
                } else if e.contains("space") {
                    (
                        ErrorCategory::InvalidInput,
                        vec![
                            "Insufficient disk space to import file".to_string(),
                            "Free up disk space and try again".to_string(),
                        ],
                    )
                } else {
                    (
                        ErrorCategory::InvalidInput,
                        vec![
                            "Verify source file is accessible and destination path is valid"
                                .to_string(),
                            "Check workspace configuration and file manager settings".to_string(),
                        ],
                    )
                };

                Ok(guided_error(
                    category,
                    format!("Failed to import file: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(guidance)
                .to_mcp_result())
            }
        }
    }
}
