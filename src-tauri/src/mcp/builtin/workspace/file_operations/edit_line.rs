use super::super::tools::file_tools::create_edit_files_input_schema;
use super::super::WorkspaceServer;
use super::utils::{
    compute_line_hash, format_hashline, format_prefix_hash, initial_prefix_hash_state,
    parse_anchor, read_file_as_string, update_prefix_hash_state,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// A single edit operation.
#[derive(Debug, Clone)]
struct LineEdit {
    start_line: usize, // 1-based. 0 is allowed for INSERT_AFTER at top.
    end_line: usize,   // 1-based, inclusive.
    new_value: String,
    start_anchor: Option<String>,
    end_anchor: Option<String>,
    action: EditAction,
}

#[derive(Debug, Clone)]
struct ParsedEdit {
    path: String,
    edit: LineEdit,
}

#[derive(Debug, Clone)]
struct PreparedFileEdit {
    path: String,
    edits: Vec<LineEdit>,
    original_content: String,
    new_content: String,
    original_line_count: usize,
    new_hash_sections: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditAction {
    Replace,
    InsertAfter,
    Delete,
}

fn requires_existing_line_anchor(edit: &LineEdit) -> bool {
    !(edit.action == EditAction::InsertAfter && edit.start_line == 0)
}

fn requires_end_hash(edit: &LineEdit) -> bool {
    matches!(edit.action, EditAction::Replace | EditAction::Delete)
        && edit.end_line > edit.start_line
}

fn normalize_edit_op_value(value: &str) -> String {
    match value {
        "replace" | "REPLACE" => "replace".to_string(),
        "insert_after" | "INSERT_AFTER" => "insert_after".to_string(),
        "delete" | "DELETE" => "delete".to_string(),
        other => other.to_string(),
    }
}

fn infer_legacy_edit_op(edit_obj: &Map<String, Value>) -> Option<&'static str> {
    if edit_obj
        .get("insertAfter")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        Some("insert_after")
    } else if let Some(v) = edit_obj.get("new_value") {
        if v.as_str() == Some("") {
            Some("delete")
        } else {
            Some("replace")
        }
    } else {
        None
    }
}

static EDIT_FILES_SCHEMA_JSON: Lazy<Result<Value, String>> = Lazy::new(|| {
    serde_json::to_value(create_edit_files_input_schema())
        .map_err(|error| format!("Failed to serialize editFiles schema: {error}"))
});

static EDIT_FILES_VALIDATOR: Lazy<Result<jsonschema::Validator, String>> = Lazy::new(|| {
    let schema_json = EDIT_FILES_SCHEMA_JSON
        .as_ref()
        .map_err(|error| error.clone())?;
    jsonschema::validator_for(schema_json)
        .map_err(|error| format!("Failed to build editFiles validator: {error}"))
});

fn canonicalize_edit_object(edit: &Value) -> Value {
    let Some(edit_obj) = edit.as_object() else {
        return edit.clone();
    };

    let mut canonical = Map::new();
    let inferred_legacy_op = infer_legacy_edit_op(edit_obj);

    for (key, value) in edit_obj {
        match key.as_str() {
            "op" => {
                if value.is_null() {
                    continue;
                }
                if let Some(op) = value.as_str() {
                    canonical.insert("op".to_string(), Value::String(normalize_edit_op_value(op)));
                } else {
                    canonical.insert("op".to_string(), value.clone());
                }
            }
            "action" => {
                if value.is_null() {
                    continue;
                }
                if !canonical.contains_key("op") {
                    if let Some(op) = value.as_str() {
                        canonical
                            .insert("op".to_string(), Value::String(normalize_edit_op_value(op)));
                    } else {
                        canonical.insert("op".to_string(), value.clone());
                    }
                }
            }
            "line" | "startLine" | "afterLine" => {
                if !value.is_null() {
                    canonical.insert("startLine".to_string(), value.clone());
                }
            }
            "endLine" => {
                if !value.is_null() {
                    canonical.insert("endLine".to_string(), value.clone());
                }
            }
            "anchor" | "startAnchor" => {
                if !value.is_null() {
                    canonical.insert("startAnchor".to_string(), value.clone());
                }
            }
            "endAnchor" => {
                if !value.is_null() {
                    canonical.insert("endAnchor".to_string(), value.clone());
                }
            }
            "new_value" | "content" => {
                if !value.is_null() {
                    if key == "new_value" && inferred_legacy_op == Some("delete") {
                        continue;
                    }
                    canonical.insert("content".to_string(), value.clone());
                }
            }
            "insertAfter" => {}
            _ => {
                canonical.insert(key.clone(), value.clone());
            }
        }
    }

    if !canonical.contains_key("op") {
        if let Some(inferred_op) = inferred_legacy_op {
            canonical.insert("op".to_string(), Value::String(inferred_op.to_string()));
        }
    }

    Value::Object(canonical)
}

