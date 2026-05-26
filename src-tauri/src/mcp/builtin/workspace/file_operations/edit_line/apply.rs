use super::super::super::WorkspaceServer;
use super::super::utils::{
    compute_line_hash, format_hashline, format_prefix_hash, initial_prefix_hash_state,
    parse_anchor, read_file_as_string, update_prefix_hash_state,
};
use super::types::{EditAction, LineEdit, PreparedFileEdit};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

fn validate_edits_do_not_overlap(edits: &[LineEdit]) -> Result<(), MCPResult> {
    let mut sorted_ranges: Vec<(usize, usize, usize)> = edits
        .iter()
        .enumerate()
        .map(|(index, edit)| (edit.start_line, edit.end_line, index))
        .collect();
    sorted_ranges.sort_by_key(|&(start, _, _)| start);

    for window in sorted_ranges.windows(2) {
        let (start_a, end_a, idx_a) = window[0];
        let (start_b, _, idx_b) = window[1];

        if start_a == 0 && start_b == 0 {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Conflicting edits: edit #{} and edit #{} both insert at the beginning of the file",
                    idx_a, idx_b
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Only one insert_after edit with startLine 0 is allowed per file".to_string()
            ])
            .to_mcp_result());
        }

        if start_b <= end_a && start_b > 0 {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Overlapping edits: edit #{} overlaps with edit #{}",
                    idx_a, idx_b
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Each line can only be covered by one edit per file".to_string()
            ])
            .to_mcp_result());
        }
    }

    Ok(())
}

fn apply_edits(orig_lines: &[&str], edits: &[LineEdit]) -> Vec<String> {
    let mut modified: Vec<String> = orig_lines.iter().map(|line| (*line).to_string()).collect();
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|edit| std::cmp::Reverse(edit.start_line));

    for edit in &sorted {
        let replacement: Vec<String> = if edit.new_value.is_empty() {
            Vec::new()
        } else {
            edit.new_value
                .lines()
                .map(|line| line.to_string())
                .collect()
        };

        match edit.action {
            EditAction::InsertAfter => {
                let insert_idx = edit.start_line;
                modified.splice(insert_idx..insert_idx, replacement);
            }
            EditAction::Replace | EditAction::Delete => {
                let start_idx = edit.start_line - 1;
                modified.splice(start_idx..edit.end_line, replacement);
            }
        }
    }

    modified
}

fn build_new_hash_sections(edits: &[LineEdit], new_content: &str) -> Vec<String> {
    let new_content_lines: Vec<&str> = new_content.lines().collect();
    let mut full_prefix_state = initial_prefix_hash_state();
    let full_hashlines: Vec<String> = new_content_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| format_hashline(idx + 1, line, &mut full_prefix_state))
        .collect();
    let mut sorted_asc = edits.to_vec();
    sorted_asc.sort_by_key(|edit| edit.start_line);

    let mut new_hash_sections = Vec::new();
    let mut line_delta: i64 = 0;
    for edit in &sorted_asc {
        let replacement_line_count = edit.replacement_line_count();
        let original_line_count = if edit.action == EditAction::InsertAfter {
            0
        } else {
            (edit.end_line - edit.start_line + 1) as i64
        };

        if replacement_line_count == 0 {
            line_delta += replacement_line_count as i64 - original_line_count;
            continue;
        }

        let start_in_new = if edit.action == EditAction::InsertAfter {
            (edit.start_line as i64 + line_delta) as usize
        } else {
            ((edit.start_line as i64 - 1) + line_delta) as usize
        };

        let end_in_new = (start_in_new + replacement_line_count).min(new_content_lines.len());
        let section: Vec<String> = full_hashlines[start_in_new..end_in_new].to_vec();
        if !section.is_empty() {
            new_hash_sections.push(section.join("\n"));
        }

        line_delta += replacement_line_count as i64 - original_line_count;
    }

    new_hash_sections
}

