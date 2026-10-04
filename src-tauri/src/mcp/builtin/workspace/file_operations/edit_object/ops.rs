//! Typed object mutations applied to `serde_json::Value`.

use super::pointer::{parse_array_index, JsonPointer};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectOpKind {
    Set,
    Delete,
    Append,
    Remove,
}

impl ObjectOpKind {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "set" => Ok(Self::Set),
            "delete" => Ok(Self::Delete),
            "append" => Ok(Self::Append),
            "remove" => Ok(Self::Remove),
            other => Err(format!(
                "Unknown op '{other}'. Allowed: set, delete, append, remove"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Delete => "delete",
            Self::Append => "append",
            Self::Remove => "remove",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectOp {
    pub kind: ObjectOpKind,
    pub path: JsonPointer,
    pub value: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct AppliedOp {
    pub index: usize,
    pub op: String,
    pub path: String,
    pub summary: String,
}

#[derive(Debug)]
pub struct ApplyError {
    pub op_index: usize,
    pub path: String,
    pub op: String,
    pub message: String,
    pub guidance: Vec<String>,
}

pub fn parse_op(raw: &Value, op_index: usize) -> Result<ObjectOp, ApplyError> {
    let op_str = raw
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| shape_error(op_index, "?", "?", "Missing required field 'op'"))?;

    let kind = ObjectOpKind::parse(op_str).map_err(|message| ApplyError {
        op_index,
        path: "?".to_string(),
        op: op_str.to_string(),
        message,
        guidance: vec!["Use one of: set, delete, append, remove".to_string()],
    })?;

    let path_str = raw
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| shape_error(op_index, op_str, "?", "Missing required field 'path'"))?;

    let path = JsonPointer::parse(path_str).map_err(|message| ApplyError {
        op_index,
        path: path_str.to_string(),
        op: op_str.to_string(),
        message,
        guidance: vec![
            "JSON Pointer examples: /scripts/dev, /keywords/0, / for whole document".to_string(),
        ],
    })?;

    if raw.get("index").is_some() {
        return Err(shape_error(
            op_index,
            op_str,
            path_str,
            "Field 'index' is not supported. For arrays use path '/arr/0' with op=set/remove (JSON Pointer to the element).",
        ));
    }

    let has_value = raw.get("value").is_some();
    let value = match kind {
        ObjectOpKind::Set | ObjectOpKind::Append => {
            if !has_value {
                return Err(shape_error(
                    op_index,
                    op_str,
                    path_str,
                    format!("op='{op_str}' requires 'value'"),
                ));
            }
            Some(raw.get("value").cloned().unwrap_or(Value::Null))
        }
        ObjectOpKind::Delete | ObjectOpKind::Remove => {
            if has_value {
                return Err(shape_error(
                    op_index,
                    op_str,
                    path_str,
                    format!("op='{op_str}' must not include 'value'"),
                ));
            }
            None
        }
    };

    if path.is_root() {
        match kind {
            ObjectOpKind::Set => {}
            ObjectOpKind::Delete | ObjectOpKind::Remove => {
                return Err(ApplyError {
                    op_index,
                    path: path_str.to_string(),
                    op: op_str.to_string(),
                    message: format!(
                        "op='{op_str}' cannot target the whole document ('/'). Use op='set' with path '/' to replace it, or target a specific key/index."
                    ),
                    guidance: vec![
                        "Example delete: {\"op\":\"delete\",\"path\":\"/scripts/old\"}".to_string(),
                        "Example remove: {\"op\":\"remove\",\"path\":\"/keywords/0\"}".to_string(),
                        "To replace the entire JSON document: {\"op\":\"set\",\"path\":\"/\",\"value\":{...}}".to_string(),
                    ],
                });
            }
            ObjectOpKind::Append => {
                return Err(ApplyError {
                    op_index,
                    path: path_str.to_string(),
                    op: op_str.to_string(),
                    message: "op='append' requires a path to an existing array (not '/')."
                        .to_string(),
                    guidance: vec![
                        "Example: {\"op\":\"append\",\"path\":\"/keywords\",\"value\":\"x\"}"
                            .to_string(),
                    ],
                });
            }
        }
    }

    Ok(ObjectOp { kind, path, value })
}

fn shape_error(
    op_index: usize,
    op: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ApplyError {
    ApplyError {
        op_index,
        path: path.into(),
        op: op.into(),
        message: message.into(),
        guidance: vec![
            "Each op needs: set/append → path+value; delete → object key path; remove → array element path (/arr/0)"
                .to_string(),
        ],
    }
}

pub fn apply_ops(root: &mut Value, ops: &[ObjectOp]) -> Result<Vec<AppliedOp>, ApplyError> {
    let mut applied = Vec::with_capacity(ops.len());
    for (op_index, op) in ops.iter().enumerate() {
        let summary = apply_one(root, op).map_err(|mut error| {
            error.op_index = op_index;
            error.op = op.kind.as_str().to_string();
            error.path = op.path.display();
            error
        })?;
        applied.push(AppliedOp {
            index: op_index,
            op: op.kind.as_str().to_string(),
            path: op.path.display(),
            summary,
        });
    }
    Ok(applied)
}

fn apply_one(root: &mut Value, op: &ObjectOp) -> Result<String, ApplyError> {
    match op.kind {
        ObjectOpKind::Set => apply_set(root, op),
        ObjectOpKind::Delete => apply_delete(root, op),
        ObjectOpKind::Append => apply_append(root, op),
        ObjectOpKind::Remove => apply_remove(root, op),
    }
}

fn array_index_guidance(len: usize) -> String {
    if len == 0 {
        "Array is currently empty. Use op=append to add elements.".to_string()
    } else {
        format!("Valid indexes are 0..{}", len - 1)
    }
}

fn apply_set(root: &mut Value, op: &ObjectOp) -> Result<String, ApplyError> {
    let value = op
        .value
        .clone()
        .ok_or_else(|| logic_error(op, "internal: set missing value"))?;

    if op.path.is_root() {
        let before = compact_value(root);
        *root = value.clone();
        return Ok(format!(
            "replaced document {before} → {}",
            compact_value(&value)
        ));
    }

    let tokens = op.path.tokens();
    let (last, parent_tokens) = tokens
        .split_last()
        .ok_or_else(|| logic_error(op, "internal: empty tokens for non-root set"))?;

    let parent = resolve_mut(root, parent_tokens, op)?;
    match parent {
        Value::Object(map) => {
            let before = map.get(last).map(compact_value);
            map.insert(last.clone(), value.clone());
            Ok(match before {
                Some(prev) => format!("set {prev} → {}", compact_value(&value)),
                None => format!("created = {}", compact_value(&value)),
            })
        }
        Value::Array(arr) => {
            let index = parse_array_index(last).map_err(|message| ApplyError {
                op_index: 0,
                path: op.path.display(),
                op: op.kind.as_str().to_string(),
                message,
                guidance: vec![
                    "For arrays, the final pointer token must be a decimal index (e.g. /items/0)"
                        .to_string(),
                ],
            })?;
            if index >= arr.len() {
                return Err(ApplyError {
                    op_index: 0,
                    path: op.path.display(),
                    op: op.kind.as_str().to_string(),
                    message: format!(
                        "Array index {index} out of bounds (len={}). set does not create sparse arrays.",
                        arr.len()
                    ),
                    guidance: vec![
                        "Use op=append to add at the end of the array".to_string(),
                        array_index_guidance(arr.len()),
                    ],
                });
            }
            let before = compact_value(&arr[index]);
            arr[index] = value.clone();
            Ok(format!("set [{index}] {before} → {}", compact_value(&value)))
        }
        other => Err(type_mismatch(
            op,
            "object or array (parent)",
            type_name(other),
        )),
    }
}

fn apply_delete(root: &mut Value, op: &ObjectOp) -> Result<String, ApplyError> {
    let tokens = op.path.tokens();
    let (last, parent_tokens) = tokens
        .split_last()
        .ok_or_else(|| logic_error(op, "internal: delete on root"))?;

    let parent = resolve_mut(root, parent_tokens, op)?;
    match parent {
        Value::Object(map) => {
            let removed = map.remove(last).ok_or_else(|| path_missing(op, last))?;
            Ok(format!("deleted {}", compact_value(&removed)))
        }
        Value::Array(_) => Err(ApplyError {
            op_index: 0,
            path: op.path.display(),
            op: op.kind.as_str().to_string(),
            message: "op='delete' cannot remove array elements. Use op='remove' with a pointer to the element (e.g. /keywords/0)."
                .to_string(),
            guidance: vec![
                "Example: {\"op\":\"remove\",\"path\":\"/keywords/0\"}".to_string(),
            ],
        }),
        other => Err(type_mismatch(op, "object", type_name(other))),
    }
}

fn apply_append(root: &mut Value, op: &ObjectOp) -> Result<String, ApplyError> {
    let value = op
        .value
        .clone()
        .ok_or_else(|| logic_error(op, "internal: append missing value"))?;
    let target = resolve_mut(root, op.path.tokens(), op)?;
    match target {
        Value::Array(arr) => {
            arr.push(value.clone());
            Ok(format!(
                "appended {} (len={})",
                compact_value(&value),
                arr.len()
            ))
        }
        other => Err(type_mismatch(op, "array", type_name(other))),
    }
}

fn apply_remove(root: &mut Value, op: &ObjectOp) -> Result<String, ApplyError> {
    // Pointer must address the element: /keywords/0 (same shape as set on arrays).
    let tokens = op.path.tokens();
    let (last, parent_tokens) = tokens
        .split_last()
        .ok_or_else(|| logic_error(op, "internal: remove on root"))?;

    let index = parse_array_index(last).map_err(|message| ApplyError {
        op_index: 0,
        path: op.path.display(),
        op: op.kind.as_str().to_string(),
        message: format!(
            "{message}. op='remove' path must end with an array index (e.g. /keywords/0)."
        ),
        guidance: vec![
            "Example: {\"op\":\"remove\",\"path\":\"/keywords/0\"}".to_string(),
            "To delete an object key use op='delete' instead".to_string(),
        ],
    })?;

    let parent = resolve_mut(root, parent_tokens, op)?;
    match parent {
        Value::Array(arr) => {
            if index >= arr.len() {
                return Err(ApplyError {
                    op_index: 0,
                    path: op.path.display(),
                    op: op.kind.as_str().to_string(),
                    message: format!("Array index {index} out of bounds (len={})", arr.len()),
                    guidance: vec![array_index_guidance(arr.len())],
                });
            }
            let removed = arr.remove(index);
            Ok(format!("removed [{index}] {}", compact_value(&removed)))
        }
        Value::Object(_) => Err(ApplyError {
            op_index: 0,
            path: op.path.display(),
            op: op.kind.as_str().to_string(),
            message: "op='remove' targets array elements. For object keys use op='delete'."
                .to_string(),
            guidance: vec![format!(
                "Example: {{\"op\":\"delete\",\"path\":\"{}\"}}",
                op.path.display()
            )],
        }),
        other => Err(type_mismatch(op, "array (parent)", type_name(other))),
    }
}

fn resolve_mut<'a>(
    root: &'a mut Value,
    tokens: &[String],
    op: &ObjectOp,
) -> Result<&'a mut Value, ApplyError> {
    let mut current = root;
    for token in tokens {
        current = match current {
            Value::Object(map) => map.get_mut(token).ok_or_else(|| path_missing(op, token))?,
            Value::Array(arr) => {
                let index = parse_array_index(token).map_err(|message| ApplyError {
                    op_index: 0,
                    path: op.path.display(),
                    op: op.kind.as_str().to_string(),
                    message,
                    guidance: vec![
                        "When traversing an array, pointer tokens must be decimal indexes"
                            .to_string(),
                    ],
                })?;
                let len = arr.len();
                arr.get_mut(index).ok_or_else(|| ApplyError {
                    op_index: 0,
                    path: op.path.display(),
                    op: op.kind.as_str().to_string(),
                    message: format!(
                        "Path not found: array index {index} out of bounds (len={len})"
                    ),
                    guidance: vec![
                        "Call workspace__readFile to inspect the current JSON structure"
                            .to_string(),
                    ],
                })?
            }
            other => {
                return Err(ApplyError {
                    op_index: 0,
                    path: op.path.display(),
                    op: op.kind.as_str().to_string(),
                    message: format!(
                        "Cannot traverse into {} at token '{token}'",
                        type_name(other)
                    ),
                    guidance: vec![
                        "Call workspace__readFile to inspect the current JSON structure"
                            .to_string(),
                    ],
                });
            }
        };
    }
    Ok(current)
}

fn path_missing(op: &ObjectOp, token: &str) -> ApplyError {
    ApplyError {
        op_index: 0,
        path: op.path.display(),
        op: op.kind.as_str().to_string(),
        message: format!("Path not found: missing key '{token}'"),
        guidance: vec![
            "Call workspace__readFile to inspect the current JSON structure".to_string(),
            "Create parent objects first with op=set before nesting deeper".to_string(),
        ],
    }
}

fn type_mismatch(op: &ObjectOp, expected: &str, actual: &str) -> ApplyError {
    ApplyError {
        op_index: 0,
        path: op.path.display(),
        op: op.kind.as_str().to_string(),
        message: format!("Type mismatch at path: expected {expected}, got {actual}"),
        guidance: vec![
            "Call workspace__readFile to inspect the current JSON structure".to_string(),
            "Choose an op that matches the value type (append/remove for arrays, set/delete for objects)"
                .to_string(),
        ],
    }
}

fn logic_error(op: &ObjectOp, message: &str) -> ApplyError {
    ApplyError {
        op_index: 0,
        path: op.path.display(),
        op: op.kind.as_str().to_string(),
        message: message.to_string(),
        guidance: vec![],
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn compact_value(value: &Value) -> String {
    const MAX_CHARS: usize = 120;
    let rendered = match value {
        Value::Object(map) if map.is_empty() => "{}".to_string(),
        Value::Array(arr) if arr.is_empty() => "[]".to_string(),
        Value::Object(map) => format!("object({} keys)", map.len()),
        Value::Array(arr) => format!("array(len={})", arr.len()),
        other => other.to_string(),
    };
    let mut chars = rendered.chars();
    let truncated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_ops, compact_value, parse_op, ObjectOpKind};
    use serde_json::json;

    fn op(value: serde_json::Value) -> super::ObjectOp {
        parse_op(&value, 0).expect("op should parse")
    }

    #[test]
    fn set_create_and_replace() {
        let mut root = json!({"scripts": {"dev": "old"}});
        let ops = vec![
            op(json!({"op":"set","path":"/scripts/lint","value":"eslint"})),
            op(json!({"op":"set","path":"/scripts/dev","value":"vite"})),
        ];
        apply_ops(&mut root, &ops).unwrap();
        assert_eq!(root["scripts"]["lint"], "eslint");
        assert_eq!(root["scripts"]["dev"], "vite");
    }

    #[test]
    fn root_set_replaces_document() {
        let mut root = json!({"a": 1});
        let ops = vec![op(json!({"op":"set","path":"/","value":{"b":2}}))];
        apply_ops(&mut root, &ops).unwrap();
        assert_eq!(root, json!({"b": 2}));
    }

    #[test]
    fn append_and_remove_via_element_pointer() {
        let mut root = json!({"keywords": ["a", "b"]});
        let ops = vec![
            op(json!({"op":"append","path":"/keywords","value":"c"})),
            op(json!({"op":"remove","path":"/keywords/0"})),
        ];
        apply_ops(&mut root, &ops).unwrap();
        assert_eq!(root["keywords"], json!(["b", "c"]));
    }

    #[test]
    fn remove_nested_array_row_by_pointer() {
        let mut root = json!({"grid": [[1, 2], [3, 4]]});
        let ops = vec![op(json!({"op":"remove","path":"/grid/0"}))];
        apply_ops(&mut root, &ops).unwrap();
        assert_eq!(root["grid"], json!([[3, 4]]));
    }

    #[test]
    fn rejects_legacy_index_field() {
        let err = parse_op(
            &json!({"op":"remove","path":"/keywords","index":0}),
            0,
        )
        .unwrap_err();
        assert!(err.message.contains("index"), "{}", err.message);
    }

    #[test]
    fn empty_array_remove_guidance_does_not_claim_0_dot_dot_0() {
        let mut root = json!({"keywords": []});
        let err = apply_ops(
            &mut root,
            &[op(json!({"op":"remove","path":"/keywords/0"}))],
        )
        .unwrap_err();
        assert!(
            err.guidance.iter().any(|g| g.contains("empty")),
            "{:?}",
            err.guidance
        );
        assert!(
            !err.guidance.iter().any(|g| g.contains("0..0")),
            "{:?}",
            err.guidance
        );
    }

    #[test]
    fn compact_value_truncates_multibyte_without_panic() {
        let long = "가".repeat(80);
        let rendered = compact_value(&json!(long));
        assert!(rendered.ends_with('…'));
        assert!(rendered.chars().count() <= 121);
    }

    #[test]
    fn delete_key() {
        let mut root = json!({"scripts": {"old": "x", "keep": "y"}});
        let ops = vec![op(json!({"op":"delete","path":"/scripts/old"}))];
        apply_ops(&mut root, &ops).unwrap();
        assert_eq!(root["scripts"], json!({"keep": "y"}));
    }

    #[test]
    fn atomic_failure_leaves_value_unchanged_when_caller_clones() {
        let original = json!({"arr": [1]});
        let mut working = original.clone();
        let ops = vec![
            op(json!({"op":"append","path":"/arr","value":2})),
            op(json!({"op":"append","path":"/missing","value":3})),
        ];
        assert!(apply_ops(&mut working, &ops).is_err());
        assert_ne!(working, original);
        assert_eq!(ObjectOpKind::parse("set").unwrap(), ObjectOpKind::Set);
    }

    #[test]
    fn rejects_delete_root() {
        let err = parse_op(&json!({"op":"delete","path":"/"}), 0).unwrap_err();
        assert!(err.message.contains("whole document"), "{}", err.message);
    }
}
