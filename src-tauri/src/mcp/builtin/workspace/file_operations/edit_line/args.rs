use super::super::super::tools::file_tools::create_edit_file_input_schema;
use super::types::{EditAction, LineEdit};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use once_cell::sync::Lazy;
use serde_json::{Map, Value};

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
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        Some("insert_after")
    } else if let Some(value) = edit_obj.get("new_value") {
        if value.as_str() == Some("") {
            Some("delete")
        } else {
            Some("replace")
        }
    } else {
        None
    }
}

static EDIT_FILE_SCHEMA_JSON: Lazy<Result<Value, String>> = Lazy::new(|| {
    serde_json::to_value(create_edit_file_input_schema())
        .map_err(|error| format!("Failed to serialize editFile schema: {error}"))
});

static EDIT_FILE_VALIDATOR: Lazy<Result<jsonschema::Validator, String>> = Lazy::new(|| {
    let schema_json = EDIT_FILE_SCHEMA_JSON
        .as_ref()
        .map_err(|error| error.clone())?;
    jsonschema::validator_for(schema_json)
        .map_err(|error| format!("Failed to build editFile validator: {error}"))
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

    if !canonical.contains_key("startLine") {
        let is_insert_after = canonical
            .get("op")
            .and_then(|value| value.as_str())
            .is_some_and(|op| op == "insert_after");
        let touches_existing_line = canonical.contains_key("startAnchor")
            || canonical.contains_key("endAnchor")
            || canonical.contains_key("endLine");
        let has_non_empty_content = canonical
            .get("content")
            .and_then(|value| value.as_str())
            .is_some_and(|content| !content.is_empty());

        if has_non_empty_content && !is_insert_after && !touches_existing_line {
            canonical.insert("startLine".to_string(), Value::Number(0.into()));
        }
    }

    Value::Object(canonical)
}

pub(super) fn canonicalize_edit_file_args(args: &Value) -> Value {
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

pub(super) fn format_edit_label(edit_obj: &Map<String, Value>, idx: usize) -> String {
    let mut parts = Vec::new();

    if let Some(path) = edit_obj
        .get("path")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        parts.push(format!("path='{}'", path));
    }

    if let Some(op) = edit_obj.get("op").and_then(|value| value.as_str()) {
        parts.push(format!("op='{}'", op));
    }

    if let Some(start_line) = edit_obj.get("startLine").and_then(|value| value.as_u64()) {
        parts.push(format!("startLine={}", start_line));
    }

    if let Some(end_line) = edit_obj.get("endLine").and_then(|value| value.as_u64()) {
        parts.push(format!("endLine={}", end_line));
    }

    if parts.is_empty() {
        format!("Edit at index {}", idx)
    } else {
        format!("Edit at index {} [{}]", idx, parts.join(", "))
    }
}

pub(super) fn validate_edit_file_arguments(args: &Value) -> Result<(), String> {
    let validator = EDIT_FILE_VALIDATOR
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

pub(super) fn parse_line_edit(
    edit_obj: &Map<String, Value>,
    idx: usize,
) -> Result<LineEdit, MCPResult> {
    let edit_label = format_edit_label(edit_obj, idx);
    let start_line = match edit_obj.get("startLine").and_then(|value| value.as_u64()) {
        Some(line) => line as usize,
        _ => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!("{edit_label}: 'startLine' field is required and must be an integer"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide startLine as an integer (e.g., \"startLine\": 10)".to_string(),
                "Existing lines are 1-based: use startLine: 1 for the first line".to_string(),
                "Use startLine: 0 only to prepend at the beginning of the file".to_string(),
            ])
            .to_mcp_result());
        }
    };

    let content_value = edit_obj.get("content");
    let has_content_field = content_value.is_some();
    let content_is_empty_string = content_value
        .and_then(|value| value.as_str())
        .map(|content| content.is_empty())
        .unwrap_or(false);

    let op_str = edit_obj.get("op").and_then(|value| value.as_str());
    let action = match op_str {
        Some("replace") => EditAction::Replace,
        Some("insert_after") => EditAction::InsertAfter,
        Some("delete") => EditAction::Delete,
        Some(other) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!("{edit_label}: invalid op '{}'", other),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Supported ops: replace, insert_after, delete".to_string()
            ])
            .to_mcp_result());
        }
        None => {
            if start_line == 0 {
                EditAction::InsertAfter
            } else if has_content_field && !content_is_empty_string {
                EditAction::Replace
            } else {
                EditAction::Delete
            }
        }
    };

    let action_name = match action {
        EditAction::Replace => "replace",
        EditAction::Delete => "delete",
        EditAction::InsertAfter => "insert_after",
    };

    if start_line == 0 && action != EditAction::InsertAfter {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "{edit_label}: 'startLine' must be >= 1 for '{}' edits",
                action_name
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Use startLine: 0 only with op='insert_after' to prepend before the first line"
                .to_string(),
            "Use startLine >= 1 for replace and delete edits".to_string(),
        ])
        .to_mcp_result());
    }

    let has_end_line = edit_obj.get("endLine").is_some();
    let end_line = match edit_obj.get("endLine").and_then(|value| value.as_u64()) {
        Some(line) if line >= start_line as u64 => line as usize,
        Some(line) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "{edit_label}: 'endLine' ({}) must be ≥ 'startLine' ({})",
                    line, start_line
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

    // Reject unnecessary endLine/endAnchor for single-line replace/delete
    if (action == EditAction::Replace || action == EditAction::Delete)
        && end_line == start_line
        && has_end_line
    {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "{edit_label}: 'endLine' must be omitted for single-line {} edits",
                action_name
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "For single-line replace: {\"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"new text\"}".to_string(),
            "For single-line delete: {\"startLine\": 10, \"startAnchor\": \"a31f2c\"}".to_string(),
            "Use endLine only for multi-line ranges (endLine > startLine)".to_string(),
        ])
        .to_mcp_result());
    }

    if action == EditAction::InsertAfter && has_end_line {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!("{edit_label}: 'endLine' cannot be used with op 'insert_after'"),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "insert_after only targets one line (or startLine 0). Remove 'endLine'.".to_string(),
        ])
        .to_mcp_result());
    }

    let new_value = match (action, content_value) {
        (EditAction::Delete, None) => String::new(),
        (EditAction::Delete, Some(Value::String(content))) if content.is_empty() => String::new(),
        (EditAction::Delete, Some(Value::String(_))) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "{edit_label}: delete edits must omit 'content' (or set op='replace' if you meant to replace)"
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "For deletion, omit 'content' entirely".to_string(),
                "For replacement, include non-empty 'content' and optionally set op='replace'"
                    .to_string(),
            ])
            .to_mcp_result());
        }
        (_, Some(Value::String(content))) => content.to_string(),
        (_, Some(_)) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!("{edit_label}: 'content' must be a string when provided"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide replacement/insertion content as a string".to_string()
            ])
            .to_mcp_result());
        }
        (_, None) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!("{edit_label}: 'content' is required for replace and insert_after"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide replacement/insertion content as a string".to_string()
            ])
            .to_mcp_result());
        }
    };

    let start_anchor = edit_obj
        .get("startAnchor")
        .and_then(|value| value.as_str())
        .map(|anchor| anchor.to_string());
    let end_anchor = edit_obj
        .get("endAnchor")
        .and_then(|value| value.as_str())
        .map(|anchor| anchor.to_string());

    let requires_anchor = !(action == EditAction::InsertAfter && start_line == 0);
    if !requires_anchor && (start_anchor.is_some() || end_anchor.is_some()) {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "{edit_label}: prepend edits with startLine 0 must omit 'startAnchor' and 'endAnchor'"
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "When startLine is 0, the edit inserts before the first line and does not target existing content".to_string(),
            "Remove startAnchor/endAnchor and keep only startLine: 0 plus content".to_string(),
        ])
        .to_mcp_result());
    }
    if requires_anchor && start_anchor.is_none() {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!("{edit_label} targets existing content and requires 'anchor' (or 'startAnchor')"),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run readFile(showLineAnchors=true) or search(showLineAnchors=true) first"
                .to_string(),
            "Copy only the 6-character start-line anchor from the line format N:anchor|content. Example: from '42:a31f2c|let x = 1;', pass only 'a31f2c'.".to_string(),
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
            format!("{edit_label} uses 'endLine' and requires 'endAnchor' for the exact end line"),
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