fn canonicalize_edit_file_args(args: &Value) -> Value {
    let Some(args_obj) = args.as_object() else {
        return args.clone();
    };

    let mut canonical = Map::new();

    for (key, value) in args_obj {
        if key == "edits" {
            if let Some(edits) = value.as_array() {
                canonical.insert(
                    "edits".to_string(),
                    Value::Array(edits.iter().map(canonicalize_edit_object).collect()),
                );
            } else {
                canonical.insert("edits".to_string(), value.clone());
            }
        } else {
            canonical.insert(key.clone(), value.clone());
        }
    }

    Value::Object(canonical)
}

fn canonicalize_edit_files_args(args: &Value) -> Value {
    let Some(args_obj) = args.as_object() else {
        return args.clone();
    };

    let mut canonical = Map::new();

    for (key, value) in args_obj {
        if key == "edits" {
            if let Some(edits) = value.as_array() {
                canonical.insert(
                    "edits".to_string(),
                    Value::Array(edits.iter().map(canonicalize_edit_object).collect()),
                );
            } else {
                canonical.insert("edits".to_string(), value.clone());
            }
        } else {
            canonical.insert(key.clone(), value.clone());
        }
    }

    Value::Object(canonical)
}

fn canonicalize_legacy_edit_file_args_as_edit_files(args: &Value) -> Value {
    let canonical = canonicalize_edit_file_args(args);
    let root_path = canonical.get("path").cloned();
    let edits = canonical
        .get("edits")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let Some(edit_obj) = item.as_object() else {
                        return item.clone();
                    };

                    let mut with_path = edit_obj.clone();
                    if !with_path.contains_key("path") {
                        if let Some(path) = &root_path {
                            if !path.is_null() {
                                with_path.insert("path".to_string(), path.clone());
                            }
                        }
                    }
                    Value::Object(with_path)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({ "edits": edits })
}

