use super::types::{EditAction, PreparedFileEdit};
use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::types::MCPResult;
use serde_json::json;

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

        file_sections.push(format!(
            "File: '{}'\nChanges:\n{}\nSummary: {}{}",
            batch.path, edit_summary, diff_summary, anchors_section
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
