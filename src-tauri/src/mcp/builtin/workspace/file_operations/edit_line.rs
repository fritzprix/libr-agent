use super::super::utils::get_diff_context_lines;
use super::super::WorkspaceServer;
use super::utils::{compute_line_hash, read_file_as_string};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

/// A single edit operation.
///
/// Replace mode (default): replaces line(s) [start_line..end_line] with new_value.
///   - Single-line (end_line == start_line): new_value must not contain `\n`
///   - Range (end_line > start_line): new_value may contain `\n`
///   - Empty new_value → deletion
///   - start_hash / end_hash validate staleness
///
/// Insert-after mode (insert_after = true):
///   - Inserts new_value AFTER start_line without touching the anchor line itself
///   - new_value may contain `\n` (becomes multiple inserted lines)
///   - start_hash validates the anchor line for staleness
///   - end_line / end_hash / old_value are ignored
#[derive(Debug, Clone)]
struct LineEdit {
    start_line: usize,
    end_line: usize,
    new_value: String,
    old_value: Option<String>,
    start_hash: Option<String>,
    end_hash: Option<String>,
    insert_after: bool,
}

impl LineEdit {
    fn is_range(&self) -> bool {
        !self.insert_after && self.end_line > self.start_line
    }
}

/// Pure apply function — applies sorted edits (high → low) to a slice of lines.
/// Extracted for testability; used by `handle_replace_lines`.
fn apply_edits(orig_lines: &[&str], edits: &[LineEdit]) -> Vec<String> {
    let mut modified: Vec<String> = orig_lines.iter().map(|&s| s.to_string()).collect();
    let mut sorted = edits.to_vec();
    sorted.sort_by(|a, b| b.start_line.cmp(&a.start_line)); // high → low to preserve indices

    for edit in &sorted {
        let start_idx = edit.start_line - 1; // 0-based
        let replacement: Vec<String> = edit.new_value.lines().map(|s| s.to_string()).collect();

        if edit.insert_after {
            // Insert-after: splice at anchor+1 without touching the anchor line
            let insert_idx = (start_idx + 1).min(modified.len());
            modified.splice(insert_idx..insert_idx, replacement);
        } else {
            // Replace / delete: splice replaces [start..end]
            modified.splice(start_idx..edit.end_line, replacement);
        }
    }
    modified
}

