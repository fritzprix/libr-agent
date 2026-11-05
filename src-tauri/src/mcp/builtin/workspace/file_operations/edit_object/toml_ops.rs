//! Object mutations applied to a `toml_edit::DocumentMut`. JSON Pointer tokens map to
//! TOML keys and array indexes; keys that are not edited keep their formatting.

use super::ops::{
    apply_ops_with, array_index_guidance, logic_error, op_error, path_missing, truncate_summary,
    type_mismatch, AppliedOp, ApplyError, ObjectOp, ObjectOpKind,
};
use super::pointer::parse_array_index;
use serde_json::Value as JsonValue;
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike, Value};

pub fn apply_toml_ops(
    document: &mut DocumentMut,
    ops: &[ObjectOp],
) -> Result<Vec<AppliedOp>, ApplyError> {
    apply_ops_with(document, ops, apply_one)
}

fn apply_one(document: &mut DocumentMut, op: &ObjectOp) -> Result<String, ApplyError> {
    match op.kind {
        ObjectOpKind::Set => apply_set(document, op),
        ObjectOpKind::Delete => apply_delete(document, op),
        ObjectOpKind::Append => apply_append(document, op),
        ObjectOpKind::Remove => apply_remove(document, op),
    }
}

/// A node reached while walking a pointer: a container, or a leaf value's type name.
enum Node<'a> {
    Table(&'a mut dyn TableLike),
    Array(&'a mut Array),
    Tables(&'a mut ArrayOfTables),
    Leaf(&'static str),
}

fn item_node(item: &mut Item) -> Node<'_> {
    match item {
        Item::Table(table) => Node::Table(table),
        Item::ArrayOfTables(tables) => Node::Tables(tables),
        Item::Value(value) => value_node(value),
        Item::None => Node::Leaf("none"),
    }
}

fn value_node(value: &mut Value) -> Node<'_> {
    match value {
        Value::InlineTable(table) => Node::Table(table),
        Value::Array(array) => Node::Array(array),
        other => Node::Leaf(value_type_name(other)),
    }
}

fn resolve<'a>(
    document: &'a mut DocumentMut,
    tokens: &[String],
    op: &ObjectOp,
) -> Result<Node<'a>, ApplyError> {
    let mut node = Node::Table(document.as_table_mut());
    for token in tokens {
        node = match node {
            Node::Table(table) => item_node(
                table
                    .get_mut(token)
                    .filter(|item| !item.is_none())
                    .ok_or_else(|| path_missing(op, token))?,
            ),
            Node::Array(array) => {
                let index = traversal_index(op, token)?;
                let len = array.len();
                value_node(
                    array
                        .get_mut(index)
                        .ok_or_else(|| traversal_out_of_bounds(op, index, len))?,
                )
            }
            Node::Tables(tables) => {
                let index = traversal_index(op, token)?;
                let len = tables.len();
                Node::Table(
                    tables
                        .get_mut(index)
                        .ok_or_else(|| traversal_out_of_bounds(op, index, len))?,
                )
            }
            Node::Leaf(kind) => {
                return Err(op_error(
                    op,
                    format!("Cannot traverse into {kind} at token '{token}'"),
                    vec![inspect_guidance()],
                ))
            }
        };
    }
    Ok(node)
}

