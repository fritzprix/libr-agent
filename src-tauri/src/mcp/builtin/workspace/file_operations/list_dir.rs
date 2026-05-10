use super::super::WorkspaceServer;
use super::utils::{is_not_found_io_error, normalize_workspace_path_input};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::builtin::workspace::utils::is_internal_workspace_artifact_path;
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
        let path_str =
            match normalize_workspace_path_input(args.get("path").and_then(|v| v.as_str()), ".") {
                Ok(path) => path,
                Err(message) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        message,
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Provide a directory path (relative paths resolve from the workspace)"
                            .to_string(),
                        "Examples: {\"path\": \"src\"} or {\"path\": \"/tmp\"}".to_string(),
                        "Use listDirectory('.') to inspect the workspace directory".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

        let requested_limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);
        let requested_offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
        let limit = usize::try_from(requested_limit)
            .unwrap_or(usize::MAX)
            .clamp(1, 500);
        let offset = usize::try_from(requested_offset)
            .unwrap_or(usize::MAX)
            .min(100_000);
        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());
        let workspace_root = self.get_workspace_dir(&target_session_id);

        let file_manager = self.get_file_manager(Some(target_session_id));
        let safe_path = match file_manager
            .get_security_validator()
            .validate_path_for_read(&path_str)
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
                    if is_internal_workspace_artifact_path(&workspace_root, &entry.path()) {
                        continue;
                    }
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
                let total_items = items.len();
                let paginated_items: Vec<_> = items.into_iter().skip(offset).take(limit).collect();
                let has_more = offset + paginated_items.len() < total_items;

                let mut table_lines = vec![
                    "| Type | Name | Size |".to_string(),
                    "|---|---|---|".to_string(),
                ];

                let item_lines: Vec<String> = paginated_items
                    .iter()
                    .map(|item| {
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                        let size = item.get("size").and_then(|v| v.as_u64());

                        // Use emoji icons for visual clarity
                        let icon = match type_ {
                            "directory" => "📁 dir",
                            "file" => "📄 file",
                            _ => "❓ other",
                        };

                        // Format size in human-readable way
                        let size_str = if let Some(s) = size {
                            if s < 1024 {
                                format!("{}B", s)
                            } else if s < 1024 * 1024 {
                                format!("{:.1}KB", s as f64 / 1024.0)
                            } else {
                                format!("{:.1}MB", s as f64 / 1024.0 / 1024.0)
                            }
                        } else {
                            "-".to_string()
                        };

                        format!("| {} | `{}` | {} |", icon, name, size_str)
                    })
                    .collect();
                table_lines.extend(item_lines);

                let listing_str = table_lines.join("\n");

                // Add truncation note if needed
                let truncation_note = if has_more {
                    format!(
                        "\n\n*(Showing {} to {} of {} items. Call listDirectory with offset: {} to see more)*",
                        offset + 1,
                        offset + paginated_items.len(),
                        total_items,
                        offset + limit
                    )
                } else if offset > 0 {
                    format!(
                        "\n\n*(Showing {} to {} of {} items)*",
                        offset + 1,
                        offset + paginated_items.len(),
                        total_items
                    )
                } else {
                    String::new()
                };

                info!(
                    "Successfully listed directory: {:?} ({} items, offset: {}, limit: {})",
                    safe_path, total_items, offset, limit
                );

                // ✅ ENHANCED: Clear messaging for empty directories
                let hint = if total_items == 0 {
                    SuccessHint::new(
                        format!(
                            "Directory listing for '{}':\n\n(This directory is empty)\n\nThis is a valid empty directory.",
                            path_str
                        ),
                        vec![
                            format!(
                                "Use writeFile with {{\"path\": \"{}/filename.txt\", \"content\": \"...\"}} to create a file",
                                path_str
                            )
                        ],
                    )
                } else {
                    SuccessHint::new(
                        format!(
                            "Directory listing for '{}':\n\n{}{}",
                            path_str, listing_str, truncation_note
                        ),
                        vec![
                            format!("Use readFile('{}/filename') to read a file", path_str),
                            format!(
                                "Use listDirectory('{}/subdir') to explore subdirectories",
                                path_str
                            ),
                            "Use search to search for content in files".to_string(),
                        ],
                    )
                };

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "items": paginated_items,
                    "path": path_str,
                    "count": paginated_items.len(),
                    "total_count": total_items,
                    "offset": offset,
                    "limit": limit
                }))))
            }
            Err(e) => {
                error!("Failed to list directory {:?}: {}", safe_path, e);
                if is_not_found_io_error(&e) {
                    Ok(guided_error(
                        ErrorCategory::ResourceNotFound,
                        format!("Directory '{}' not found", path_str),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Use listDirectory('.') to inspect the workspace root".to_string(),
                        "Verify the directory path is correct".to_string(),
                        "Check whether the directory exists and is readable".to_string(),
                    ])
                    .to_mcp_result())
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