fn validate_edit_files_arguments(args: &Value) -> Result<(), String> {
    let validator = EDIT_FILES_VALIDATOR
        .as_ref()
        .map_err(|error| error.clone())?;

    let errors = validator
        .iter_errors(args)
        .take(3)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn parse_line_edit(edit_obj: &Map<String, Value>, idx: usize) -> Result<LineEdit, MCPResult> {
    let op_str = edit_obj.get("op").and_then(|v| v.as_str());
    let action = match op_str {
        Some("replace") => EditAction::Replace,
        Some("insert_after") => EditAction::InsertAfter,
        Some("delete") => EditAction::Delete,
        Some(other) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!("Edit at index {}: invalid op '{}'", idx, other),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Supported ops: replace, insert_after, delete".to_string()
            ])
            .to_mcp_result());
        }
        None => {
            return Err(missing_param_error("op", ToolGroup::Workspace));
        }
    };

    let start_line = match edit_obj.get("startLine").and_then(|v| v.as_u64()) {
        Some(n) => n as usize,
        _ => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Edit at index {}: 'startLine' field is required and must be an integer",
                    idx
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide startLine as an integer (e.g., \"startLine\": 10)".to_string(),
                "Use startLine: 0 ONLY with op='insert_after' to insert at top".to_string(),
            ])
            .to_mcp_result());
        }
    };

    if start_line == 0 && action != EditAction::InsertAfter {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Edit at index {}: startLine 0 is only valid for op 'insert_after'",
                idx
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "To insert at the beginning, use startLine: 0 and op: 'insert_after'".to_string(),
        ])
        .to_mcp_result());
    }

    let has_end_line = edit_obj.get("endLine").is_some();
    let end_line = match edit_obj.get("endLine").and_then(|v| v.as_u64()) {
        Some(n) if n >= start_line as u64 => n as usize,
        Some(n) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Edit at index {}: 'endLine' ({}) must be ≥ 'startLine' ({})",
                    idx, n, start_line
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec!["endLine must be ≥ startLine".to_string()])
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

    if action == EditAction::InsertAfter && has_end_line {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Edit at index {}: 'endLine' cannot be used with op 'insert_after'",
                idx
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "insert_after only targets one line (or startLine 0). Remove 'endLine'.".to_string(),
        ])
        .to_mcp_result());
    }

    let new_value = match edit_obj.get("content").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            if action == EditAction::Delete {
                String::new()
            } else {
                return Err(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Edit at index {}: 'content' is required for replace and insert_after",
                        idx
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide replacement/insertion content as a string".to_string()
                ])
                .to_mcp_result());
            }
        }
    };

    let start_anchor = edit_obj
        .get("startAnchor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let end_anchor = edit_obj
        .get("endAnchor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let requires_anchor = !(action == EditAction::InsertAfter && start_line == 0);
    if requires_anchor && start_anchor.is_none() {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Edit at index {} targets existing content and requires 'startAnchor'",
                idx
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run readFile(showLineAnchors=true) or search(showLineAnchors=true) first"
                .to_string(),
            "Copy only the 6-character start-line anchor from the form N:anchor|content (the part between ':' and '|')".to_string(),
            "If the edit also uses endLine for a range, copy only the 6-character endAnchor from the exact final line"
                .to_string(),
            "Only insert_after with startLine: 0 may omit anchors".to_string(),
        ])
        .to_mcp_result());
    }

    if matches!(action, EditAction::Replace | EditAction::Delete)
        && end_line > start_line
        && end_anchor.is_none()
    {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Edit at index {} uses 'endLine' and requires 'endAnchor' for the exact end line",
                idx
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run readFile(showLineAnchors=true) or search(showLineAnchors=true) first".to_string(),
            "Copy the start-line anchor as 'startAnchor' and the final-line anchor as 'endAnchor'"
                .to_string(),
            "Only multi-line replace and delete need 'endAnchor'".to_string(),
        ])
        .to_mcp_result());
    }

    Ok(LineEdit {
        start_line,
        end_line,
        new_value,
        start_anchor,
        end_anchor,
        action,
    })
}

fn validate_edits_do_not_overlap(edits: &[LineEdit]) -> Result<(), MCPResult> {
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

/// Pure apply function — applies sorted edits (high → low) to a slice of lines.
fn apply_edits(orig_lines: &[&str], edits: &[LineEdit]) -> Vec<String> {
    let mut modified: Vec<String> = orig_lines.iter().map(|&s| s.to_string()).collect();
    let mut sorted = edits.to_vec();
    // Sort high -> low. For InsertAfter at line 0, it stays at the bottom of the sort (lowest index).
    sorted.sort_by_key(|edit| std::cmp::Reverse(edit.start_line));

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

fn build_new_hash_sections(edits: &[LineEdit], new_content: &str) -> Vec<String> {
    let new_content_lines: Vec<&str> = new_content.lines().collect();
    let mut full_prefix_state = initial_prefix_hash_state();
    let full_hashlines: Vec<String> = new_content_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| format_hashline(idx + 1, line, &mut full_prefix_state))
        .collect();
    let mut sorted_asc = edits.to_vec();
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
        let section: Vec<String> = full_hashlines[start_in_new..end_in_new].to_vec();
        new_hash_sections.push(section.join("\n"));

        let orig_len = if edit.action == EditAction::InsertAfter {
            0
        } else {
            (edit.end_line - edit.start_line + 1) as i64
        };
        line_delta += n_lines as i64 - orig_len;
    }

    new_hash_sections
}