fn apply_set(document: &mut DocumentMut, op: &ObjectOp) -> Result<String, ApplyError> {
    let json = op
        .value
        .as_ref()
        .ok_or_else(|| logic_error(op, "internal: set missing value"))?;
    let mut value = to_toml_value(op, json)?;

    let Some((last, parent_tokens)) = op.path.tokens().split_last() else {
        let table = into_table(op, value, "object (a TOML document root is a table)")?;
        let after = format!("table({} keys)", table.len());
        *document.as_table_mut() = table;
        return Ok(format!("replaced document → {after}"));
    };

    match resolve(document, parent_tokens, op)? {
        Node::Table(table) => {
            let after = compact_value(&value);
            if let Some(item) = table.get_mut(last).filter(|item| !item.is_none()) {
                let before = compact_item(item);
                let replaces_value = if let Item::Value(old) = &*item {
                    *value.decor_mut() = old.decor().clone();
                    true
                } else {
                    false
                };
                *item = Item::Value(value);
                if !replaces_value {
                    // A `[table]` key has no spacing around `=`; use the default.
                    if let Some(mut key) = table.key_mut(last) {
                        key.leaf_decor_mut().clear();
                    }
                }
                Ok(format!("set {before} → {after}"))
            } else {
                table.insert(last, Item::Value(value));
                Ok(format!("created = {after}"))
            }
        }
        Node::Array(array) => {
            let index = set_index(op, last, array.len())?;
            let after = compact_value(&value);
            let before = compact_value(&array.replace(index, value));
            Ok(format!("set [{index}] {before} → {after}"))
        }
        Node::Tables(tables) => {
            let index = set_index(op, last, tables.len())?;
            let table = into_table(op, value, "object (array of tables element)")?;
            let after = format!("table({} keys)", table.len());
            let slot = tables
                .get_mut(index)
                .ok_or_else(|| logic_error(op, "internal: checked index missing"))?;
            let before = format!("table({} keys)", slot.len());
            *slot = table;
            Ok(format!("set [{index}] {before} → {after}"))
        }
        Node::Leaf(kind) => Err(type_mismatch(op, "table or array (parent)", kind)),
    }
}

fn apply_delete(document: &mut DocumentMut, op: &ObjectOp) -> Result<String, ApplyError> {
    let (last, parent_tokens) = op
        .path
        .tokens()
        .split_last()
        .ok_or_else(|| logic_error(op, "internal: delete on root"))?;

    match resolve(document, parent_tokens, op)? {
        Node::Table(table) => {
            let removed = table
                .remove(last)
                .filter(|item| !item.is_none())
                .ok_or_else(|| path_missing(op, last))?;
            Ok(format!("deleted {}", compact_item(&removed)))
        }
        Node::Array(_) | Node::Tables(_) => Err(op_error(
            op,
            "op='delete' cannot remove array elements. Use op='remove' with a pointer to the element (e.g. /keywords/0).",
            vec!["Example: {\"op\":\"remove\",\"path\":\"/keywords/0\"}".to_string()],
        )),
        Node::Leaf(kind) => Err(type_mismatch(op, "table", kind)),
    }
}

fn apply_append(document: &mut DocumentMut, op: &ObjectOp) -> Result<String, ApplyError> {
    let json = op
        .value
        .as_ref()
        .ok_or_else(|| logic_error(op, "internal: append missing value"))?;
    let value = to_toml_value(op, json)?;

    match resolve(document, op.path.tokens(), op)? {
        Node::Array(array) => {
            let summary = compact_value(&value);
            push_keeping_layout(array, value);
            Ok(format!("appended {summary} (len={})", array.len()))
        }
        Node::Tables(tables) => {
            let table = into_table(op, value, "object (array of tables element)")?;
            let summary = format!("table({} keys)", table.len());
            tables.push(table);
            Ok(format!("appended {summary} (len={})", tables.len()))
        }
        Node::Table(_) => Err(type_mismatch(op, "array", "table")),
        Node::Leaf(kind) => Err(type_mismatch(op, "array", kind)),
    }
}

fn apply_remove(document: &mut DocumentMut, op: &ObjectOp) -> Result<String, ApplyError> {
    let (last, parent_tokens) = op
        .path
        .tokens()
        .split_last()
        .ok_or_else(|| logic_error(op, "internal: remove on root"))?;
    let index = parse_array_index(last).map_err(|message| {
        op_error(
            op,
            format!("{message}. op='remove' path must end with an array index (e.g. /keywords/0)."),
            vec![
                "Example: {\"op\":\"remove\",\"path\":\"/keywords/0\"}".to_string(),
                "To delete a table key use op='delete' instead".to_string(),
            ],
        )
    })?;

    match resolve(document, parent_tokens, op)? {
        Node::Array(array) => {
            check_remove_index(op, index, array.len())?;
            let removed = remove_with_line_comment(array, index);
            Ok(format!("removed [{index}] {}", compact_value(&removed)))
        }
        Node::Tables(tables) => {
            check_remove_index(op, index, tables.len())?;
            let removed = tables.remove(index);
            Ok(format!("removed [{index}] table({} keys)", removed.len()))
        }
        Node::Table(_) => Err(op_error(
            op,
            "op='remove' targets array elements. For table keys use op='delete'.",
            vec![format!(
                "Example: {{\"op\":\"delete\",\"path\":\"{}\"}}",
                op.path.display()
            )],
        )),
        Node::Leaf(kind) => Err(type_mismatch(op, "array (parent)", kind)),
    }
}

