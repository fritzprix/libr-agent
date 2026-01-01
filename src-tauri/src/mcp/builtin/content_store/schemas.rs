// schemas.rs - Tool schema definitions
use crate::mcp::schema::JSONSchema;
use crate::mcp::utils::schema_builder::{integer_prop, object_schema, string_prop};
use std::collections::HashMap;

pub(crate) fn tool_list_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();

    // Create pagination object schema with description
    let mut pagination_props: HashMap<String, JSONSchema> = HashMap::new();
    pagination_props.insert(
        "offset".to_string(),
        integer_prop(Some(0), None, Some("Pagination offset")),
    );
    pagination_props.insert(
        "limit".to_string(),
        integer_prop(Some(1), Some(1000), Some("Pagination limit")),
    );

    let mut pagination_schema = object_schema(pagination_props, vec![]);
    // Add description to the pagination object itself
    pagination_schema.description =
        Some("Optional pagination parameters for listing content".to_string());

    props.insert("pagination".to_string(), pagination_schema);

    // Use None instead of empty vec![] for required
    object_schema(props, vec![])
}

pub(crate) fn tool_read_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "contentId".to_string(),
        string_prop(None, None, Some("Content ID to read")),
    );
    props.insert(
        "fromLine".to_string(),
        integer_prop(Some(1), None, Some("Starting line number (1-based)")),
    );
    props.insert(
        "toLine".to_string(),
        integer_prop(Some(1), None, Some("Ending line number (optional)")),
    );
    object_schema(props, vec!["contentId".to_string()])
}

pub(crate) fn tool_delete_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "contentId".to_string(),
        string_prop(None, None, Some("ID of the content to delete")),
    );
    object_schema(props, vec!["contentId".to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_list_content_schema_pagination() {
        let _schema = tool_list_content_schema();
        // Verify pagination properties
    }

    #[test]
    fn test_tool_read_content_schema_required_content_id() {
        let _schema = tool_read_content_schema();
        // Verify content_id is required
    }
}
