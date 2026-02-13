use super::super::super::WorkspaceServer;
use super::super::utils::{calculate_similarity, format_string_diff, read_file_as_string};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tracing::info;

impl WorkspaceServer {
    pub async fn handle_edit_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter existence validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a valid file path: editFile({path, oldString, newString})".to_string(),
                    "Use listDirectory('.') to find files".to_string(),
                ])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // Get oldString parameter
        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "⚠️ CRITICAL: Call readFile FIRST to get exact content".to_string(),
                    "Extract text exactly as shown in readFile response".to_string(),
                    "Include surrounding context (3-5 lines) for uniqueness".to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("oldString", ToolGroup::Workspace)),
        };

        // Get newString parameter (can be empty for deletion)
        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace)),
        };

        // Validate: Reject identical strings early (before file I/O)
        if old_string == new_string {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "oldString and newString are identical - no changes needed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "The replacement would result in no changes to the file".to_string(),
                "Verify that newString contains the intended modifications".to_string(),
                "If no changes are needed, consider skipping this operation".to_string(),
            ])
            .to_mcp_result());
        }

        // Layer 2: Business logic - path validation and file reading
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(
                    guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                        .guidance(vec![
                            "Verify the file exists with listDirectory".to_string(),
                            "Check file permissions".to_string(),
                            "Use readFile to see the current content".to_string(),
                        ])
                        .to_mcp_result(),
                );
            }
        };

        // Count occurrences
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Calculate similarity for suggestions
            let lines: Vec<&str> = original_content.lines().collect();
            let old_lines: Vec<&str> = old_string.lines().collect();
            let search_size = old_lines.len();

            let mut best_match: Option<(usize, f32)> = None; // (line_num, similarity)
            for (line_idx, window) in lines.windows(search_size.max(1)).enumerate() {
                let window_text = window.join("\n");
                let similarity = calculate_similarity(&window_text, old_string);
                if similarity > 0.3 && best_match.as_ref().is_none_or(|m| similarity > m.1) {
                    best_match = Some((line_idx + 1, similarity));
                }
            }

            let (error_msg, guidance) = if let Some((line_num, similarity)) = best_match {
                (
                    format!(
                        "Pattern NOT FOUND (but {}% similar at line {})",
                        (similarity * 100.0) as u32,
                        line_num
                    ),
                    vec![
                        format!("Use readFile('{}', {}, {}) to see actual content", path_str, line_num, line_num + search_size.saturating_sub(1)),
                        "Extract the EXACT text from readFile response including ALL whitespace (newlines, indentation)".to_string(),
                        "Include 3-5 lines of surrounding context for uniqueness".to_string(),
                    ],
                )
            } else {
                (
                    "Pattern NOT FOUND in file".to_string(),
                    vec![
                        format!("Use readFile('{}') to see current file content", path_str),
                        "Verify you are copying the text exactly as it appears in the latest readFile output".to_string(),
                    ],
                )
            };

            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                &error_msg,
                ToolGroup::Workspace,
            )
            .guidance(guidance)
            .to_mcp_result());
        }

        if occurrences > 1 {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Pattern found {} times (not unique)", occurrences),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Include more surrounding context (3-5 more lines) to make the pattern unique"
                    .to_string(),
                format!(
                    "Use readFile('{}') to see full content and find unique context",
                    path_str
                ),
            ])
            .to_mcp_result());
        }

        // Apply replacement
        let new_content = original_content.replacen(old_string, new_string, 1);
        let write_result = tokio::fs::write(&safe_path, &new_content).await;

        match write_result {
            Ok(_) => {
                info!("Successfully edited file: {}", path_str);

                // Invalidate service context cache
                self.invalidate_context_cache().await;

                // Generate diff output
                let diff_output = format_string_diff(
                    &[(old_string.to_string(), new_string.to_string())],
                    path_str,
                );

                let output = format!(
                    "**✅ File Edited Successfully**\n\n\
                    **File:** `{}`\n\n\
                    **Changes:**\n\
                    {}",
                    path_str, diff_output
                );

                let hint = SuccessHint::new(
                    output,
                    vec![
                        "Use readFile to verify the changes".to_string(),
                        "For multiple changes, call editFile again or use editFileMulti"
                            .to_string(),
                        "Each replacement is atomic and independent".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "path": path_str,
                    "old_string_length": old_string.len(),
                    "new_string_length": new_string.len(),
                    "diff": diff_output,
                }))))
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.to_string().contains("Permission denied") =>
            {
                Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Permission denied writing to '{}'", path_str),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "File may be read-only or locked by another process".to_string(),
                    "Use listDirectory to check file permissions".to_string(),
                ])
                .to_mcp_result())
            }
            Err(e) => Ok(guided_error(
                ErrorCategory::OperationFailed,
                e.to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "File may be locked or inaccessible".to_string(),
                format!("Use readFile('{}') to verify file still exists", path_str),
            ])
            .to_mcp_result()),
        }
    }
}