/// Convert an op value to a TOML value. Objects become inline tables.
fn to_toml_value(op: &ObjectOp, json: &JsonValue) -> Result<Value, ApplyError> {
    Ok(match json {
        JsonValue::Null => {
            return Err(op_error(
                op,
                "TOML has no null value",
                vec![
                    "To remove a key use op='delete'; to remove an array element use op='remove'"
                        .to_string(),
                ],
            ))
        }
        JsonValue::Bool(flag) => Value::from(*flag),
        JsonValue::Number(number) => match (number.as_i64(), number.as_f64()) {
            (Some(integer), _) => Value::from(integer),
            (None, Some(float)) if number.is_f64() => Value::from(float),
            _ => {
                return Err(op_error(
                    op,
                    format!("Number {number} is outside the TOML integer range (64-bit signed)"),
                    vec![],
                ))
            }
        },
        JsonValue::String(text) => Value::from(text.as_str()),
        JsonValue::Array(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(to_toml_value(op, item)?);
            }
            Value::Array(array)
        }
        JsonValue::Object(map) => {
            let mut table = InlineTable::new();
            for (key, item) in map {
                table.insert(key.as_str(), to_toml_value(op, item)?);
            }
            Value::InlineTable(table)
        }
    })
}

// Array layout in `toml_edit`: the text between a comma and the next element is
// that element's prefix, so an element's same-line comment (`"a", # note`) is the
// head (text before the first newline) of the next element's prefix, or of the
// array trailing text for the last element. Without a trailing comma, the last
// element's comment is the head of its own suffix.

/// Push after the last element. In a single-line array the space before `]`
/// moves to the new element (`[ "a" ]` → `[ "a", "b" ]`). In a multi-line array
/// the new element goes on its own line with the previous element's indent, and
/// the previous element keeps its same-line comment.
fn push_keeping_layout(array: &mut Array, value: Value) {
    let trailing_comma = array.trailing_comma();
    let trailing = array.trailing().as_str().unwrap_or("").to_string();
    let Some(last) = array.iter_mut().last() else {
        array.push(value);
        return;
    };
    let last_prefix = decor_str(last.decor().prefix());
    let (prefix, suffix, new_trailing) = match last_prefix.rfind('\n') {
        None => {
            let suffix = decor_str(last.decor().suffix());
            last.decor_mut().set_suffix("");
            (" ".to_string(), suffix, None)
        }
        Some(newline) => {
            let indent = &last_prefix[newline + 1..];
            if trailing_comma {
                let (comment, rest) = split_line_head(&trailing);
                (
                    format!("{comment}\n{indent}"),
                    String::new(),
                    Some(rest.to_string()),
                )
            } else {
                let last_suffix = decor_str(last.decor().suffix());
                let (comment, rest) = split_line_head(&last_suffix);
                let layout = (format!("{comment}\n{indent}"), rest.to_string(), None);
                last.decor_mut().set_suffix("");
                layout
            }
        }
    };
    if let Some(trailing) = new_trailing {
        array.set_trailing(trailing);
    }
    array.push(value);
    if let Some(pushed) = array.iter_mut().last() {
        pushed.decor_mut().set_prefix(prefix);
        pushed.decor_mut().set_suffix(suffix);
    }
}