fn validate_edit_anchors(
    path_str: &str,
    edit: &LineEdit,
    line_count: usize,
    orig_lines: &[&str],
    prefix_hashes: &[String],
) -> Result<(), MCPResult> {
    if !edit.requires_existing_line_anchor() {
        return Ok(());
    }

    if edit.start_line > line_count {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "File '{}': line {} does not exist (file has {} lines)",
                path_str, edit.start_line, line_count
            ),
            ToolGroup::Workspace,
        )
        .to_mcp_result());
    }

    if matches!(edit.action, EditAction::Replace | EditAction::Delete) && edit.end_line > line_count
    {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "File '{}': end line {} does not exist (file has {} lines)",
                path_str, edit.end_line, line_count
            ),
            ToolGroup::Workspace,
        )
        .to_mcp_result());
    }

    let expected_anchor = edit
        .start_anchor
        .as_ref()
        .expect("start_anchor required for existing-line edits");
    let (expected_hash, expected_prefix) = match parse_anchor(expected_anchor) {
        Some(parts) => parts,
        None => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "File '{}': invalid anchor for line {}: expected 6-character hexadecimal code",
                    path_str, edit.start_line
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Run readFile(showLineAnchors=true) or search(showLineAnchors=true) again"
                    .to_string(),
                "Copy only the 6-character anchor from the returned line format N:anchor|content. Example: from '42:a31f2c|let x = 1;', pass only 'a31f2c'."
                    .to_string(),
            ])
            .to_mcp_result());
        }
    };

    let actual_line = orig_lines[edit.start_line - 1];
    let actual_hash = compute_line_hash(actual_line);
    if actual_hash != expected_hash {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "File '{}': STALE ANCHOR on line {} (current line content changed)",
                path_str, edit.start_line
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run readFile with showLineAnchors=true to get current anchors".to_string(),
            "Rebuild the edit using the latest anchor".to_string(),
        ])
        .to_mcp_result());
    }

    let actual_prefix_hash = &prefix_hashes[edit.start_line - 1];
    if actual_prefix_hash != expected_prefix {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "File '{}': STALE ANCHOR on line {} (earlier content changed before this line)",
                path_str, edit.start_line
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run readFile with showLineAnchors=true to get current anchors".to_string(),
            "Rebuild the edit using the latest anchor".to_string(),
        ])
        .to_mcp_result());
    }

    if edit.requires_end_hash() {
        let expected_end_anchor = edit
            .end_anchor
            .as_ref()
            .expect("end_anchor required for multi-line replace/delete");
        let (expected_end_hash, expected_end_prefix) = match parse_anchor(expected_end_anchor) {
            Some(parts) => parts,
            None => {
                return Err(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "File '{}': invalid endAnchor for line {}: expected 6-character hexadecimal code",
                        path_str, edit.end_line
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Run readFile(showLineAnchors=true) or search(showLineAnchors=true) again"
                        .to_string(),
                    "Copy only the 6-character endAnchor from the returned line format N:anchor|content. Example: from '42:a31f2c|let x = 1;', pass only 'a31f2c'.".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let actual_end_line = orig_lines[edit.end_line - 1];
        let actual_end_hash = compute_line_hash(actual_end_line);
        if actual_end_hash != expected_end_hash {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "File '{}': STALE END ANCHOR on line {} (range boundary changed)",
                    path_str, edit.end_line
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Run readFile with showLineAnchors=true to get the current end anchor".to_string(),
                "Rebuild the edit with an updated endAnchor".to_string(),
            ])
            .to_mcp_result());
        }

        let actual_end_prefix = &prefix_hashes[edit.end_line - 1];
        if actual_end_prefix != expected_end_prefix {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "File '{}': STALE END ANCHOR on line {} (earlier content changed before range boundary)",
                    path_str, edit.end_line
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Run readFile with showLineAnchors=true to get the current end anchor".to_string(),
                "Rebuild the edit with an updated endAnchor".to_string(),
            ])
            .to_mcp_result());
        }
    }

    Ok(())
}

pub(super) async fn prepare_file_edit_batch(
    server: &WorkspaceServer,
    path_str: &str,
    edits: Vec<LineEdit>,
    session_id: Option<String>,
) -> Result<PreparedFileEdit, MCPResult> {
    validate_edits_do_not_overlap(&edits)?;

    let safe_path = match server.validate_path_with_error_for_write(path_str, session_id) {
        Ok(path) => path,
        Err(error) => {
            return Err(guided_error(
                ErrorCategory::PermissionDenied,
                format!("Path validation failed for '{}': {}", path_str, error),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use a normal file path without '..' traversal segments".to_string(),
                "Use listDirectory to inspect valid target paths".to_string(),
            ])
            .to_mcp_result());
        }
    };

    let original_content = match read_file_as_string(&safe_path).await {
        Ok(content) => content,
        Err(error) => {
            return Err(
                guided_error(ErrorCategory::OperationFailed, error, ToolGroup::Workspace)
                    .to_mcp_result(),
            );
        }
    };

    let orig_lines: Vec<&str> = original_content.lines().collect();
    let line_count = orig_lines.len();
    let mut prefix_state = initial_prefix_hash_state();
    let prefix_hashes: Vec<String> = orig_lines
        .iter()
        .map(|line| {
            prefix_state = update_prefix_hash_state(prefix_state, line);
            format_prefix_hash(prefix_state)
        })
        .collect();

    for edit in &edits {
        validate_edit_anchors(path_str, edit, line_count, &orig_lines, &prefix_hashes)?;
    }

    let modified_lines = apply_edits(&orig_lines, &edits);
    let new_content = modified_lines.join("\n");
    let new_content = if original_content.ends_with('\n') && !new_content.ends_with('\n') {
        format!("{}\n", new_content)
    } else {
        new_content
    };

    Ok(PreparedFileEdit {
        path: path_str.to_string(),
        edits: edits.clone(),
        original_content,
        new_content: new_content.clone(),
        original_line_count: line_count,
        new_hash_sections: build_new_hash_sections(&edits, &new_content),
    })
}
