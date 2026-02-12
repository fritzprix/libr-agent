use super::super::WorkspaceServer;
use super::utils::format_file_size;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

impl WorkspaceServer {
    pub async fn handle_delete_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter Validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a file path relative to workspace root".to_string(),
                    "Example: {\"path\": \"src/temp.txt\"}".to_string(),
                    "Use listDirectory('.') to see available files".to_string(),
                ])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // Path traversal validation
        if path_str.contains("..") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use relative paths from workspace root".to_string(),
                "Example: 'src/file.txt' instead of '../file.txt'".to_string(),
            ])
            .to_mcp_result());
        }

        // Layer 2: Path Security Validation
        let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file path is within workspace boundaries".to_string(),
                    "Use listDirectory to see available files".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Layer 3: File Existence Check
        if !safe_path.exists() {
            return Ok(not_found_error("File", path_str, ToolGroup::Workspace));
        }

        // Layer 4: Verify it's a file, not a directory
        if safe_path.is_dir() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("'{}' is a directory, not a file", path_str),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "deleteFile can only delete files, not directories".to_string(),
                "Use listDirectory to see directory contents".to_string(),
                format!(
                    "To delete directory contents, delete individual files inside '{}'",
                    path_str
                ),
            ])
            .to_mcp_result());
        }

        // Get file metadata before deletion (for confirmation message)
        let metadata = (fs::metadata(&safe_path).await).ok();

        let file_size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let size_str = format_file_size(file_size);

        // Layer 5: Perform Deletion
        match fs::remove_file(&safe_path).await {
            Ok(()) => {
                info!("Successfully deleted file: {}", path_str);

                // Invalidate service context cache
                self.invalidate_context_cache().await;

                let parent_dir = std::path::Path::new(path_str)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or(".");

                let output = format!(
                    "**✅ File Deleted Successfully**\n\n**File:** `{}`\n**Size:** {}\n\n⚠️ This operation is permanent - the file cannot be recovered through this tool.",
                    path_str,
                    size_str
                );

                let hint = SuccessHint::new(
                    output,
                    vec![
                        format!("Use listDirectory(\"{}\") to verify deletion", parent_dir),
                        format!(
                            "Use writeFile(\"{}\", content) to create a new file at this path",
                            path_str
                        ),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "path": path_str,
                    "size_bytes": file_size,
                    "deleted": true
                }))))
            }
            Err(e) => {
                error!("Failed to delete file {}: {}", path_str, e);

                let is_permission = e.to_string().contains("Permission denied")
                    || e.to_string().contains("permission")
                    || e.kind() == std::io::ErrorKind::PermissionDenied;

                if is_permission {
                    Ok(guided_error(
                        ErrorCategory::PermissionDenied,
                        "Permission denied: Cannot delete file",
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Check file permissions and ownership".to_string(),
                        "Ensure the file is not locked by another process".to_string(),
                        "On Windows, close any programs that might have the file open".to_string(),
                    ])
                    .to_mcp_result())
                } else {
                    Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        e.to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the file exists and is accessible".to_string(),
                        "Check if the file is locked by another process".to_string(),
                        format!("Use listDirectory to verify file path: {}", path_str),
                    ])
                    .to_mcp_result())
                }
            }
        }
    }
}