/// Remove an element. Comment lines directly above it go with it; its own
/// same-line comment is dropped; the previous element's same-line comment
/// (stored in the removed element's prefix) is kept.
fn remove_with_line_comment(array: &mut Array, index: usize) -> Value {
    let trailing_comma = array.trailing_comma();
    let removed = array.remove(index);
    let removed_prefix = decor_str(removed.decor().prefix());
    let (kept_head, _) = split_line_head(&removed_prefix);
    let kept_head = kept_head.to_string();

    if let Some(next) = array.get_mut(index) {
        let next_prefix = decor_str(next.decor().prefix());
        let (_, rest) = split_line_head(&next_prefix);
        let prefix = format!("{kept_head}{rest}");
        next.decor_mut().set_prefix(prefix);
    } else if trailing_comma || array.is_empty() {
        let tail = format!(
            "{}{}",
            if trailing_comma {
                String::new()
            } else {
                decor_str(removed.decor().suffix())
            },
            array.trailing().as_str().unwrap_or("")
        );
        let (_, rest) = split_line_head(&tail);
        let trailing = format!("{kept_head}{rest}");
        array.set_trailing(trailing);
    } else {
        let tail = format!(
            "{}{}",
            decor_str(removed.decor().suffix()),
            array.trailing().as_str().unwrap_or("")
        );
        let (_, rest) = split_line_head(&tail);
        let rest = rest.to_string();
        array.set_trailing("");
        if let Some(previous) = array.iter_mut().last() {
            let suffix = format!("{}{kept_head}{rest}", decor_str(previous.decor().suffix()));
            previous.decor_mut().set_suffix(suffix);
        }
    }
    removed
}

fn decor_str(raw: Option<&toml_edit::RawString>) -> String {
    raw.and_then(|raw| raw.as_str()).unwrap_or("").to_string()
}

/// Split `text` at its first newline: (same-line head, rest from the newline).
/// Without a newline the whole text is the head.
fn split_line_head(text: &str) -> (&str, &str) {
    match text.find('\n') {
        Some(newline) => text.split_at(newline),
        None => (text, ""),
    }
}

fn into_table(op: &ObjectOp, value: Value, expected: &str) -> Result<Table, ApplyError> {
    match value {
        Value::InlineTable(table) => Ok(table.into_table()),
        other => Err(type_mismatch(op, expected, value_type_name(&other))),
    }
}

fn traversal_index(op: &ObjectOp, token: &str) -> Result<usize, ApplyError> {
    parse_array_index(token).map_err(|message| {
        op_error(
            op,
            message,
            vec!["When traversing an array, pointer tokens must be decimal indexes".to_string()],
        )
    })
}

fn traversal_out_of_bounds(op: &ObjectOp, index: usize, len: usize) -> ApplyError {
    op_error(
        op,
        format!("Path not found: array index {index} out of bounds (len={len})"),
        vec![inspect_guidance()],
    )
}

fn set_index(op: &ObjectOp, token: &str, len: usize) -> Result<usize, ApplyError> {
    let index = parse_array_index(token).map_err(|message| {
        op_error(
            op,
            message,
            vec![
                "For arrays, the final pointer token must be a decimal index (e.g. /items/0)"
                    .to_string(),
            ],
        )
    })?;
    if index >= len {
        return Err(op_error(
            op,
            format!(
                "Array index {index} out of bounds (len={len}). set does not create sparse arrays."
            ),
            vec![
                "Use op=append to add at the end of the array".to_string(),
                array_index_guidance(len),
            ],
        ));
    }
    Ok(index)
}

fn check_remove_index(op: &ObjectOp, index: usize, len: usize) -> Result<(), ApplyError> {
    if index >= len {
        return Err(op_error(
            op,
            format!("Array index {index} out of bounds (len={len})"),
            vec![array_index_guidance(len)],
        ));
    }
    Ok(())
}

fn inspect_guidance() -> String {
    "Call workspace__readFile to inspect the current document structure".to_string()
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Integer(_) => "integer",
        Value::Float(_) => "float",
        Value::Boolean(_) => "boolean",
        Value::Datetime(_) => "datetime",
        Value::Array(_) => "array",
        Value::InlineTable(_) => "inline table",
    }
}

fn compact_value(value: &Value) -> String {
    truncate_summary(match value {
        Value::Array(array) => format!("array(len={})", array.len()),
        Value::InlineTable(table) => format!("table({} keys)", table.len()),
        other => other.to_string().trim().to_string(),
    })
}

