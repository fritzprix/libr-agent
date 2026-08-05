use super::super::super::tools::file_tools::{
    create_edit_file_input_schema, create_edit_file_validation_schema,
};
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

/// Parse `start` / `end` values copied from readFile output (`N:anchor`).
///
/// Special case: `"0"` means prepend / top insert and carries no anchor.
fn parse_line_ref(raw: &str, field_name: &str) -> Result<(usize, Option<String>), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "'{field_name}' must be a non-empty \"N:anchor\" string"
        ));
    }
    if trimmed.contains('|') {
        return Err(format!(
            "'{field_name}' must be only the \"N:anchor\" prefix from readFile output \
(e.g. \"42:a31f2c\"), not the trailing '|content'"
        ));
    }
    if trimmed == "0" {
        return Ok((0, None));
    }

    let mut parts = trimmed.splitn(2, ':');
    let line_part = parts.next().unwrap_or("");
    let anchor_part = parts.next();

    let Some(anchor_part) = anchor_part else {
        return Err(format!(
            "'{field_name}' must use \"N:anchor\" format (e.g. \"42:a31f2c\"). \
Copy the prefix before '|' from workspace__readFile(showLineAnchors=true)."
        ));
    };

    let line = line_part.parse::<usize>().map_err(|_| {
        format!(
            "'{field_name}' line number must be an integer (e.g. \"42:a31f2c\"), got '{line_part}'"
        )
    })?;

    let anchor = anchor_part.trim();
    if anchor.is_empty() {
        return Err(format!(
            "'{field_name}' is missing the anchor after ':' (e.g. \"42:a31f2c\")"
        ));
    }
    if anchor.len() != 6 || !anchor.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "'{field_name}' anchor must be the 6-character hex code from readFile \
(e.g. \"42:a31f2c\"), got '{anchor}'"
        ));
    }

    Ok((line, Some(anchor.to_string())))
}

fn expand_line_ref_fields(canonical: &mut Map<String, Value>) -> Result<(), String> {
    if let Some(start_value) = canonical.remove("start") {
        let start_str = start_value
            .as_str()
            .ok_or_else(|| "'start' must be a string (e.g. \"42:a31f2c\")".to_string())?;
        let (line, anchor) = parse_line_ref(start_str, "start")?;
        canonical.insert("startLine".to_string(), json!(line));
        if let Some(anchor) = anchor {
            canonical.insert("startAnchor".to_string(), Value::String(anchor));
        }
    }

    if let Some(end_value) = canonical.remove("end") {
        let end_str = end_value
            .as_str()
            .ok_or_else(|| "'end' must be a string (e.g. \"72:b47aa1\")".to_string())?;
        let (line, anchor) = parse_line_ref(end_str, "end")?;
        if line == 0 {
            return Err(
                "'end' cannot be \"0\"; omit end for single-line edits or use a 1-based range"
                    .to_string(),
            );
        }
        let Some(anchor) = anchor else {
            return Err("'end' must use \"N:anchor\" format (e.g. \"72:b47aa1\")".to_string());
        };
        canonical.insert("endLine".to_string(), json!(line));
        canonical.insert("endAnchor".to_string(), Value::String(anchor));
    }

    Ok(())
}

static EDIT_FILE_DISCOVERY_SCHEMA_JSON: Lazy<Result<Value, String>> = Lazy::new(|| {
    serde_json::to_value(create_edit_file_input_schema()).map_err(|error| {
        format!("Failed to serialize workspace__editFile discovery schema: {error}")
    })
});

static EDIT_FILE_VALIDATION_SCHEMA_JSON: Lazy<Result<Value, String>> = Lazy::new(|| {
    serde_json::to_value(create_edit_file_validation_schema()).map_err(|error| {
        format!("Failed to serialize workspace__editFile validation schema: {error}")
    })
});

static EDIT_FILE_FLAT_VALIDATOR: Lazy<Result<jsonschema::Validator, String>> = Lazy::new(|| {
    let schema_json = EDIT_FILE_DISCOVERY_SCHEMA_JSON
        .as_ref()
        .map_err(|error| error.clone())?;
    jsonschema::validator_for(schema_json)
        .map_err(|error| format!("Failed to build workspace__editFile flat validator: {error}"))
});

static EDIT_FILE_BATCH_VALIDATOR: Lazy<Result<jsonschema::Validator, String>> = Lazy::new(|| {
    let schema_json = EDIT_FILE_VALIDATION_SCHEMA_JSON
        .as_ref()
        .map_err(|error| error.clone())?;
    jsonschema::validator_for(schema_json)
        .map_err(|error| format!("Failed to build workspace__editFile batch validator: {error}"))
});