impl WorkspaceServer {
    pub async fn handle_replace_lines(
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
                    "Replace:      [{\"line\": 10, \"line_hash\": \"a3\", \"new_value\": \"text\"}]".to_string(),
                    "Insert-after: [{\"line\": 10, \"line_hash\": \"a3\", \"insertAfter\": true, \"new_value\": \"new line\"}]".to_string(),
                    "Range:        [{\"line\": 10, \"endLine\": 15, \"new_value\": \"line1\\nline2\"}]".to_string(),
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

            // `line` — start line (1-based, required)
            let start_line = match edit_obj.get("line").and_then(|v| v.as_u64()) {
                Some(n) if n > 0 => n as usize,
                Some(0) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: 'line' must be ≥ 1 (1-based)", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec!["Line numbers start at 1".to_string()])
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
                        "Use readFile(showLineHashes=true) to get line numbers".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // `endLine` — end of range (optional; defaults to start_line for single-line mode)
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
                None => start_line, // single-line mode
            };

            // `insertAfter` — insert mode flag (optional, default false)
            let insert_after = edit_obj
                .get("insertAfter")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // `new_value` — replacement/insertion content (required)
            let new_value = match edit_obj.get("new_value").and_then(|v| v.as_str()) {
                Some(s) => {
                    // Forbid \n in single-line replace mode only
                    // (range mode and insert_after both allow \n)
                    if !insert_after && end_line == start_line && s.contains('\n') {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!(
                                "Edit at index {}: single-line replace cannot contain \\n",
                                idx
                            ),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "To replace multiple lines: add 'endLine'".to_string(),
                            "To insert new lines after a line: add 'insertAfter': true".to_string(),
                        ])
                        .to_mcp_result());
                    }
                    s.to_string()
                }
                None => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {}: 'new_value' is required", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Provide replacement/insertion content as a string".to_string()
                    ])
                    .to_mcp_result());
                }
            };

            // Optional fields
            let old_value = edit_obj
                .get("old_value")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // `line_hash` (canonical) or `startHash` alias
            let start_hash = edit_obj
                .get("line_hash")
                .or_else(|| edit_obj.get("startHash"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let end_hash = edit_obj
                .get("endHash")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            edits.push(LineEdit {
                start_line,
                end_line,
                new_value,
                old_value,
                start_hash,
                end_hash,
                insert_after,
            });
        }

        // --- Overlap detection: sort by start, ensure no ranges overlap ---
        {
            let mut sorted_ranges: Vec<(usize, usize, usize)> = edits
                .iter()
                .enumerate()
                .map(|(i, e)| (e.start_line, e.end_line, i))
                .collect();
            sorted_ranges.sort_by_key(|&(s, _, _)| s);

            for window in sorted_ranges.windows(2) {
                let (_, end_a, idx_a) = window[0];
                let (start_b, _, idx_b) = window[1];
                if start_b <= end_a {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Overlapping edits: edit #{} (ends at line {}) overlaps with edit #{} (starts at line {})",
                            idx_a, end_a, idx_b, start_b
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Each line can only be covered by one edit per operation".to_string(),
                        "Ensure line ranges in 'edits' do not overlap".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        }

        // Validate path — use write validator to block reserved filenames on edits
        let safe_path = self.validate_path_with_error_for_write(path_str, session_id.clone())?;

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
                ])
                .to_mcp_result());
            }
        };

        let orig_lines: Vec<&str> = original_content.lines().collect();
        let line_count = orig_lines.len();

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
                "Consider splitting the file into smaller modules".to_string()
            ])
            .to_mcp_result());
        }

        // --- Validate all edits against file content ---
        for edit in &edits {
            // Bounds check: anchor line must exist in the file
            if edit.start_line > line_count {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line {} does not exist (file has {} lines)",
                        edit.start_line, line_count
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    format!("Valid range: 1-{}", line_count),
                    format!(
                        "To append at end: use line: {}, insertAfter: true",
                        line_count
                    ),
                ])
                .to_mcp_result());
            }
            if edit.end_line > line_count + 1 {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "endLine {} does not exist (file has {} lines)",
                        edit.end_line, line_count
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![format!("Valid range: 1-{}", line_count)])
                .to_mcp_result());
            }

            // old_value validation (single-line only)
            if !edit.is_range() {
                if let Some(ref expected) = edit.old_value {
                    let actual = orig_lines[edit.start_line - 1];
                    if actual != expected {
                        let actual_hash = compute_line_hash(actual);
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!(
                                "CONTENT MISMATCH on line {} — current: \"{}\" (hash: {})\n  your old_value was: \"{}\"",
                                edit.start_line, actual, actual_hash, expected
                            ),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            format!("→ If this IS still the right line: update old_value to match current content, or switch to line_hash: '{}'", actual_hash),
                            "→ If the target moved: use searchLines to find it".to_string(),
                            "→ Do NOT call readFile — current content is already shown above".to_string(),
                        ])
                        .to_mcp_result());
                    }
                }
            }

            // start_hash validation
            if let Some(ref expected) = edit.start_hash {
                let actual = orig_lines[edit.start_line - 1];
                let actual_hash = compute_line_hash(actual);
                if actual_hash != *expected {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "STALE HASH on line {} — retry with line_hash: '{}'\n  (your hash '{}' is outdated)\n  current line: {}:{}|{}",
                            edit.start_line, actual_hash, expected, edit.start_line, actual_hash, actual
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        format!("→ If current content matches your intent: just swap line_hash to '{}' and retry NOW", actual_hash),
                        "→ If content changed unexpectedly: use searchLines to locate where your target moved".to_string(),
                        "→ Do NOT call readFile — everything you need is shown above".to_string(),
                    ])
                    .to_mcp_result());
                }
            }

            // end_hash validation (range mode only)
            if edit.is_range() {
                if let Some(ref expected) = edit.end_hash {
                    let actual = orig_lines[edit.end_line - 1];
                    let actual_hash = compute_line_hash(actual);
                    if actual_hash != *expected {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!(
                                "STALE HASH on endLine {} — retry with endHash: '{}'\n  (your hash '{}' is outdated)\n  current line: {}:{}|{}",
                                edit.end_line, actual_hash, expected, edit.end_line, actual_hash, actual
                            ),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            format!("→ If this is still the correct range boundary: swap endHash to '{}' and retry NOW", actual_hash),
                            "→ If boundary moved: use searchLines to find the new end line".to_string(),
                            "→ Do NOT call readFile — the current hash is already shown above".to_string(),
                        ])
                        .to_mcp_result());
                    }
                }
            }
        }

        // --- Apply edits in reverse order (preserves line indices for subsequent edits) ---
        let modified_lines = apply_edits(&orig_lines, &edits);

        let new_content = modified_lines.join("\n");
        let new_content = if original_content.ends_with('\n') && !new_content.ends_with('\n') {
            format!("{}\n", new_content)
        } else {
            new_content
        };

        // --- Generate diff ---
        let context_lines = get_diff_context_lines().await;
        let mut diff_output = String::new();

        // Collect context windows around each changed region (in original line space)
        let mut regions: Vec<(usize, usize)> = edits
            .iter()
            .map(|e| {
                (
                    e.start_line.saturating_sub(1 + context_lines),
                    (e.end_line + context_lines).min(line_count),
                )
            })
            .collect();
        regions.sort_by_key(|&(s, _)| s);

        // Merge overlapping regions
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in regions {
            if let Some(last) = merged.last_mut() {
                if s <= last.1 {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            merged.push((s, e));
        }

        // Build a set of original line indices that are REPLACED (not insert_after anchors)
        let changed_orig_indices: std::collections::HashSet<usize> = edits
            .iter()
            .filter(|e| !e.insert_after)
            .flat_map(|e| (e.start_line - 1)..e.end_line)
            .collect();

        let mut show_lines: Vec<usize> = merged.iter().flat_map(|&(s, e)| s..e).collect();
        show_lines.sort_unstable();
        show_lines.dedup();

        let mut prev_shown: Option<usize> = None;
        for orig_idx in &show_lines {
            let orig_idx = *orig_idx;
            if let Some(prev) = prev_shown {
                if orig_idx > prev + 1 {
                    diff_output.push_str("...\n");
                }
            }
            prev_shown = Some(orig_idx);

            let line_num = orig_idx + 1;
            if changed_orig_indices.contains(&orig_idx) {
                diff_output.push_str(&format!("-{}: {}\n", line_num, orig_lines[orig_idx]));
            } else {
                diff_output.push_str(&format!("  {}: {}\n", line_num, orig_lines[orig_idx]));
            }
        }

        // Show added lines for each changed region
        for edit in &edits {
            if !edit.new_value.is_empty() {
                // insert_after: new lines sit after the anchor (start_line+1, start_line+2, ...)
                // replace:      new lines start at start_line
                let first_new_line = if edit.insert_after {
                    edit.start_line + 1
                } else {
                    edit.start_line
                };
                for (i, new_line) in edit.new_value.lines().enumerate() {
                    diff_output.push_str(&format!("+{}: {}\n", first_new_line + i, new_line));
                }
            }
        }

        // --- Write file ---
        let file_manager = self.get_file_manager(session_id.clone());
        if let Err(e) = file_manager.write_file_string(path_str, &new_content).await {
            return Ok(
                guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                    .guidance(vec![
                        "Check file permissions".to_string(),
                        "Ensure sufficient disk space".to_string(),
                    ])
                    .to_mcp_result(),
            );
        }

        self.invalidate_context_cache().await;

        // --- Success response ---
        let edit_summary = edits
            .iter()
            .map(|e| {
                if e.insert_after {
                    format!(
                        "  Insert after line {}: {} line(s)",
                        e.start_line,
                        e.new_value.lines().count()
                    )
                } else if e.is_range() {
                    format!(
                        "  Lines {}-{}: {} line(s) → {} line(s)",
                        e.start_line,
                        e.end_line,
                        e.end_line - e.start_line + 1,
                        e.new_value.lines().count()
                    )
                } else if e.new_value.is_empty() {
                    format!("  Line {}: deleted", e.start_line)
                } else {
                    format!("  Line {}: \"{}\"", e.start_line, e.new_value)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Compute new hashlines for edited regions so the agent can immediately
        // do follow-up edits without a readFile roundtrip.
        let new_content_lines: Vec<&str> = new_content.lines().collect();
        let total_new_lines = new_content_lines.len();

        // Process edits in ascending order to track line-number deltas.
        let mut sorted_asc = edits.clone();
        sorted_asc.sort_by_key(|e| e.start_line);

        let mut new_hash_sections: Vec<String> = Vec::new();
        let mut line_delta: i64 = 0;

        for edit in &sorted_asc {
            let new_line_count = edit
                .new_value
                .lines()
                .count()
                .max(if edit.new_value.is_empty() { 0 } else { 1 });

            if edit.insert_after {
                // Inserted lines sit immediately after the (unchanged) anchor line
                // anchor line in new file: (start_line + line_delta)
                let anchor_new = (edit.start_line as i64 + line_delta) as usize; // 1-based
                let insert_start = anchor_new; // 0-based index of first inserted line
                let section_end = (insert_start + new_line_count).min(total_new_lines);

                let section: Vec<String> = new_content_lines[insert_start..section_end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        let line_num = insert_start + i + 1;
                        let hash = compute_line_hash(line);
                        format!("{}:{}|{}", line_num, hash, line)
                    })
                    .collect();

                new_hash_sections.push(section.join("\n"));
                line_delta += new_line_count as i64;
            } else {
                let actual_start = ((edit.start_line as i64 - 1) + line_delta) as usize; // 0-based
                let section_end = (actual_start + new_line_count).min(total_new_lines);

                let section: Vec<String> = new_content_lines[actual_start..section_end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        let line_num = actual_start + i + 1;
                        let hash = compute_line_hash(line);
                        format!("{}:{}|{}", line_num, hash, line)
                    })
                    .collect();

                new_hash_sections.push(section.join("\n"));
                let orig_range = (edit.end_line - edit.start_line + 1) as i64;
                line_delta += new_line_count as i64 - orig_range;
            }
        }

        let new_hashlines_block = new_hash_sections.join("\n...\n");

        let hint = SuccessHint::new(
            format!(
                "✓ Applied {} edit(s) to '{}'\n\nChanges:\n{}\n\nDiff:\n```diff\n{}\n```\n\nNew hashlines (ready for next replaceLines — no re-read needed):\n```\n{}\n```",
                edits.len(),
                path_str,
                edit_summary,
                diff_output.trim(),
                new_hashlines_block
            ),
            vec![
                "Hashes above are current — use directly in the next replaceLines call".to_string(),
                "Use readFile only if you need broader context beyond the edited lines".to_string(),
                "Use searchLines to locate other lines to edit".to_string(),
            ],
        );

        Ok(hint.to_mcp_result())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- helpers ---

    fn make_edit(line: usize, new_value: &str) -> LineEdit {
        LineEdit {
            start_line: line,
            end_line: line,
            new_value: new_value.to_string(),
            old_value: None,
            start_hash: None,
            end_hash: None,
            insert_after: false,
        }
    }

    fn make_range_edit(start: usize, end: usize, new_value: &str) -> LineEdit {
        LineEdit {
            start_line: start,
            end_line: end,
            new_value: new_value.to_string(),
            old_value: None,
            start_hash: None,
            end_hash: None,
            insert_after: false,
        }
    }

    fn make_insert_after(line: usize, new_value: &str) -> LineEdit {
        LineEdit {
            start_line: line,
            end_line: line,
            new_value: new_value.to_string(),
            old_value: None,
            start_hash: None,
            end_hash: None,
            insert_after: true,
        }
    }

    fn lines(s: &str) -> Vec<&str> {
        s.lines().collect()
    }

    // --- replace ---

    #[test]
    fn test_replace_single_line() {
        let orig = lines("line1\nline2\nline3");
        let result = apply_edits(&orig, &[make_edit(2, "replaced")]);
        assert_eq!(result, vec!["line1", "replaced", "line3"]);
    }

    #[test]
    fn test_replace_range_with_fewer_lines() {
        let orig = lines("a\nb\nc\nd\ne");
        // Replace lines 2-4 with a single line
        let result = apply_edits(&orig, &[make_range_edit(2, 4, "merged")]);
        assert_eq!(result, vec!["a", "merged", "e"]);
    }

    #[test]
    fn test_replace_range_with_more_lines() {
        let orig = lines("a\nb\nc");
        // Replace line 2 range (2-2) with two lines
        let result = apply_edits(&orig, &[make_range_edit(2, 2, "x\ny")]);
        assert_eq!(result, vec!["a", "x", "y", "c"]);
    }

    // --- delete ---

    #[test]
    fn test_delete_single_line() {
        let orig = lines("line1\nline2\nline3");
        let result = apply_edits(&orig, &[make_edit(2, "")]);
        assert_eq!(result, vec!["line1", "line3"]);
    }

    #[test]
    fn test_delete_range() {
        let orig = lines("a\nb\nc\nd\ne");
        let result = apply_edits(&orig, &[make_range_edit(2, 4, "")]);
        assert_eq!(result, vec!["a", "e"]);
    }

    #[test]
    fn test_delete_first_line() {
        let orig = lines("first\nsecond\nthird");
        let result = apply_edits(&orig, &[make_edit(1, "")]);
        assert_eq!(result, vec!["second", "third"]);
    }

    #[test]
    fn test_delete_last_line() {
        let orig = lines("first\nsecond\nlast");
        let result = apply_edits(&orig, &[make_edit(3, "")]);
        assert_eq!(result, vec!["first", "second"]);
    }

    // --- insert_after ---

    #[test]
    fn test_insert_after_middle_line() {
        let orig = lines("a\nb\nc");
        let result = apply_edits(&orig, &[make_insert_after(2, "inserted")]);
        assert_eq!(result, vec!["a", "b", "inserted", "c"]);
    }

    #[test]
    fn test_insert_after_anchor_line_is_untouched() {
        // The anchor line must survive intact
        let orig = lines("fn foo() {\n    // body\n}");
        let result = apply_edits(&orig, &[make_insert_after(1, "// new comment")]);
        assert_eq!(result[0], "fn foo() {");
        assert_eq!(result[1], "// new comment");
        assert_eq!(result[2], "    // body");
    }

    #[test]
    fn test_insert_after_last_line_appends() {
        let orig = lines("first\nlast");
        let result = apply_edits(&orig, &[make_insert_after(2, "appended")]);
        assert_eq!(result, vec!["first", "last", "appended"]);
    }

    #[test]
    fn test_insert_after_multiline_new_value() {
        let orig = lines("a\nb");
        let result = apply_edits(&orig, &[make_insert_after(1, "x\ny\nz")]);
        assert_eq!(result, vec!["a", "x", "y", "z", "b"]);
    }

    // --- multiple edits ---

    #[test]
    fn test_multiple_edits_high_to_low_applied_correctly() {
        // Delete line 3, replace line 1 — both should apply independently
        let orig = lines("a\nb\nc\nd");
        let edits = vec![make_edit(1, "A"), make_edit(3, "")];
        let result = apply_edits(&orig, &edits);
        assert_eq!(result, vec!["A", "b", "d"]);
    }

    #[test]
    fn test_delete_then_insert_after_independent_regions() {
        let orig = lines("a\nb\nc\nd\ne");
        // delete line 4, insert after line 1
        let edits = vec![make_edit(4, ""), make_insert_after(1, "inserted")];
        let result = apply_edits(&orig, &edits);
        assert_eq!(result, vec!["a", "inserted", "b", "c", "e"]);
    }

    // --- is_range ---

    #[test]
    fn test_is_range_false_for_single_line() {
        assert!(!make_edit(5, "x").is_range());
    }

    #[test]
    fn test_is_range_true_for_span() {
        assert!(make_range_edit(3, 7, "x").is_range());
    }

    #[test]
    fn test_is_range_false_for_insert_after_even_with_span() {
        let e = LineEdit {
            start_line: 1,
            end_line: 5,
            new_value: "x".to_string(),
            old_value: None,
            start_hash: None,
            end_hash: None,
            insert_after: true,
        };
        assert!(!e.is_range(), "insert_after is never a range");
    }
}
