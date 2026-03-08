use super::super::WorkspaceServer;
use super::utils::{compute_line_hash, read_file_as_string};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

/// A single edit operation.
#[derive(Debug, Clone)]
struct LineEdit {
    start_line: usize, // 1-based. 0 is allowed for INSERT_AFTER at top.
    end_line: usize,   // 1-based, inclusive.
    new_value: String,
    start_hash: Option<String>,
    action: EditAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditAction {
    Replace,
    InsertAfter,
    Delete,
}

/// Pure apply function — applies sorted edits (high → low) to a slice of lines.
fn apply_edits(orig_lines: &[&str], edits: &[LineEdit]) -> Vec<String> {
    let mut modified: Vec<String> = orig_lines.iter().map(|&s| s.to_string()).collect();
    let mut sorted = edits.to_vec();
    // Sort high -> low. For InsertAfter at line 0, it stays at the bottom of the sort (lowest index).
    sorted.sort_by(|a, b| b.start_line.cmp(&a.start_line));

    for edit in &sorted {
        let replacement: Vec<String> = if edit.new_value.is_empty() {
            Vec::new()
        } else {
            edit.new_value.lines().map(|s| s.to_string()).collect()
        };

        match edit.action {
            EditAction::InsertAfter => {
                // Insert-after: splice at anchor+1. If anchor is 0, inserts at 0.
                let insert_idx = edit.start_line; // 0-based index where to insert
                modified.splice(insert_idx..insert_idx, replacement);
            }
            EditAction::Replace | EditAction::Delete => {
                // Replace / delete: splice replaces [start..end]
                let start_idx = edit.start_line - 1; // 1-based to 0-based
                modified.splice(start_idx..edit.end_line, replacement);
            }
        }
    }
    modified
}

impl WorkspaceServer {
    pub async fn handle_edit_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // --- Parameter extraction ---
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
                    "Replace:      [{\"line\": 10, \"action\": \"REPLACE\", \"new_value\": \"text\"}]".to_string(),
                    "Insert-top:   [{\"line\": 0, \"action\": \"INSERT_AFTER\", \"new_value\": \"header\"}]".to_string(),
                    "Delete range: [{\"line\": 10, \"endLine\": 15, \"action\": \"DELETE\"}]".to_string(),
                    "Use readFile(showLineHashes=true) to get line + hash values first".to_string(),
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
            .guidance(vec!["Provide at least one edit operation".to_string()])
            .to_mcp_result());
        }

        // --- Parse each edit item ---
        let mut edits: Vec<LineEdit> = Vec::with_capacity(edits_array.len());

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
                        "Single-line: {\"line\": 10, \"new_value\": \"text\"}".to_string(),
                        "Range:       {\"line\": 10, \"endLine\": 15, \"new_value\": \"...\"}"
                            .to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // `action` — REPLACE, INSERT_AFTER, DELETE
            let action_str = edit_obj.get("action").and_then(|v| v.as_str());
            let action = match action_str {
                Some("REPLACE") => EditAction::Replace,
                Some("INSERT_AFTER") => EditAction::InsertAfter,
                Some("DELETE") => EditAction::Delete,
                Some(other) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: invalid action '{}'", idx, other),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Supported actions: REPLACE, INSERT_AFTER, DELETE".to_string()
                    ])
                    .to_mcp_result());
                }
                None => {
                    // Legacy/Implicit detection
                    if edit_obj
                        .get("insertAfter")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        EditAction::InsertAfter
                    } else if let Some(v) = edit_obj.get("new_value") {
                        if v.as_str() == Some("") {
                            EditAction::Delete
                        } else {
                            EditAction::Replace
                        }
                    } else {
                        EditAction::Replace // Default
                    }
                }
            };

            // `line` — start line (1-based, except line: 0 for INSERT_AFTER)
            let start_line = match edit_obj.get("line").and_then(|v| v.as_u64()) {
                Some(n) => n as usize,
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Edit at index {}: 'line' field is required and must be an integer",
                            idx
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Provide line number as integer (e.g., \"line\": 10)".to_string(),
                        "Use line: 0 ONLY with action='INSERT_AFTER' to insert at top".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            if start_line == 0 && action != EditAction::InsertAfter {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Edit at index {}: line 0 is only valid for action 'INSERT_AFTER'",
                        idx
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "To insert at the beginning, use line: 0 and action: 'INSERT_AFTER'"
                        .to_string(),
                ])
                .to_mcp_result());
            }

            // `endLine` validation
            let has_end_line = edit_obj.get("endLine").is_some();
            let end_line = match edit_obj.get("endLine").and_then(|v| v.as_u64()) {
                Some(n) if n >= start_line as u64 => n as usize,
                Some(n) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Edit at index {}: 'endLine' ({}) must be ≥ 'line' ({})",
                            idx, n, start_line
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec!["endLine must be ≥ line".to_string()])
                    .to_mcp_result());
                }
                None => {
                    if start_line == 0 {
                        0
                    } else {
                        start_line
                    }
                }
            };

            // Check for mutual exclusivity of INSERT_AFTER and endLine
            if action == EditAction::InsertAfter && has_end_line {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Edit at index {}: 'endLine' cannot be used with action 'INSERT_AFTER'",
                        idx
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "INSERT_AFTER only targets one line (or line 0). Remove 'endLine'.".to_string(),
                ])
                .to_mcp_result());
            }

            // `new_value` extraction
            let new_value = match edit_obj.get("new_value").and_then(|v| v.as_str()) {
                Some(s) => {
                    // Forbid \n in single-line replace mode only
                    if action == EditAction::Replace && end_line == start_line && s.contains('\n') {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!(
                                "Edit at index {}: single-line REPLACE cannot contain \\n",
                                idx
                            ),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "To replace multiple lines: add 'endLine'".to_string(),
                            "To insert new lines after a line: use action: 'INSERT_AFTER'"
                                .to_string(),
                        ])
                        .to_mcp_result());
                    }
                    s.to_string()
                }
                None => {
                    if action == EditAction::Delete {
                        String::new()
                    } else {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!("Edit at index {}: 'new_value' is required for REPLACE and INSERT_AFTER", idx),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "Provide replacement/insertion content as a string".to_string()
                        ])
                        .to_mcp_result());
                    }
                }
            };

            let start_hash = edit_obj
                .get("line_hash")
                .or_else(|| edit_obj.get("startHash"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            edits.push(LineEdit {
                start_line,
                end_line,
                new_value,
                start_hash,
                action,
            });
        }

        // --- Overlap detection ---
        {
            let mut sorted_ranges: Vec<(usize, usize, usize)> = edits
                .iter()
                .enumerate()
                .map(|(i, e)| (e.start_line, e.end_line, i))
                .collect();
            sorted_ranges.sort_by_key(|&(s, _, _)| s);

            for window in sorted_ranges.windows(2) {
                let (_start_a, end_a, idx_a) = window[0];
                let (start_b, _, idx_b) = window[1];

                if start_b <= end_a && start_b > 0 {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Overlapping edits: edit #{} overlaps with edit #{}",
                            idx_a, idx_b
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Each line can only be covered by one edit per operation".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        }

        let safe_path = self.validate_path_with_error_for_write(path_str, session_id.clone())?;
        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(
                    guided_error(ErrorCategory::OperationFailed, e, ToolGroup::Workspace)
                        .to_mcp_result(),
                )
            }
        };

        let orig_lines: Vec<&str> = original_content.lines().collect();
        let line_count = orig_lines.len();

        // --- Validate all edits against file content ---
        for edit in &edits {
            if edit.start_line > line_count {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line {} does not exist (file has {} lines)",
                        edit.start_line, line_count
                    ),
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            if matches!(edit.action, EditAction::Replace | EditAction::Delete)
                && edit.end_line > line_count
            {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "End line {} does not exist (file has {} lines)",
                        edit.end_line, line_count
                    ),
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            // Hash validation
            if edit.start_line > 0 {
                if let Some(ref expected) = edit.start_hash {
                    let actual = orig_lines[edit.start_line - 1];
                    let actual_hash = compute_line_hash(actual);
                    if actual_hash != *expected {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!(
                                "STALE HASH on line {} (current: {})",
                                edit.start_line, actual_hash
                            ),
                            ToolGroup::Workspace,
                        )
                        .to_mcp_result());
                    }
                }
            }
        }

        let modified_lines = apply_edits(&orig_lines, &edits);
        let new_content = modified_lines.join("\n");
        let new_content = if original_content.ends_with('\n') && !new_content.ends_with('\n') {
            format!("{}\n", new_content)
        } else {
            new_content
        };

        let file_manager = self.get_file_manager(session_id.clone());
        if let Err(e) = file_manager.write_file_string(path_str, &new_content).await {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                e.to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Check file permissions".to_string(),
                "Ensure sufficient disk space".to_string(),
            ])
            .to_mcp_result());
        }

        self.invalidate_context_cache().await;

        // --- Success response with summary, diff, and new hashlines ---
        let edit_summary = edits
            .iter()
            .map(|e| match e.action {
                EditAction::InsertAfter => format!(
                    "  Insert after line {}: {} line(s)",
                    e.start_line,
                    e.new_value.lines().count()
                ),
                EditAction::Delete => format!("  Delete lines {}-{}", e.start_line, e.end_line),
                EditAction::Replace => format!(
                    "  Replace lines {}-{}: {} line(s)",
                    e.start_line,
                    e.end_line,
                    e.new_value.lines().count()
                ),
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Simple diff (just added/removed counts for now to stay concise)
        let diff_summary = format!(
            "{} lines in, {} lines out",
            new_content.lines().count(),
            line_count
        );

        let new_content_lines: Vec<&str> = new_content.lines().collect();
        let mut sorted_asc = edits.clone();
        sorted_asc.sort_by_key(|e| e.start_line);

        let mut new_hash_sections = Vec::new();
        let mut line_delta: i64 = 0;
        for edit in &sorted_asc {
            let n_lines = edit
                .new_value
                .lines()
                .count()
                .max(if edit.new_value.is_empty() { 0 } else { 1 });
            let start_in_new = if edit.action == EditAction::InsertAfter {
                (edit.start_line as i64 + line_delta) as usize
            } else {
                ((edit.start_line as i64 - 1) + line_delta) as usize
            };

            let end_in_new = (start_in_new + n_lines).min(new_content_lines.len());
            let section: Vec<String> = new_content_lines[start_in_new..end_in_new]
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    format!(
                        "{}:{}|{}",
                        start_in_new + i + 1,
                        compute_line_hash(line),
                        line
                    )
                })
                .collect();
            new_hash_sections.push(section.join("\n"));

            let orig_len = if edit.action == EditAction::InsertAfter {
                0
            } else {
                (edit.end_line - edit.start_line + 1) as i64
            };
            line_delta += n_lines as i64 - orig_len;
        }

        let hint = SuccessHint::new(
            format!("✓ Applied {} edit(s) to '{}'\n\nChanges:\n{}\n\nSummary: {}\n\nNew hashlines:\n```\n{}\n```", edits.len(), path_str, edit_summary, diff_summary, new_hash_sections.join("\n...\n")),
            vec![
                "Hashes above are current — use directly in the next editFile call".to_string(),
                "Use readFile only if you need broader context beyond the edited lines".to_string(),
            ],
        );

        Ok(hint.to_mcp_result())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edit(line: usize, val: &str, action: EditAction) -> LineEdit {
        LineEdit {
            start_line: line,
            end_line: line,
            new_value: val.to_string(),
            start_hash: None,
            action,
        }
    }

    #[test]
    fn test_insert_at_top() {
        let orig = vec!["line1", "line2"];
        let edits = vec![make_edit(0, "header", EditAction::InsertAfter)];
        let res = apply_edits(&orig, &edits);
        assert_eq!(res, vec!["header", "line1", "line2"]);
    }

    #[test]
    fn test_replace_and_insert() {
        let orig = vec!["a", "b", "c"];
        let edits = vec![
            make_edit(1, "A", EditAction::Replace),
            make_edit(2, "B+", EditAction::InsertAfter),
        ];
        let res = apply_edits(&orig, &edits);
        assert_eq!(res, vec!["A", "b", "B+", "c"]);
    }
}