fn compact_item(item: &Item) -> String {
    match item {
        Item::Value(value) => compact_value(value),
        Item::Table(table) => format!("table({} keys)", table.len()),
        Item::ArrayOfTables(tables) => format!("array of tables(len={})", tables.len()),
        Item::None => "none".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::format::{parse_toml, serialize_toml};
    use super::super::ops::{parse_op, ObjectOp};
    use super::apply_toml_ops;
    use serde_json::{json, Value};

    fn ops(raw: Value) -> Vec<ObjectOp> {
        raw.as_array()
            .expect("ops array")
            .iter()
            .enumerate()
            .map(|(index, op)| parse_op(op, index).expect("op should parse"))
            .collect()
    }

    fn edit(text: &str, raw_ops: Value) -> String {
        let mut document = parse_toml(text).expect("valid TOML");
        apply_toml_ops(&mut document, &ops(raw_ops)).expect("ops should apply");
        serialize_toml(&document, text, false).expect("serializable")
    }

    fn edit_err(text: &str, raw_ops: Value) -> String {
        let mut document = parse_toml(text).expect("valid TOML");
        apply_toml_ops(&mut document, &ops(raw_ops))
            .expect_err("ops should fail")
            .message
    }

    #[test]
    fn round_trip_keeps_comments_and_unedited_keys() {
        let original = "# Top comment\nname   =   \"demo\"   # trailing name comment\nversion = \"0.1.0\"\n\n[deps]\n# serde comment\nserde = \"1\"\nold = \"x\"  # old comment\n\n[tool]\nlist = [ \"a\",   \"b\" ]\n";
        let output = edit(
            original,
            json!([
                {"op":"set","path":"/version","value":"0.2.0"},
                {"op":"delete","path":"/deps/old"},
                {"op":"append","path":"/tool/list","value":"c"},
                {"op":"remove","path":"/tool/list/0"}
            ]),
        );
        assert_eq!(
            output,
            "# Top comment\nname   =   \"demo\"   # trailing name comment\nversion = \"0.2.0\"\n\n[deps]\n# serde comment\nserde = \"1\"\n\n[tool]\nlist = [ \"b\", \"c\" ]\n"
        );
    }

    #[test]
    fn set_keeps_trailing_comment_of_existing_key() {
        let output = edit(
            "port = 80  # http port\n",
            json!([{"op":"set","path":"/port","value":8080}]),
        );
        assert_eq!(output, "port = 8080  # http port\n");
    }

    #[test]
    fn type_changing_set_keeps_decor() {
        let output = edit(
            "a = \"x\" # note\n",
            json!([{"op":"set","path":"/a","value":{"b":1}}]),
        );
        assert_eq!(output, "a = { b = 1 } # note\n");
    }

    #[test]
    fn delete_removes_comment_above_key() {
        let output = edit(
            "[deps]\n# serde comment\nserde = \"1\"\nkeep = 1\n",
            json!([{"op":"delete","path":"/deps/serde"}]),
        );
        assert_eq!(output, "[deps]\nkeep = 1\n");
    }

    #[test]
    fn new_keys_use_default_format() {
        let output = edit(
            "[deps]\nserde  =  \"1\"\n",
            json!([
                {"op":"set","path":"/deps/new","value":"2"},
                {"op":"set","path":"/meta","value":{"a":1,"tags":["x"]}}
            ]),
        );
        assert_eq!(
            output,
            "meta = { a = 1, tags = [\"x\"] }\n[deps]\nserde  =  \"1\"\nnew = \"2\"\n"
        );
    }

    #[test]
    fn append_and_remove_in_multiline_array_with_comments() {
        let output = edit(
            "list = [\n  \"a\", # first\n  \"b\",\n]\n",
            json!([
                {"op":"append","path":"/list","value":"c"},
                {"op":"remove","path":"/list/0"}
            ]),
        );
        assert_eq!(output, "list = [\n  \"b\",\n  \"c\",\n]\n");
    }

    #[test]
    fn remove_keeps_comment_above_next_element() {
        let output = edit(
            "list = [\n  \"a\",\n  # about b\n  \"b\", # last\n]\n",
            json!([
                {"op":"remove","path":"/list/1"},
                {"op":"remove","path":"/list/0"}
            ]),
        );
        assert_eq!(output, "list = [\n]\n");
        let output = edit(
            "list = [\n  \"a\",\n  # about b\n  \"b\",\n]\n",
            json!([{"op":"remove","path":"/list/0"}]),
        );
        assert_eq!(output, "list = [\n  # about b\n  \"b\",\n]\n");
    }

    #[test]
    fn remove_keeps_previous_element_comment() {
        let list = "list = [\n  \"a\", # keep a\n  \"b\", # drop b\n  \"c\", # keep c\n]\n";
        assert_eq!(
            edit(list, json!([{"op":"remove","path":"/list/1"}])),
            "list = [\n  \"a\", # keep a\n  \"c\", # keep c\n]\n"
        );
        assert_eq!(
            edit(list, json!([{"op":"remove","path":"/list/2"}])),
            "list = [\n  \"a\", # keep a\n  \"b\", # drop b\n]\n"
        );
    }

    #[test]
    fn remove_last_without_trailing_comma_keeps_previous_comment() {
        assert_eq!(
            edit(
                "list = [\n  \"a\", # keep a\n  \"b\" # drop b\n]\n",
                json!([{"op":"remove","path":"/list/1"}])
            ),
            "list = [\n  \"a\" # keep a\n]\n"
        );
    }

    #[test]
    fn remove_in_single_line_array() {
        let list = "list = [ \"a\", \"b\", \"c\" ]\n";
        for (index, expected) in [
            (0, "list = [ \"b\", \"c\" ]\n"),
            (1, "list = [ \"a\", \"c\" ]\n"),
            (2, "list = [ \"a\", \"b\" ]\n"),
        ] {
            let path = format!("/list/{index}");
            assert_eq!(edit(list, json!([{"op":"remove","path":path}])), expected);
        }
    }

    #[test]
    fn append_in_multiline_array_uses_own_line() {
        assert_eq!(
            edit(
                "list = [\n  \"a\",\n  \"b\", # b\n]\n",
                json!([{"op":"append","path":"/list","value":"c"}])
            ),
            "list = [\n  \"a\",\n  \"b\", # b\n  \"c\",\n]\n"
        );
        assert_eq!(
            edit(
                "list = [\n  \"a\",\n  \"b\" # b\n]\n",
                json!([{"op":"append","path":"/list","value":"c"}])
            ),
            "list = [\n  \"a\",\n  \"b\", # b\n  \"c\"\n]\n"
        );
    }

    #[test]
    fn replacing_table_with_value_uses_default_key_format() {
        assert_eq!(
            edit(
                "a = 1\n\n[deps]\nk = 1\n",
                json!([{"op":"set","path":"/deps","value":{"k":2}}])
            ),
            "a = 1\ndeps = { k = 2 }\n"
        );
    }

    #[test]
    fn array_of_tables_append_set_remove() {
        let output = edit(
            "[[bin]]\nname = \"a\"\n\n[[bin]]\nname = \"b\"\n",
            json!([
                {"op":"set","path":"/bin/0/path","value":"src/a.rs"},
                {"op":"append","path":"/bin","value":{"name":"c"}},
                {"op":"remove","path":"/bin/1"}
            ]),
        );
        assert_eq!(
            output,
            "[[bin]]\nname = \"a\"\npath = \"src/a.rs\"\n\n[[bin]]\nname = \"c\"\n"
        );
    }

    #[test]
    fn null_value_is_rejected() {
        let message = edit_err("a = 1\n", json!([{"op":"set","path":"/a","value":null}]));
        assert!(message.contains("no null"), "{message}");
    }

    #[test]
    fn out_of_range_integer_is_rejected() {
        let message = edit_err(
            "a = 1\n",
            json!([{"op":"set","path":"/a","value":18446744073709551615u64}]),
        );
        assert!(message.contains("integer range"), "{message}");
    }

    #[test]
    fn root_set_requires_table() {
        let message = edit_err("a = 1\n", json!([{"op":"set","path":"/","value":[1]}]));
        assert!(
            message.contains("Type mismatch") || message.contains("object"),
            "{message}"
        );
    }

    #[test]
    fn delete_on_array_points_to_remove() {
        let message = edit_err("list = [1]\n", json!([{"op":"delete","path":"/list/0"}]));
        assert!(message.contains("op='remove'"), "{message}");
    }

    #[test]
    fn missing_path_is_reported() {
        let message = edit_err("[deps]\n", json!([{"op":"set","path":"/nope/x","value":1}]));
        assert!(message.contains("nope"), "{message}");
    }

    #[test]
    fn failing_op_reports_its_index() {
        let mut document = parse_toml("a = 1\n").expect("valid TOML");
        let error = apply_toml_ops(
            &mut document,
            &ops(json!([
                {"op":"set","path":"/b","value":2},
                {"op":"append","path":"/a","value":3}
            ])),
        )
        .expect_err("second op should fail");
        assert_eq!(error.op_index, 1);
    }
}
