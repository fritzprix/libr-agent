use super::super::WorkspaceServer;
use super::utils::format_file_size;
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, ErrorCategory, ErrorGuidance,
    ToolGroup,
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
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    vec![
                        "Provide a file path relative to workspace root".to_string(),
                        "Example: {\"path\": \"src/temp.txt\"}".to_string(),
                        "Use listDirectory('.') to see available files".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // Path traversal validation
        if path_str.contains("..") {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                vec![
                    "Use relative paths from workspace root".to_string(),
                    "Example: 'src/file.txt' instead of '../file.txt'".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Layer 2: Path Security Validation
        let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
            Ok(path) => path,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    vec![
                        "Verify the file path is within workspace boundaries".to_string(),
                        "Use listDirectory to see available files".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // Layer 3: File Existence Check
        if !safe_path.exists() {
            return Ok(not_found_error("File", path_str, ToolGroup::Workspace));
        }

        // Layer 4: Verify it's a file, not a directory
        if safe_path.is_dir() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("'{}' is a directory, not a file", path_str),
                vec![
                    "deleteFile can only delete files, not directories".to_string(),
                    "Use listDirectory to see directory contents".to_string(),
                    format!(
                        "To delete directory contents, delete individual files inside '{}'",
                        path_str
                    ),
                ],
                ToolGroup::Workspace,
            )
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

                let output = format!(
                    "**✅ File Deleted Successfully**\n\n\
                    **File:** `{}`\n\
                    **Size:** {}\n\n\
                    ⚠️ This operation is permanent - the file cannot be recovered through this tool.\n\n\
                    **Next Steps:**\n\
                    - 📋 Use `listDirectory(\"{}\")` to verify deletion\n\
                    - 📝 Use `createFile(\"{}\", content)` to create a new file at this path",
                    path_str,
                    size_str,
                    std::path::Path::new(path_str)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("."),
                    path_str
                );

                Ok(MCPResult::success_with_data(
                    &output,
                    json!({
                        "path": path_str,
                        "size_bytes": file_size,
                        "deleted": true
                    }),
                ))
            }
            Err(e) => {
                error!("Failed to delete file {}: {}", path_str, e);

                let is_permission = e.to_string().contains("Permission denied")
                    || e.to_string().contains("permission")
                    || e.kind() == std::io::ErrorKind::PermissionDenied;

                if is_permission {
                    Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::PermissionDenied,
                        "Permission denied: Cannot delete file",
                        vec![
                            "Check file permissions and ownership".to_string(),
                            "Ensure the file is not locked by another process".to_string(),
                            "On Windows, close any programs that might have the file open"
                                .to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result())
                } else {
                    Ok(operation_failed_error(
                        "Delete file",
                        &e.to_string(),
                        vec![
                            "Verify the file exists and is accessible".to_string(),
                            "Check if the file is locked by another process".to_string(),
                            format!("Use listDirectory to verify file path: {}", path_str),
                        ],
                        ToolGroup::Workspace,
                    ))
                }
            }
        }
    }
}