fn canonicalize_edit_object(edit: &Value) -> Result<Value, String> {
    let Some(edit_obj) = edit.as_object() else {
        return Ok(edit.clone());
    };

    let mut canonical = Map::new();
    let inferred_legacy_op = infer_legacy_edit_op(edit_obj);

    for (key, value) in edit_obj {
        match key.as_str() {
            "path" => {}
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
            "start" | "end" => {
                if !value.is_null() {
                    canonical.insert(key.clone(), value.clone());
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

    expand_line_ref_fields(&mut canonical)?;

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

    Ok(Value::Object(canonical))
}

/// Normalize editFile args into `{ path, edits: [...] }`.
///
/// Flat single-edit calls (`path` + `start`/`content`/…) are wrapped into a
/// one-element `edits` array. Legacy `edits` arrays (hidden dispatch aliases)
/// are preserved.
pub(super) fn canonicalize_edit_file_args(args: &Value) -> Result<Value, String> {
    let Some(args_obj) = args.as_object() else {
        return Ok(args.clone());
    };

    if let Some(edits) = args_obj.get("edits") {
        let mut canonical = Map::new();
        for (key, value) in args_obj {
            if key == "edits" {
                continue;
            }
            canonical.insert(key.clone(), value.clone());
        }

        if let Some(edits_array) = edits.as_array() {
            let mut normalized = Vec::with_capacity(edits_array.len());
            for edit in edits_array {
                normalized.push(canonicalize_edit_object(edit)?);
            }
            canonical.insert("edits".to_string(), Value::Array(normalized));
        } else {
            canonical.insert("edits".to_string(), edits.clone());
        }

        return Ok(Value::Object(canonical));
    }

    // Flat single-edit form advertised by the discovery schema.
    let path = args_obj.get("path").cloned().unwrap_or(Value::Null);
    let edit = canonicalize_edit_object(&Value::Object(args_obj.clone()))?;
    Ok(json!({
        "path": path,
        "edits": [edit],
    }))
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

fn format_schema_errors(validator: &jsonschema::Validator, args: &Value) -> String {
    validator
        .iter_errors(args)
        .take(3)
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn validate_edit_file_arguments(
    original_args: &Value,
    canonical_args: &Value,
) -> Result<(), String> {
    let used_flat_form = original_args
        .as_object()
        .is_some_and(|obj| !obj.contains_key("edits"));

    if used_flat_form {
        let validator = EDIT_FILE_FLAT_VALIDATOR
            .as_ref()
            .map_err(|error| error.clone())?;
        let errors = format_schema_errors(validator, original_args);
        if !errors.is_empty() {
            return Err(errors);
        }
    }

    let validator = EDIT_FILE_BATCH_VALIDATOR
        .as_ref()
        .map_err(|error| error.clone())?;
    let errors = format_schema_errors(validator, canonical_args);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
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
                format!("{edit_label}: 'start' (or startLine) is required"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide start as \"N:anchor\" (e.g. \"start\": \"42:a31f2c\")".to_string(),
                "Copy the \"42:a31f2c\" prefix from workspace__readFile output: 42:a31f2c|content"
                    .to_string(),
                "Existing lines are 1-based; use start: \"0\" only to prepend at the file top"
                    .to_string(),
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
                "{edit_label}: 'start' must be a 1-based \"N:anchor\" for '{}' edits",
                action_name
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Use start: \"0\" only with op='insert_after' (or content-only prepend)".to_string(),
            "Use start: \"N:anchor\" for replace and delete edits".to_string(),
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
                    "{edit_label}: 'end' line ({}) must be ≥ 'start' line ({})",
                    line, start_line
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec!["end must be ≥ start".to_string()])
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
                "{edit_label}: 'end' must be omitted for single-line {} edits",
                action_name
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "For single-line replace: {\"start\": \"10:a31f2c\", \"content\": \"new text\"}"
                .to_string(),
            "For single-line delete: {\"start\": \"10:a31f2c\"}".to_string(),
            "Use end only for multi-line ranges".to_string(),
        ])
        .to_mcp_result());
    }

    if action == EditAction::InsertAfter && has_end_line {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!("{edit_label}: 'end' cannot be used with op 'insert_after'"),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "insert_after only targets one line (or start \"0\"). Remove 'end'.".to_string(),
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
    let (start_anchor, end_anchor) = if !requires_anchor {
        (None, None)
    } else {
        (start_anchor, end_anchor)
    };
    if requires_anchor && start_anchor.is_none() {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!("{edit_label} targets existing content and requires 'anchor' (use start: \"N:anchor\")"),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true) first".to_string(),
            "Copy start as \"N:anchor\" from the line format N:anchor|content \
(e.g. from '42:a31f2c|let x = 1;', pass \"start\": \"42:a31f2c\")."
                .to_string(),
            "For ranges, also pass end: \"M:anchor\" for the final line".to_string(),
            "Only start: \"0\" (prepend) may omit anchors".to_string(),
        ])
        .to_mcp_result());
    }

    if matches!(action, EditAction::Replace | EditAction::Delete)
        && end_line > start_line
        && end_anchor.is_none()
    {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!("{edit_label} uses a range and requires 'end' for the exact end line"),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Run workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true) first".to_string(),
            "Pass start: \"N:anchor\" and end: \"M:anchor\" copied from workspace__readFile output"
                .to_string(),
            "Only multi-line replace and delete need 'end'".to_string(),
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
