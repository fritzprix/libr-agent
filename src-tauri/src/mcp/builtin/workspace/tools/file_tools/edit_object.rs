use crate::mcp::schema::SchemaProperties;
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Keep in sync with `file_operations::edit_object::format::MAX_OPS`.
const EDIT_OBJECT_MAX_OPS: u32 = 32;

fn create_op_item_schema() -> crate::mcp::schema::JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert(
        "op".to_string(),
        enum_prop_required(
            vec!["set", "delete", "append", "remove"],
            "Mutation kind. set=create/replace; delete=remove object key; append=push to array; remove=delete array element at pointer (e.g. /keywords/0).",
        ),
    );
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some(
                "JSON Pointer to the target. Must start with '/'. Use '/' with op=set to replace the whole document. Examples: /scripts/dev, /keywords/0.",
            ),
        ),
    );
    props.insert(
        "value".to_string(),
        any_json_prop(Some(
            "JSON value for set/append (any JSON type). Omit for delete/remove.",
        )),
    );

    // value is conditionally required; runtime validates per op.
    object_schema(props, vec!["op".to_string(), "path".to_string()])
}

pub fn create_edit_object_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some(
                "Path to an existing .json file. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Skill aliases are read/list-only.",
            ),
        ),
    );
    props.insert(
        "ops".to_string(),
        array_schema_with_max_items(
            create_op_item_schema(),
            Some(EDIT_OBJECT_MAX_OPS),
            Some(
                "Atomic list of mutations applied in order. If any op fails, the file is left unchanged.",
            ),
        ),
    );

    MCPTool {
        name: "editObject".to_string(),
        title: Some("Edit JSON Object File".to_string()),
        description: "Edit an existing JSON file by path + typed operations (set/delete/append/remove).

Parses JSON, applies all ops atomically in memory, then writes pretty-printed JSON (2-space indent, trailing newline). Key encounter order is preserved; whitespace is normalized.

Strict JSON only — comments/trailing commas (JSONC) are rejected. Prefer package.json and other pure JSON configs. Files like many tsconfig.json templates that contain // comments must be edited with workspace__strReplace first (or cleaned to strict JSON). Not for source code, YAML, or TOML.

PREREQUISITE: file must already exist and be valid JSON.
- set: create or replace value at JSON Pointer (path '/' replaces the whole document)
- delete: remove an object key (e.g. /scripts/old)
- append: push value onto an array (path points to the array)
- remove: delete an array element (path points to the element, e.g. /keywords/0)".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "ops".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}
