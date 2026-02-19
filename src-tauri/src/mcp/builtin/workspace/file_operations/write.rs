use super::super::WorkspaceServer;
use super::utils::{detect_language, format_file_size};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, permission_denied_error, ErrorCategory, SuccessHint,
    ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tracing::error;

impl WorkspaceServer {
    pub async fn handle_write_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Proactive Parameter Validation

        // 1. Path parameter existence and non-empty check
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
                    "Example: {\"path\": \"src/main.rs\"}".to_string(),
                    "Use listDirectory('.') to explore available paths".to_string(),
                ])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // 2. Path traversal validation
        if path_str.contains("..") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use relative paths from workspace root".to_string(),
                "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
                "Use listDirectory to explore available paths".to_string(),
            ])
            .to_mcp_result());
        }

        // 3. Content parameter validation
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(content) => content,
            None => {
                return Ok(missing_param_error("content", ToolGroup::Workspace));
            }
        };

        // 4. Overwrite parameter (default: false)
        let overwrite = args
            .get("overwrite")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate path security
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
                    "Use listDirectory to see available paths".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Check if file already exists
        let file_exists = safe_path.exists();
        let mut old_content = String::new();

        if file_exists {
            if !overwrite {
                // Return error if file exists and overwrite is false
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "File '{}' already exists and overwrite is set to false",
                        path_str
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "✅ To overwrite: Set \"overwrite\": true in your request".to_string(),
                    format!(
                        "   → first: readFile(\"{}\") into memory (if needed)",
                        path_str
                    ),
                    "   → then: writeFile(path, content, overwrite=true)".to_string(),
                    "".to_string(),
                    "⚠️ ALTERNATIVE: Use editFile for targeted edits (safer)".to_string(),
                    format!("   → editFile(\"{}\", oldText, newText)", path_str),
                ])
                .to_mcp_result());
            } else {
                // File exists and overwrite is true - read old content for diff
                match tokio::fs::read_to_string(&safe_path).await {
                    Ok(c) => old_content = c,
                    Err(e) => {
                        let error_msg = if e.kind() == std::io::ErrorKind::InvalidData {
                            "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
                        } else {
                            e.to_string()
                        };

                        return Ok(guided_error(
                            ErrorCategory::OperationFailed,
                            &error_msg,
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "File exists but could not be read".to_string(),
                            "Check file permissions".to_string(),
                        ])
                        .to_mcp_result());
                    }
                }
            }
        }

        let file_manager = self.get_file_manager(session_id.clone());
        let result = file_manager.write_file_string(path_str, content).await;

        match result {
            Ok(()) => {
                // Invalidate service context cache
                self.invalidate_context_cache().await;

                let lines = content.lines().count();
                let size_str = format_file_size(content.len() as u64);
                let language = detect_language(std::path::Path::new(path_str));

                let message_header = if file_exists {
                    "**✅ File Overwritten Successfully**"
                } else {
                    "**✅ New File Created Successfully**"
                };

                let mut message = format!(
                    "{}\n\n**File:** `{}`\n**Size:** {}\n**Lines:** {}\n\n",
                    message_header, path_str, size_str, lines
                );

                if file_exists {
                    // Show diff for overwritten files
                    use super::utils::format_file_diff;
                    let diff_output = format_file_diff(&old_content, content, path_str);
                    message.push_str(&diff_output);
                } else {
                    // New file - show content preview (truncated if large)
                    let max_display_lines = 100;
                    let max_display_bytes = 51200; // 50KB
                    let content_lines: Vec<&str> = content.lines().collect();
                    let is_truncated = content_lines.len() > max_display_lines
                        || content.len() > max_display_bytes;

                    let display_content = if is_truncated {
                        let truncated_lines: Vec<&str> = if content.len() > max_display_bytes {
                            // Truncate by bytes first
                            let truncated = &content[..max_display_bytes.min(content.len())];
                            truncated.lines().take(max_display_lines).collect()
                        } else {
                            content_lines
                                .iter()
                                .take(max_display_lines)
                                .copied()
                                .collect()
                        };
                        format!(
                            "{}\n\n... ⚠️ TRUNCATED: Showing first {} of {} lines ({}% shown)",
                            truncated_lines.join("\n"),
                            truncated_lines.len(),
                            content_lines.len(),
                            (truncated_lines.len() * 100) / content_lines.len()
                        )
                    } else {
                        content.to_string()
                    };

                    message.push_str(&format!(
                        "**Content:**\n```{}\n{}\n```\n",
                        language, display_content
                    ));

                    if is_truncated {
                        message.push_str(
                            "\n⚠️ **CONTENT TRUNCATED**: Only showing first 100 lines as preview\n",
                        );
                    }
                }

                // Context-aware next steps
                let mut next_steps = Vec::new();

                if file_exists {
                    next_steps.push("Verify changes with readFile if unsure".to_string());
                } else {
                    next_steps.push("Use readFile to see full content (if truncated)".to_string());
                }

                // File type specific suggestions
                if path_str.ends_with(".md")
                    || path_str.ends_with(".txt")
                    || path_str.ends_with(".rs")
                    || path_str.ends_with(".js")
                    || path_str.ends_with(".ts")
                {
                    next_steps.push(format!(
                        "Use editFile(\"{}\", oldText, newText) for targeted edits",
                        path_str
                    ));
                }

                next_steps.push(format!(
                    "Use deleteFile(\"{}\") to remove if needed",
                    path_str
                ));

                let hint = SuccessHint::new(message, next_steps);

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "path": path_str,
                    "bytes_written": content.len(),
                    "lines": lines,
                    "overwritten": file_exists
                }))))
            }
            Err(e) => {
                error!("Failed to write file {}: {}", path_str, e);
                let is_permission = e.to_string().contains("Permission denied")
                    || e.to_string().contains("permission");
                if is_permission {
                    Ok(permission_denied_error(path_str, ToolGroup::Workspace))
                } else {
                    Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        e.to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Check that the directory exists with listDirectory".to_string(),
                        "Verify you have write permissions".to_string(),
                        "Ensure the path is valid and within allowed directories".to_string(),
                    ])
                    .to_mcp_result())
                }
            }
        }
    }
}
