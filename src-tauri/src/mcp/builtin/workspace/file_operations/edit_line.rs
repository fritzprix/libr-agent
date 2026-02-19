use super::super::utils::get_diff_context_lines;
use super::super::WorkspaceServer;
use super::utils::{compute_line_hash, read_file_as_string};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

/// A single edit operation — either a single-line replacement or a line-range replacement.
///
/// Single-line mode (no `end_line`):
///   - `new_value` must not contain `\n`
///   - `old_value` exact-match validation applies
///   - `start_hash` validated against hash of that line
///
/// Range mode (`end_line > start_line`):
///   - `new_value` may contain `\n` (splices in multiple lines)
///   - `old_value` is ignored (range too wide for single string match)
///   - `start_hash` validated against first line, `end_hash` against last line
#[derive(Debug, Clone)]
struct LineEdit {
    start_line: usize,
    end_line: usize,
    new_value: String,
    old_value: Option<String>,
    start_hash: Option<String>,
    end_hash: Option<String>,
}

impl LineEdit {
    fn is_range(&self) -> bool {
        self.end_line > self.start_line
    }
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
                    "Single-line: [{\"line\": 10, \"line_hash\": \"a3\", \"new_value\": \"text\"}]"
                        .to_string(),
                    "Range:       [{\"line\": 10, \"endLine\": 15, \"new_value\": \"line1\\nline2\"}]"
                        .to_string(),
                    "Use readFile(showLineHashes=true) to get line + hash values first"
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

            // `new_value` — replacement content (required)
            let new_value = match edit_obj.get("new_value").and_then(|v| v.as_str()) {
                Some(s) => {
                    // only forbid newlines in single-line mode
                    if end_line == start_line && s.contains('\n') {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            format!(
                                "Edit at index {}: single-line edit cannot contain \\n — provide 'endLine' for range replacements",
                                idx
                            ),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "Add 'endLine' to enable multi-line new_value".to_string(),
                            "Example: {\"line\": 10, \"endLine\": 12, \"new_value\": \"a\\nb\\nc\"}".to_string(),
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
                    .guidance(vec!["Provide replacement content as a string".to_string()])
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

        // --- Read file ---
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
            // Bounds check
            if edit.start_line > line_count {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line {} does not exist (file has {} lines)",
                        edit.start_line, line_count
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![format!("Valid range: 1-{}", line_count)])
                .to_mcp_result());
            }
            if edit.end_line > line_count {
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
        let mut modified_lines: Vec<String> = orig_lines.iter().map(|&s| s.to_string()).collect();
        let mut sorted_edits = edits.clone();
        sorted_edits.sort_by(|a, b| b.start_line.cmp(&a.start_line)); // high → low

        for edit in &sorted_edits {
            let start_idx = edit.start_line - 1; // 0-based
            let end_idx = edit.end_line; // exclusive end for splice

            let replacement: Vec<String> = edit.new_value.lines().map(|s| s.to_string()).collect();

            modified_lines.splice(start_idx..end_idx, replacement);
        }

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

        // Build a set of original line indices that are in a changed range
        let changed_orig_indices: std::collections::HashSet<usize> = edits
            .iter()
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
                // Show removed line
                diff_output.push_str(&format!("-{}: {}\n", line_num, orig_lines[orig_idx]));
            } else {
                diff_output.push_str(&format!("  {}: {}\n", line_num, orig_lines[orig_idx]));
            }
        }

        // Show added lines for each changed region
        for edit in &edits {
            if !edit.new_value.is_empty() {
                for (i, new_line) in edit.new_value.lines().enumerate() {
                    diff_output.push_str(&format!("+{}: {}\n", edit.start_line + i, new_line));
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
                if e.is_range() {
                    format!(
                        "  Lines {}-{}: {} line(s) → {} line(s)",
                        e.start_line,
                        e.end_line,
                        e.end_line - e.start_line + 1,
                        e.new_value.lines().count()
                    )
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
            let actual_start = ((edit.start_line as i64 - 1) + line_delta) as usize; // 0-based
            let new_line_count = edit.new_value.lines().count().max(1);
            let section_end = (actual_start + new_line_count).min(total_new_lines);

            let section: Vec<String> = new_content_lines[actual_start..section_end]
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let line_num = actual_start + i + 1; // 1-based
                    let hash = compute_line_hash(line);
                    format!("{}:{}|{}", line_num, hash, line)
                })
                .collect();

            new_hash_sections.push(section.join("\n"));

            let orig_range = (edit.end_line - edit.start_line + 1) as i64;
            line_delta += new_line_count as i64 - orig_range;
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
