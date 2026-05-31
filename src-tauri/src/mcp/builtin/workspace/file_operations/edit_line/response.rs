use super::super::utils::{compute_anchor, initial_prefix_hash_state};
use super::types::{EditAction, PreparedFileEdit};
use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::types::MCPResult;
use serde_json::json;

const MAX_DIFF_PREVIEW_LINES: usize = 50;
const DIFF_CONTEXT_LINES: usize = 1;

#[derive(Clone)]
struct AnchoredLine {
    line_number: usize,
    content: String,
    anchor: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffPreviewKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone)]
struct DiffPreviewLine {
    kind: DiffPreviewKind,
    content: String,
    line_number: usize,
    anchor: Option<String>,
}

impl DiffPreviewLine {
    fn render_hashline(&self) -> String {
        match self.anchor.as_deref() {
            Some(anchor) => format!("{}:{}|{}", self.line_number, anchor, self.content),
            None => format!("{}|{}", self.line_number, self.content),
        }
    }

    fn render(&self) -> String {
        match self.kind {
            DiffPreviewKind::Context => format!("  {}", self.render_hashline()),
            DiffPreviewKind::Added => format!("+ {}", self.render_hashline()),
            DiffPreviewKind::Removed => format!("- {}", self.render_hashline()),
        }
    }
}

fn build_anchored_lines(content: &str) -> Vec<AnchoredLine> {
    let mut prefix_state = initial_prefix_hash_state();

    content
        .lines()
        .enumerate()
        .map(|(idx, line)| AnchoredLine {
            line_number: idx + 1,
            content: line.to_string(),
            anchor: compute_anchor(line, &mut prefix_state),
        })
        .collect()
}

fn push_context_line(lines: &mut Vec<DiffPreviewLine>, line: &AnchoredLine) {
    lines.push(DiffPreviewLine {
        kind: DiffPreviewKind::Context,
        content: line.content.clone(),
        line_number: line.line_number,
        anchor: Some(line.anchor.clone()),
    });
}

fn push_added_line(lines: &mut Vec<DiffPreviewLine>, line: &AnchoredLine) {
    lines.push(DiffPreviewLine {
        kind: DiffPreviewKind::Added,
        content: line.content.clone(),
        line_number: line.line_number,
        anchor: Some(line.anchor.clone()),
    });
}

fn push_removed_line(lines: &mut Vec<DiffPreviewLine>, line: &AnchoredLine) {
    lines.push(DiffPreviewLine {
        kind: DiffPreviewKind::Removed,
        content: line.content.clone(),
        line_number: line.line_number,
        anchor: Some(line.anchor.clone()),
    });
}

fn build_preview_lines(
    batch: &PreparedFileEdit,
    original_lines: &[AnchoredLine],
    new_lines: &[AnchoredLine],
) -> Vec<DiffPreviewLine> {
    let mut preview_lines = Vec::new();
    let mut sorted_edits = batch.edits.clone();
    sorted_edits.sort_by_key(|edit| edit.start_line);

    let mut current_old_idx = 0usize;
    let mut current_new_idx = 0usize;
    let mut line_delta = 0i64;

    for edit in sorted_edits {
        let original_line_count = match edit.action {
            EditAction::InsertAfter => 0usize,
            EditAction::Delete | EditAction::Replace => edit.end_line - edit.start_line + 1,
        };
        let replacement_line_count = edit.replacement_line_count();
        let old_start_idx = match edit.action {
            EditAction::InsertAfter => edit.start_line,
            EditAction::Delete | EditAction::Replace => edit.start_line.saturating_sub(1),
        }
        .min(original_lines.len());
        let old_end_idx = match edit.action {
            EditAction::InsertAfter => old_start_idx,
            EditAction::Delete | EditAction::Replace => edit.end_line.min(original_lines.len()),
        };
        let mapped_new_start = match edit.action {
            EditAction::InsertAfter => edit.start_line as i64 + line_delta,
            EditAction::Delete | EditAction::Replace => edit.start_line as i64 - 1 + line_delta,
        };
        let new_start_idx = if mapped_new_start <= 0 {
            0
        } else {
            mapped_new_start as usize
        }
        .min(new_lines.len());
        let new_end_idx = (new_start_idx + replacement_line_count).min(new_lines.len());

        while current_old_idx < old_start_idx && current_new_idx < new_start_idx {
            push_context_line(&mut preview_lines, &new_lines[current_new_idx]);
            current_old_idx += 1;
            current_new_idx += 1;
        }

        let original_slice = &original_lines[old_start_idx..old_end_idx];
        let new_slice = &new_lines[new_start_idx..new_end_idx];
        let slices_match = original_slice.len() == new_slice.len()
            && original_slice
                .iter()
                .zip(new_slice.iter())
                .all(|(original_line, new_line)| original_line.content == new_line.content);

        if slices_match {
            for new_line in new_slice {
                push_context_line(&mut preview_lines, new_line);
            }
        } else {
            for original_line in original_slice {
                push_removed_line(&mut preview_lines, original_line);
            }
            for new_line in new_slice {
                push_added_line(&mut preview_lines, new_line);
            }
        }

        current_old_idx = old_end_idx;
        current_new_idx = new_end_idx;
        line_delta += replacement_line_count as i64 - original_line_count as i64;
    }

    while current_old_idx < original_lines.len() && current_new_idx < new_lines.len() {
        push_context_line(&mut preview_lines, &new_lines[current_new_idx]);
        current_old_idx += 1;
        current_new_idx += 1;
    }

    preview_lines
}

