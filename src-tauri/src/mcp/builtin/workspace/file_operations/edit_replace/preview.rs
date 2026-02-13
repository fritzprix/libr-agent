use super::super::super::WorkspaceServer;
use super::super::utils::{calculate_similarity, read_file_as_string};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};

impl WorkspaceServer {
    pub async fn handle_preview_replacement(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Parameter validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec!["Provide a valid file path".to_string()])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Extract exact text from readFile response".to_string(),
                    "Include surrounding context for uniqueness".to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("oldString", ToolGroup::Workspace));
        };

        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace));
        };

        // Validate path and read file
        let safe_path = self.validate_path_with_error(path_str, session_id)?;
        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(
                    guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                        .guidance(vec![
                            "Verify file exists with listDirectory".to_string(),
                            format!("Use readFile('{}') to check content", path_str),
                        ])
                        .to_mcp_result(),
                );
            }
        };

        // Find matches and generate preview
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Similar content search
            let lines: Vec<&str> = original_content.lines().collect();
            let old_lines: Vec<&str> = old_string.lines().collect();
            let search_size = old_lines.len();

            let mut best_match: Option<(usize, f32)> = None;
            for (line_idx, window) in lines.windows(search_size.max(1)).enumerate() {
                let window_text = window.join("\n");
                let similarity = calculate_similarity(&window_text, old_string);
                if similarity > 0.3 && best_match.is_none_or(|m| similarity > m.1) {
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
                        format!(
                            "Use readFile('{}', {}, {}) to see actual content on these lines",
                            path_str,
                            line_num,
                            line_num + search_size.saturating_sub(1)
                        ),
                        "Extract the EXACT text from readFile response including ALL whitespace"
                            .to_string(),
                    ],
                )
            } else {
                (
                    "Pattern NOT FOUND in file".to_string(),
                    vec![
                        format!("Use readFile('{}') to see full current content", path_str),
                        "Verify the string you are trying to replace is exactly as it appears in the file".to_string(),
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

        // Exactly 1 match - generate context preview
        let preview_diff = generate_replacement_context(&original_content, old_string, new_string);

        let output = format!(
            "**🔍 Preview Replacement**\n\n\
            **File:** `{}`\n\
            **Status:** ✅ EXACT MATCH FOUND\n\n\
            **Changes Preview:**\n\
            ```diff\n\
            {}\n\
            ```",
            path_str, preview_diff
        );

        let hint = SuccessHint::new(
            output,
            vec![
                "Preview looks correct? Call editFile with SAME parameters to apply".to_string(),
                "Use readFile to see full file context if needed".to_string(),
            ],
        );

        Ok(hint.to_mcp_result_with_data(Some(json!({
            "path": path_str,
            "occurrences": 1,
            "status": "ready"
        }))))
    }
}

/// Generate contextual diff preview (shows surrounding lines)
fn generate_replacement_context(content: &str, old_string: &str, new_string: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let search_lines: Vec<&str> = old_string.lines().collect();

    // Find the match location
    for (line_idx, window) in lines.windows(search_lines.len()).enumerate() {
        if window.join("\n") == old_string {
            // Show context: 2 lines before, matched section, 2 lines after
            let context_start = line_idx.saturating_sub(2);
            let context_end = (line_idx + search_lines.len() + 2).min(lines.len());

            let mut diff_lines = Vec::new();
            diff_lines.push(format!(
                "@@ Lines {}-{} (showing context) @@",
                line_idx + 1,
                line_idx + search_lines.len()
            ));

            for (i, line) in lines[context_start..context_end].iter().enumerate() {
                let absolute_line = context_start + i + 1;
                let relative_to_match = (context_start + i) as isize - line_idx as isize;

                if relative_to_match < 0 || relative_to_match >= search_lines.len() as isize {
                    // Context lines (unchanged)
                    diff_lines.push(format!("  {:4} | {}", absolute_line, line));
                } else {
                    // Matched lines (will be replaced)
                    diff_lines.push(format!("- {:4} | {}", absolute_line, line));
                }
            }

            // Show new content
            for (i, new_line) in new_string.lines().enumerate() {
                let target_line = line_idx + i + 1;
                diff_lines.push(format!("+ {:4} | {}", target_line, new_line));
            }

            return diff_lines.join("\n");
        }
    }

    "ERROR: Match location not found (should not happen)".to_string()
}
