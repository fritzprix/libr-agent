use super::super::utils::get_diff_context_lines;
use super::super::WorkspaceServer;
use super::utils::read_file_as_string;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct LineEdit {
    line: usize,
    old_value: Option<String>,
    new_value: String,
    expected_hash: String, // Now mandatory
}

impl WorkspaceServer {
    pub async fn handle_edit_line_in_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter extraction
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        let edits_array = match args.get("edits").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => {
                return Ok(guided_error(
                    ErrorCategory::MissingRequiredParam,
                    "Parameter 'edits' is required and must be an array",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide an array of edit objects: [{line, old_value?, new_value}, ...]"
                        .to_string(),
                    "Use searchLineInFile to find line numbers first".to_string(),
                    "Example: {\"edits\": [{\"line\": 10, \"new_value\": \"updated text\"}]}"
                        .to_string(),
                ])
                .to_mcp_result());
            }
        };

        if edits_array.is_empty() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'edits' cannot be empty",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide at least one edit operation".to_string(),
                "For single edits, consider using editFile instead".to_string(),
            ])
            .to_mcp_result());
        }

        // Parse edit operations
        let mut edits = Vec::new();
        let mut line_numbers = HashSet::new();

        for (idx, edit_obj) in edits_array.iter().enumerate() {
            let edit_obj = match edit_obj.as_object() {
                Some(obj) => obj,
                None => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {} must be an object", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Each edit must be an object with 'line' and 'new_value' fields"
                            .to_string(),
                        "Example: {\"line\": 10, \"new_value\": \"updated text\"}".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // Extract line number (1-based)
            let line = match edit_obj.get("line").and_then(|v| v.as_u64()) {
                Some(n) if n > 0 => n as usize,
                Some(0) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Edit at index {}: Line numbers must be 1-based (starting from 1)",
                            idx
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "The map is not the territory. Line numbers start at 1.".to_string(),
                        "Your index is off by one. Adjust your coordinates.".to_string(),
                        "Use searchLineInFile to verify the terrain before striking.".to_string(),
                    ])
                    .to_mcp_result());
                }
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: 'line' field is required and must be a positive integer", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Provide line number as integer (e.g., \"line\": 10)".to_string(),
                        "Use searchLineInFile to find line numbers".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // Check for duplicate line numbers
            if !line_numbers.insert(line) {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Duplicate line number {} - each line can only be edited once per operation", line),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Remove duplicate line numbers from edits array".to_string(),
                    "Combine multiple changes to the same line into one edit".to_string(),
                ])
                .to_mcp_result());
            }

            // Extract new_value (required, must be single-line)
            let new_value = match edit_obj.get("new_value").and_then(|v| v.as_str()) {
                Some(s) => {
                    if s.contains('\n') {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!("Edit at index {}: new_value must be single-line (no newline characters)", idx),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "One line at a time. Simplicity is key.".to_string(),
                            "This tool handles single lines only. Remove the newline characters.".to_string(),
                            "For heavier lifting (multi-line changes), use editFile instead.".to_string(),
                        ])
                        .to_mcp_result());
                    }
                    s.to_string()
                }
                None => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: 'new_value' field is required", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Provide new_value as string (e.g., \"new_value\": \"updated text\")"
                            .to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // Extract old_value (optional validation)
            let old_value = edit_obj
                .get("old_value")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Extract expected_hash (MANDATORY)
            let expected_hash = edit_obj
                .get("expected_hash")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    format!(
                        "Missing 'expected_hash' for line {}. You MUST read the file with `showHash: true` first.",
                        line
                    )
                })?
                .to_string();

            edits.push(LineEdit {
                line,
                old_value,
                new_value,
                expected_hash,
            });
        }

        // Validate path and read file
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    e.to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file exists with listDirectory".to_string(),
                    "Check file permissions".to_string(),
                    "Ensure the path is correct".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let lines: Vec<&str> = original_content.lines().collect();
        let line_count = lines.len();

        // Check line count limit (10,000 lines)
        const MAX_LINES: usize = 10_000;
        if line_count > MAX_LINES {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "File has {} lines, exceeding the limit of {} lines",
                    line_count, MAX_LINES
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Files exceeding 10,000 lines are beyond practical LLM context windows".to_string(),
                "Consider splitting the file into smaller modules".to_string(),
                "Use editFile for smaller, targeted changes instead".to_string(),
            ])
            .to_mcp_result());
        }

        // Validate all line numbers exist
        for edit in &edits {
            if edit.line > line_count {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line {} does not exist (file has {} lines)",
                        edit.line, line_count
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    format!("Valid line range: 1-{}", line_count),
                    "Use searchLineInFile to find correct line numbers".to_string(),
                    "Use readFile to see file structure".to_string(),
                ])
                .to_mcp_result());
            }

            // Validate old_value if provided
            if let Some(ref expected) = edit.old_value {
                let actual = lines[edit.line - 1]; // Convert to 0-based
                if actual != expected {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Line {} content mismatch:\nExpected: \"{}\"\nActual: \"{}\"",
                            edit.line, expected, actual
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Call readFile FIRST to get current line content".to_string(),
                        "Use exact text from readFile response for old_value".to_string(),
                        "File may have been modified since last read".to_string(),
                    ])
                    .to_mcp_result());
                }
            }

            // Validate expected_hash (MANDATORY HashLine mechanism)
            let actual_line = lines[edit.line - 1]; // Convert to 0-based
            let actual_hash = super::utils::compute_line_hash(actual_line);
            
            if actual_hash != edit.expected_hash {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line {} hash mismatch:\nExpected: \"{}\"\nActual: \"{}\"",
                        edit.line, edit.expected_hash, actual_hash
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "The line content has changed since you last read it.".to_string(),
                    "Race condition detected! Aborting to prevent data corruption.".to_string(),
                    "Re-read the file with `readFile` (and `showHash: true`) to get updated hashes.".to_string(),
                ])
                .to_mcp_result());
            }
        }

        // Apply edits (in reverse order to maintain line stability)
        let mut modified_lines: Vec<String> = lines.iter().map(|&s| s.to_string()).collect();
        let mut sorted_edits = edits.clone();
        sorted_edits.sort_by(|a, b| b.line.cmp(&a.line)); // High to low

        for edit in sorted_edits {
            modified_lines[edit.line - 1] = edit.new_value; // Convert to 0-based
        }

        let new_content = modified_lines.join("\n");

        // Preserve trailing newline if original had one
        let new_content = if original_content.ends_with('\n') && !new_content.ends_with('\n') {
            format!("{}\n", new_content)
        } else {
            new_content
        };

        // Generate diff with context
        let context_lines = get_diff_context_lines().await;
        let orig_lines: Vec<&str> = original_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();
        let mut diff_output = String::new();

        // Identify changed line indices (0-based)
        let changed_indices: HashSet<usize> = edits.iter().map(|e| e.line - 1).collect();

        // Calculate lines to show
        let mut lines_to_show: Vec<usize> = Vec::new();
        for &idx in &changed_indices {
            let start = idx.saturating_sub(context_lines);
            let end = (idx + context_lines).min(orig_lines.len().saturating_sub(1));
            for i in start..=end {
                lines_to_show.push(i);
            }
        }
        lines_to_show.sort_unstable();
        lines_to_show.dedup();

        // Generate diff
        let mut prev_idx: Option<usize> = None;
        for &idx in &lines_to_show {
            if let Some(prev) = prev_idx {
                if idx > prev + 1 {
                    diff_output.push_str("...\n");
                }
            }
            prev_idx = Some(idx);

            let line_num = idx + 1;
            if changed_indices.contains(&idx) {
                if idx < orig_lines.len() {
                    diff_output.push_str(&format!("-{}: {}\n", line_num, orig_lines[idx]));
                }
                if idx < new_lines.len() {
                    diff_output.push_str(&format!("+{}: {}\n", line_num, new_lines[idx]));
                }
            } else if idx < orig_lines.len() {
                diff_output.push_str(&format!("  {}: {}\n", line_num, orig_lines[idx]));
            }
        }

        // Write file using file manager
        let file_manager = self.get_file_manager(session_id.clone());
        if let Err(e) = file_manager.write_file_string(path_str, &new_content).await {
            return Ok(
                guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                    .guidance(vec![
                        "Check file permissions".to_string(),
                        "Ensure sufficient disk space".to_string(),
                        "Verify the file is not locked by another process".to_string(),
                    ])
                    .to_mcp_result(),
            );
        }

        // Invalidate service context cache
        self.invalidate_context_cache().await;

        // Success response
        let edit_summary = edits
            .iter()
            .map(|e| format!("  Line {}: \"{}\"", e.line, e.new_value))
            .collect::<Vec<_>>()
            .join("\n");

        let hint = SuccessHint::new(
            format!(
                "✓ Applied {} line edit(s) to '{}'\n\n\
                Changes:\n{}\n\n\
                Diff:\n```diff\n{}\n```",
                edits.len(),
                path_str,
                edit_summary,
                diff_output.trim()
            ),
            vec![
                "Use readFile to verify the changes".to_string(),
                "Use searchLineInFile to find other lines to edit".to_string(),
            ],
        );

        Ok(hint.to_mcp_result())
    }
}
