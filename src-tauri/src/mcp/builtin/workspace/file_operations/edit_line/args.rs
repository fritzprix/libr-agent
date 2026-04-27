use super::super::super::tools::file_tools::create_edit_files_input_schema;
use super::types::{EditAction, LineEdit};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};

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

pub(super) fn canonicalize_edit_files_args(args: &Value) -> Value {
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

pub(super) fn canonicalize_legacy_edit_file_args_as_edit_files(args: &Value) -> Value {
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

pub(super) fn validate_edit_files_arguments(args: &Value) -> Result<(), String> {
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

pub(super) fn parse_line_edit(
    edit_obj: &Map<String, Value>,
    idx: usize,
) -> Result<LineEdit, MCPResult> {
    let start_line = match edit_obj.get("startLine").and_then(|value| value.as_u64()) {
        Some(line) => line as usize,
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
                "Use startLine: 0 to insert at the beginning of the file".to_string(),
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
                format!("Edit at index {}: invalid op '{}'", idx, other),
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

    let has_end_line = edit_obj.get("endLine").is_some();
    let end_line = match edit_obj.get("endLine").and_then(|value| value.as_u64()) {
        Some(line) if line >= start_line as u64 => line as usize,
        Some(line) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Edit at index {}: 'endLine' ({}) must be ≥ 'startLine' ({})",
                    idx, line, start_line
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

    let new_value = match (action, content_value) {
        (EditAction::Delete, None) => String::new(),
        (EditAction::Delete, Some(Value::String(content))) if content.is_empty() => String::new(),
        (EditAction::Delete, Some(Value::String(_))) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Edit at index {}: delete edits must omit 'content' (or set op='replace' if you meant to replace)",
                    idx
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
                format!(
                    "Edit at index {}: 'content' must be a string when provided",
                    idx
                ),
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