impl WorkspaceServer {
    pub async fn handle_replace_lines(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Batch mode: edits array provided directly.
        if let Some(edits) = args.get("edits").and_then(|v| v.as_array()) {
            let tagged: Vec<Value> = edits
                .iter()
                .map(|e| {
                    let mut item = e.clone();
                    item["action"] = json!("REPLACE");
                    item
                })
                .collect();
            let delegated = json!({
                "path": args.get("path").cloned().unwrap_or(Value::Null),
                "edits": tagged,
            });
            return self.handle_edit_file(delegated, session_id).await;
        }

        // Single-edit (flat params) — wrap and delegate.
        let delegated = json!({
            "path": args.get("path").cloned().unwrap_or(Value::Null),
            "edits": [{
                "action": "REPLACE",
                "line": args.get("line").cloned().unwrap_or(Value::Null),
                "endLine": args.get("endLine").cloned().unwrap_or(Value::Null),
                "new_value": args.get("new_value").cloned().unwrap_or(Value::Null),
                "anchor": args.get("anchor").cloned().unwrap_or(Value::Null),
                "endAnchor": args.get("endAnchor").cloned().unwrap_or(Value::Null)
            }]
        });

        self.handle_edit_file(delegated, session_id).await
    }

    pub async fn handle_insert_after_line(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Batch mode: edits array provided directly.
        if let Some(edits) = args.get("edits").and_then(|v| v.as_array()) {
            let tagged: Vec<Value> = edits
                .iter()
                .map(|e| {
                    let mut item = json!({
                        "action": "INSERT_AFTER",
                        "line": e.get("afterLine").cloned().unwrap_or(Value::Null),
                        "new_value": e.get("new_value").cloned().unwrap_or(Value::Null),
                        "anchor": e.get("anchor").cloned().unwrap_or(Value::Null),
                    });
                    // Preserve any extra fields the caller may have included.
                    if let (Some(obj), Some(src)) = (item.as_object_mut(), e.as_object()) {
                        for (k, v) in src {
                            obj.entry(k).or_insert_with(|| v.clone());
                        }
                    }
                    item
                })
                .collect();
            let delegated = json!({
                "path": args.get("path").cloned().unwrap_or(Value::Null),
                "edits": tagged,
            });
            return self.handle_edit_file(delegated, session_id).await;
        }

        // Single-edit (flat params) — wrap and delegate.
        let delegated = json!({
            "path": args.get("path").cloned().unwrap_or(Value::Null),
            "edits": [{
                "action": "INSERT_AFTER",
                "line": args.get("afterLine").cloned().unwrap_or(Value::Null),
                "new_value": args.get("new_value").cloned().unwrap_or(Value::Null),
                "anchor": args.get("anchor").cloned().unwrap_or(Value::Null)
            }]
        });

        self.handle_edit_file(delegated, session_id).await
    }

    pub async fn handle_delete_lines(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Batch mode: edits array provided directly.
        if let Some(edits) = args.get("edits").and_then(|v| v.as_array()) {
            let tagged: Vec<Value> = edits
                .iter()
                .map(|e| {
                    let mut item = e.clone();
                    item["action"] = json!("DELETE");
                    item
                })
                .collect();
            let delegated = json!({
                "path": args.get("path").cloned().unwrap_or(Value::Null),
                "edits": tagged,
            });
            return self.handle_edit_file(delegated, session_id).await;
        }

        // Single-edit (flat params) — wrap and delegate.
        let delegated = json!({
            "path": args.get("path").cloned().unwrap_or(Value::Null),
            "edits": [{
                "action": "DELETE",
                "line": args.get("line").cloned().unwrap_or(Value::Null),
                "endLine": args.get("endLine").cloned().unwrap_or(Value::Null),
                "anchor": args.get("anchor").cloned().unwrap_or(Value::Null),
                "endAnchor": args.get("endAnchor").cloned().unwrap_or(Value::Null)
            }]
        });

        self.handle_edit_file(delegated, session_id).await
    }

