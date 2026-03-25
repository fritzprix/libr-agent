use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

impl WorkspaceServer {
    pub async fn handle_list_directory(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let file_manager = self.get_file_manager(session_id.clone());
        let safe_path = match file_manager
            .get_security_validator()
            .validate_path_for_read(path_str)
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the directory path is correct".to_string(),
                    "Use listDirectory to see available files".to_string(),
                    "Ensure you have read permissions for the directory".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // If the validated path is actually a file, return a clear InvalidInput-style error
        if safe_path.is_file() {
            info!(
                "listDirectory called with file path instead of directory: {:?}",
                safe_path
            );
            return Ok(
                guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "The path '{}' points to a file, not a directory. Use readFile to read file contents.",
                        path_str
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a directory path when using listDirectory".to_string(),
                    "Use readFile to read the contents of a single file".to_string(),
                    "Use listDirectory on the parent directory to see available files and subdirectories".to_string(),
                ])
                .to_mcp_result(),
            );
        }

        match fs::read_dir(&safe_path).await {
            Ok(mut entries) => {
                let mut items = Vec::new();

                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        let file_type = if metadata.is_dir() {
                            "directory"
                        } else if metadata.is_file() {
                            "file"
                        } else {
                            "other"
                        };

                        let name = entry.file_name().to_string_lossy().to_string();
                        let size = if metadata.is_file() {
                            Some(metadata.len())
                        } else {
                            None
                        };

                        items.push(json!({
                            "name": name,
                            "type": file_type,
                            "size": size
                        }));
                    }
                }

                items.sort_by(|a, b| {
                    let a_type = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let b_type = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");

                    match (a_type, b_type) {
                        ("directory", "file") => std::cmp::Ordering::Less,
                        ("file", "directory") => std::cmp::Ordering::Greater,
                        _ => a_name.cmp(b_name),
                    }
                });

                // ✅ ENHANCED: Format listing with emojis, types, and sizes for AI visibility
                const MAX_ITEMS_DISPLAY: usize = 100;

                let item_lines: Vec<String> = items
                    .iter()
                    .take(MAX_ITEMS_DISPLAY)
                    .map(|item| {
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                        let size = item.get("size").and_then(|v| v.as_u64());

                        // Use emoji icons for visual clarity
                        let icon = match type_ {
                            "directory" => "📁",
                            "file" => "📄",
                            "symlink" => "🔗",
                            _ => "❓",
                        };

                        // Format size in human-readable way
                        let size_str = if let Some(s) = size {
                            if s < 1024 {
                                format!(" ({}B)", s)
                            } else if s < 1024 * 1024 {
                                format!(" ({:.1}KB)", s as f64 / 1024.0)
                            } else {
                                format!(" ({:.1}MB)", s as f64 / 1024.0 / 1024.0)
                            }
                        } else {
                            "".to_string()
                        };

                        format!("{} [{}] {}{}", icon, type_, name, size_str)
                    })
                    .collect();

                // Add truncation note if needed
                let truncation_note = if items.len() > MAX_ITEMS_DISPLAY {
                    format!("\n\n... and {} more items", items.len() - MAX_ITEMS_DISPLAY)
                } else {
                    String::new()
                };

                info!(
                    "Successfully listed directory: {:?} ({} items)",
                    safe_path,
                    items.len()
                );

                // ✅ ENHANCED: Clear messaging for empty directories
                let hint = if items.is_empty() {
                    SuccessHint::new(
                        format!(
                            "Directory listing for '{}':\n\n(This directory is empty)\n\n💡 Next Steps:\n- Use writeFile('{}/filename.txt', content) to create a file\n- Use listDirectory('{}') to verify the directory exists\n- This is a valid empty directory",
                            path_str, path_str, path_str
                        ),
                        vec![],
                    )
                } else {
                    let listing_str = item_lines.join("\n");
                    SuccessHint::new(
                        format!(
                            "Directory listing for '{}':\n\n{}{}\n\n💡 Next Steps:\n- Use readFile('{}/filename') to read a file\n- Use listDirectory('{}/subdir') to explore subdirectories\n- Use search to search for content in files",
                            path_str, listing_str, truncation_note, path_str, path_str
                        ),
                        vec![],
                    )
                };

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "items": items,
                    "path": path_str,
                    "count": items.len()
                }))))
            }
            Err(e) => {
                error!("Failed to list directory {:?}: {}", safe_path, e);
                let is_not_found =
                    e.to_string().contains("No such file") || e.to_string().contains("not found");
                if is_not_found {
                    Ok(not_found_error("Directory", path_str, ToolGroup::Workspace))
                } else {
                    Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        e.to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the directory exists".to_string(),
                        "Check directory permissions".to_string(),
                        "Try using '.' to list the current directory".to_string(),
                    ])
                    .to_mcp_result())
                }
            }
        }
    }
}