fn build_preview_ranges(preview_lines: &[DiffPreviewLine]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();

    for changed_idx in preview_lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| (line.kind != DiffPreviewKind::Context).then_some(idx))
    {
        let start = changed_idx.saturating_sub(DIFF_CONTEXT_LINES);
        let end = (changed_idx + DIFF_CONTEXT_LINES + 1).min(preview_lines.len());

        if let Some((_, last_end)) = ranges.last_mut() {
            if start <= *last_end {
                *last_end = (*last_end).max(end);
                continue;
            }
        }

        ranges.push((start, end));
    }

    ranges
}

/// Generate a git-style diff preview with anchor annotations for changed lines.
fn build_diff_with_anchors(batch: &PreparedFileEdit) -> String {
    let original_lines = build_anchored_lines(&batch.original_content);
    let new_lines = build_anchored_lines(&batch.new_content);
    let preview_lines = build_preview_lines(batch, &original_lines, &new_lines);
    let preview_ranges = build_preview_ranges(&preview_lines);

    if preview_ranges.is_empty() {
        return "  (no line-level changes to preview)".to_string();
    }

    let total_selected_lines: usize = preview_ranges
        .iter()
        .map(|(start, end)| end.saturating_sub(*start))
        .sum();
    let mut rendered_lines: Vec<(String, bool)> = Vec::new();
    let mut emitted_lines = 0usize;
    let mut emitted_selected_lines = 0usize;
    let mut previous_end = 0usize;
    let mut truncated = false;

    'ranges: for (range_index, (start, end)) in preview_ranges.iter().enumerate() {
        if range_index > 0 {
            let omitted_count = start.saturating_sub(previous_end);
            if omitted_count > 0 {
                if emitted_lines >= MAX_DIFF_PREVIEW_LINES {
                    truncated = true;
                    break;
                }

                rendered_lines.push((
                    format!("  ... {} unchanged line(s) omitted", omitted_count),
                    false,
                ));
                emitted_lines += 1;
            }
        }

        for preview_line in &preview_lines[*start..*end] {
            if emitted_lines >= MAX_DIFF_PREVIEW_LINES {
                truncated = true;
                break 'ranges;
            }

            rendered_lines.push((preview_line.render(), true));
            emitted_lines += 1;
            emitted_selected_lines += 1;
        }

        previous_end = *end;
    }

    if truncated {
        let mut remaining_selected_lines =
            total_selected_lines.saturating_sub(emitted_selected_lines);
        if remaining_selected_lines > 0 {
            if rendered_lines.len() >= MAX_DIFF_PREVIEW_LINES {
                if let Some((_, was_selected_line)) = rendered_lines.pop() {
                    if was_selected_line {
                        remaining_selected_lines += 1;
                    }
                }
            }
            rendered_lines.push((
                format!(
                    "  ... {} more diff line(s) omitted",
                    remaining_selected_lines
                ),
                false,
            ));
        }
    }

    rendered_lines
        .into_iter()
        .map(|(line, _)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn build_edit_files_success(prepared_batches: &[PreparedFileEdit]) -> MCPResult {
    let mut file_sections = Vec::new();
    let total_edits: usize = prepared_batches.iter().map(|batch| batch.edits.len()).sum();
    let has_new_anchors = prepared_batches
        .iter()
        .any(|batch| !batch.new_hash_sections.is_empty());

    for batch in prepared_batches {
        let edit_summary = batch
            .edits
            .iter()
            .map(|edit| match edit.action {
                EditAction::InsertAfter => format!(
                    "  Insert after line {}: {} line(s)",
                    edit.start_line,
                    edit.replacement_line_count()
                ),
                EditAction::Delete => {
                    format!("  Delete lines {}-{}", edit.start_line, edit.end_line)
                }
                EditAction::Replace => format!(
                    "  Replace lines {}-{}: {} line(s)",
                    edit.start_line,
                    edit.end_line,
                    edit.replacement_line_count()
                ),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let diff_summary = format!(
            "{} lines in, {} lines out",
            batch.original_line_count,
            batch.new_content.lines().count()
        );
        let anchors_section = if batch.new_hash_sections.is_empty() {
            "\n\nAnchor refresh: no new anchors were generated because these edits only removed existing lines.".to_string()
        } else {
            format!(
                "\n\nNew anchors:\n```\n{}\n```",
                batch.new_hash_sections.join("\n...\n")
            )
        };
        let diff_section = build_diff_with_anchors(batch);

        file_sections.push(format!(
            "File: '{}'\nChanges:\n{}\nSummary: {}\n\nDiff:\n```diff\n{}\n```{}",
            batch.path, edit_summary, diff_summary, diff_section, anchors_section
        ));
    }

    let next_actions = if has_new_anchors {
        vec![
            "Anchors above are current for the edited ranges — reuse them directly with editFiles for follow-up edits in those same ranges".to_string(),
            "Use readFile only when you need broader context, untouched lines, or fresh anchors outside the ranges shown above".to_string(),
        ]
    } else {
        vec![
            "No new anchors were generated because these edits only removed existing lines"
                .to_string(),
            "Use readFile when you need fresh anchors after the deletion or broader context"
                .to_string(),
        ]
    };

    let hint = SuccessHint::new(
        format!(
            "Applied {} edit(s) across {} file(s)\n\n{}",
            total_edits,
            prepared_batches.len(),
            file_sections.join("\n\n")
        ),
        next_actions,
    );

    hint.to_mcp_result_with_data(Some(json!({
        "file_count": prepared_batches.len(),
        "edit_count": total_edits,
        "files": prepared_batches
            .iter()
            .map(|batch| json!({
                "path": batch.path,
                "edit_count": batch.edits.len(),
                "line_count_before": batch.original_line_count,
                "line_count_after": batch.new_content.lines().count(),
            }))
            .collect::<Vec<_>>()
    })))
}