    async fn prepare_file_edit_batch(
        &self,
        path_str: &str,
        edits: Vec<LineEdit>,
        session_id: Option<String>,
    ) -> Result<PreparedFileEdit, MCPResult> {
        validate_edits_do_not_overlap(&edits)?;

        let safe_path = match self.validate_path_with_error_for_write(path_str, session_id.clone())
        {
            Ok(path) => path,
            Err(error) => {
                return Err(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed for '{}': {}", path_str, error),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Use paths relative to workspace root".to_string(),
                    "Use listDirectory to inspect valid target paths".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(error) => {
                return Err(guided_error(
                    ErrorCategory::OperationFailed,
                    error,
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
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
            if !requires_existing_line_anchor(edit) {
                continue;
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

            if matches!(edit.action, EditAction::Replace | EditAction::Delete)
                && edit.end_line > line_count
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
                        "Copy only the 6-character anchor from the returned N:anchor|content line (the part between ':' and '|')"
                            .to_string(),
                    ])
                    .to_mcp_result());
                }
            };
            let actual = orig_lines[edit.start_line - 1];
            let actual_hash = compute_line_hash(actual);
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

            if requires_end_hash(edit) {
                let expected_end_anchor = edit
                    .end_anchor
                    .as_ref()
                    .expect("end_anchor required for multi-line replace/delete");
                let (expected_end_hash, expected_end_prefix) = match parse_anchor(
                    expected_end_anchor,
                ) {
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
                            "Copy only the 6-character endAnchor from the returned N:anchor|content line (the part between ':' and '|')".to_string(),
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
                        "Run readFile with showLineAnchors=true to get the current end anchor"
                            .to_string(),
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
                        "Run readFile with showLineAnchors=true to get the current end anchor"
                            .to_string(),
                        "Rebuild the edit with an updated endAnchor".to_string(),
                    ])
                    .to_mcp_result());
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

        Ok(PreparedFileEdit {
            path: path_str.to_string(),
            edits: edits.clone(),
            original_content,
            new_content: new_content.clone(),
            original_line_count: line_count,
            new_hash_sections: build_new_hash_sections(&edits, &new_content),
        })
    }

    fn build_edit_files_success(prepared_batches: &[PreparedFileEdit]) -> MCPResult {
        let mut file_sections = Vec::new();
        let total_edits: usize = prepared_batches.iter().map(|batch| batch.edits.len()).sum();

        for batch in prepared_batches {
            let edit_summary = batch
                .edits
                .iter()
                .map(|edit| match edit.action {
                    EditAction::InsertAfter => format!(
                        "  Insert after line {}: {} line(s)",
                        edit.start_line,
                        edit.new_value.lines().count()
                    ),
                    EditAction::Delete => {
                        format!("  Delete lines {}-{}", edit.start_line, edit.end_line)
                    }
                    EditAction::Replace => format!(
                        "  Replace lines {}-{}: {} line(s)",
                        edit.start_line,
                        edit.end_line,
                        edit.new_value.lines().count()
                    ),
                })
                .collect::<Vec<_>>()
                .join("\n");
            let diff_summary = format!(
                "{} lines in, {} lines out",
                batch.new_content.lines().count(),
                batch.original_line_count
            );
            let anchors = if batch.new_hash_sections.is_empty() {
                "(No new lines were created by these edits)".to_string()
            } else {
                batch.new_hash_sections.join("\n...\n")
            };

            file_sections.push(format!(
                "File: '{}'\nChanges:\n{}\nSummary: {}\n\nNew anchors:\n```\n{}\n```",
                batch.path, edit_summary, diff_summary, anchors
            ));
        }

        let hint = SuccessHint::new(
            format!(
                "Applied {} edit(s) across {} file(s)\n\n{}",
                total_edits,
                prepared_batches.len(),
                file_sections.join("\n\n")
            ),
            vec![
                "Anchors above are current — reuse them with editFiles per file".to_string(),
                "Use readFile only if you need broader context beyond the edited ranges"
                    .to_string(),
            ],
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

    pub async fn handle_edit_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let canonical_args = canonicalize_edit_files_args(&args);

        if let Err(validation_error) = validate_edit_files_arguments(&canonical_args) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "editFiles arguments do not match the declared schema: {validation_error}"
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Replace: [{\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"text\"}]".to_string(),
                "Insert top: [{\"path\": \"src/a.ts\", \"op\": \"insert_after\", \"startLine\": 0, \"content\": \"header\"}]".to_string(),
                "Delete range: [{\"path\": \"src/b.ts\", \"op\": \"delete\", \"startLine\": 10, \"endLine\": 15, \"startAnchor\": \"a31f2c\", \"endAnchor\": \"b47aa1\"}]".to_string(),
                "Use readFile(showLineAnchors=true) first to get anchor values".to_string(),
            ])
            .to_mcp_result());
        }

        let edits_array = match canonical_args.get("edits").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => {
                return Ok(guided_error(
                    ErrorCategory::MissingRequiredParam,
                    "Parameter 'edits' is required and must be an array",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Replace: [{\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"text\"}]".to_string(),
                    "Insert-top: [{\"path\": \"src/a.ts\", \"op\": \"insert_after\", \"startLine\": 0, \"content\": \"header\"}]".to_string(),
                    "Delete range: [{\"path\": \"src/b.ts\", \"op\": \"delete\", \"startLine\": 10, \"endLine\": 15, \"startAnchor\": \"a31f2c\", \"endAnchor\": \"b47aa1\"}]".to_string(),
                    "Use readFile(showLineAnchors=true) to get anchor values first".to_string(),
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

        let mut parsed_edits = Vec::with_capacity(edits_array.len());
        for (idx, edit_value) in edits_array.iter().enumerate() {
            let edit_obj = match edit_value.as_object() {
                Some(obj) => obj,
                None => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {} must be an object", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Single-line: {\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"text\"}".to_string(),
                        "Range: {\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"endLine\": 15, \"startAnchor\": \"a31f2c\", \"endAnchor\": \"b47aa1\", \"content\": \"...\"}".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            let path = match edit_obj.get("path").and_then(|v| v.as_str()) {
                Some(path) if !path.trim().is_empty() => path.trim().to_string(),
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Edit at index {}: 'path' field is required and must be a non-empty string",
                            idx
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Each edit item must include its own path".to_string(),
                        "Example: {\"path\": \"src/main.rs\", ...}".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            let edit = match parse_line_edit(edit_obj, idx) {
                Ok(edit) => edit,
                Err(result) => return Ok(result),
            };

            parsed_edits.push(ParsedEdit { path, edit });
        }

        let mut grouped: BTreeMap<String, Vec<LineEdit>> = BTreeMap::new();
        for parsed in parsed_edits {
            grouped.entry(parsed.path).or_default().push(parsed.edit);
        }

        let mut prepared_batches = Vec::with_capacity(grouped.len());
        for (path, edits) in grouped {
            let prepared = match self
                .prepare_file_edit_batch(&path, edits, session_id.clone())
                .await
            {
                Ok(batch) => batch,
                Err(result) => return Ok(result),
            };
            prepared_batches.push(prepared);
        }

        let file_manager = self.get_file_manager(session_id.clone());
        let mut written_paths: Vec<String> = Vec::new();

        for batch in &prepared_batches {
            if let Err(error) = file_manager
                .write_file_string(&batch.path, &batch.new_content)
                .await
            {
                let mut rollback_failures = Vec::new();
                for written_path in written_paths.iter().rev() {
                    if let Some(previous_batch) = prepared_batches
                        .iter()
                        .find(|candidate| &candidate.path == written_path)
                    {
                        if let Err(rollback_error) = file_manager
                            .write_file_string(
                                &previous_batch.path,
                                &previous_batch.original_content,
                            )
                            .await
                        {
                            rollback_failures
                                .push(format!("{} ({})", previous_batch.path, rollback_error));
                        }
                    }
                }

                let rollback_note = if rollback_failures.is_empty() {
                    "Any earlier writes in this request were rolled back.".to_string()
                } else {
                    format!("Rollback failed for: {}", rollback_failures.join(", "))
                };

                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    format!("Failed to write '{}': {}", batch.path, error),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    rollback_note,
                    "Check file permissions and available disk space".to_string(),
                    "Rerun readFile(showLineAnchors=true) before retrying if files may have changed"
                        .to_string(),
                ])
                .to_mcp_result());
            }
            written_paths.push(batch.path.clone());
        }

        self.invalidate_context_cache().await;

        Ok(Self::build_edit_files_success(&prepared_batches))
    }

    pub async fn handle_edit_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let canonical_args = canonicalize_legacy_edit_file_args_as_edit_files(&args);
        self.handle_edit_files(canonical_args, session_id).await
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
            start_anchor: None,
            end_anchor: None,
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
