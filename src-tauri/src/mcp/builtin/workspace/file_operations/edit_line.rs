use super::super::WorkspaceServer;
use super::utils::read_file_as_string;
use crate::mcp::builtin::error_guidance::*;
use crate::mcp::types::MCPResult;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct LineEdit {
    line: usize,
    old_value: Option<String>,
    new_value: String,
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
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::MissingRequiredParam,
                    "Parameter 'edits' is required and must be an array",
                    vec![
                        "Provide an array of edit objects: [{line, old_value?, new_value}, ...]"
                            .to_string(),
                        "Use searchLineInFile to find line numbers first".to_string(),
                        "Example: {\"edits\": [{\"line\": 10, \"new_value\": \"updated text\"}]}"
                            .to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        if edits_array.is_empty() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Parameter 'edits' cannot be empty",
                vec![
                    "Provide at least one edit operation".to_string(),
                    "For single edits, consider using editFile instead".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Parse edit operations
        let mut edits = Vec::new();
        let mut line_numbers = HashSet::new();

        for (idx, edit_obj) in edits_array.iter().enumerate() {
            let edit_obj = match edit_obj.as_object() {
                Some(obj) => obj,
                None => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {} must be an object", idx),
                        vec![
                            "Each edit must be an object with 'line' and 'new_value' fields"
                                .to_string(),
                            "Example: {\"line\": 10, \"new_value\": \"updated text\"}".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };

            // Extract line number (1-based)
            let line = match edit_obj.get("line").and_then(|v| v.as_u64()) {
                Some(n) if n > 0 => n as usize,
                Some(0) => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Edit at index {}: Line numbers must be 1-based (starting from 1)",
                            idx
                        ),
                        vec![
                            "Line numbers start from 1, not 0".to_string(),
                            "Use searchLineInFile to get correct line numbers".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
                _ => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: 'line' field is required and must be a positive integer", idx),
                        vec![
                            "Provide line number as integer (e.g., \"line\": 10)".to_string(),
                            "Use searchLineInFile to find line numbers".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };

            // Check for duplicate line numbers
            if !line_numbers.insert(line) {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Duplicate line number {} - each line can only be edited once per operation", line),
                    vec![
                        "Remove duplicate line numbers from edits array".to_string(),
                        "Combine multiple changes to the same line into one edit".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            // Extract new_value (required, must be single-line)
            let new_value = match edit_obj.get("new_value").and_then(|v| v.as_str()) {
                Some(s) => {
                    if s.contains('\n') {
                        return Ok(ErrorGuidance::with_guidance(
                            ErrorCategory::InvalidInput,
                            format!("Edit at index {}: new_value must be single-line (no newline characters)", idx),
                            vec![
                                "Remove \\n characters from new_value".to_string(),
                                "For multi-line replacements, use editFile instead".to_string(),
                                "editLineInFile is designed for single-line edits only".to_string(),
                            ],
                            ToolGroup::Workspace,
                        )
                        .to_mcp_result());
                    }
                    s.to_string()
                }
                None => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: 'new_value' field is required", idx),
                        vec![
                            "Provide new_value as string (e.g., \"new_value\": \"updated text\")"
                                .to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };

            // Extract old_value (optional validation)
            let old_value = edit_obj
                .get("old_value")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            edits.push(LineEdit {
                line,
                old_value,
                new_value,
            });
        }

        // Validate path and read file
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for line editing",
                    &e.to_string(),
                    vec![
                        "Verify the file exists with listDirectory".to_string(),
                        "Check file permissions".to_string(),
                        "Ensure the path is correct".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        let lines: Vec<&str> = original_content.lines().collect();
        let line_count = lines.len();

        // Check line count limit (10,000 lines)
        const MAX_LINES: usize = 10_000;
        if line_count > MAX_LINES {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "File has {} lines, exceeding the limit of {} lines",
                    line_count, MAX_LINES
                ),
                vec![
                    "Files exceeding 10,000 lines are beyond practical LLM context windows"
                        .to_string(),
                    "Consider splitting the file into smaller modules".to_string(),
                    "Use editFile for smaller, targeted changes instead".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Validate all line numbers exist
        for edit in &edits {
            if edit.line > line_count {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line {} does not exist (file has {} lines)",
                        edit.line, line_count
                    ),
                    vec![
                        format!("Valid line range: 1-{}", line_count),
                        "Use searchLineInFile to find correct line numbers".to_string(),
                        "Use readFile to see file structure".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            // Validate old_value if provided
            if let Some(ref expected) = edit.old_value {
                let actual = lines[edit.line - 1]; // Convert to 0-based
                if actual != expected {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Line {} content mismatch:\nExpected: \"{}\"\nActual: \"{}\"",
                            edit.line, expected, actual
                        ),
                        vec![
                            "Call readFile FIRST to get current line content".to_string(),
                            "Use exact text from readFile response for old_value".to_string(),
                            "File may have been modified since last read".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
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

        // Generate simple diff showing changed lines
        let orig_lines: Vec<&str> = original_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();
        let mut diff_output = String::new();

        for edit in &edits {
            let line_idx = edit.line - 1;
            if line_idx < orig_lines.len() {
                diff_output.push_str(&format!("-{}: {}\n", edit.line, orig_lines[line_idx]));
            }
            if line_idx < new_lines.len() {
                diff_output.push_str(&format!("+{}: {}\n", edit.line, new_lines[line_idx]));
            }
        }

        // Write file using file manager
        let file_manager = self.get_file_manager(session_id.clone());
        if let Err(e) = file_manager.write_file_string(path_str, &new_content).await {
            return Ok(operation_failed_error(
                "Write file after line edits",
                &e,
                vec![
                    "Check file permissions".to_string(),
                    "Ensure sufficient disk space".to_string(),
                    "Verify the file is not locked by another process".to_string(),
                ],
                ToolGroup::Workspace,
            ));
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
