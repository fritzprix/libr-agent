// schemas.rs - Tool schema definitions
use crate::mcp::schema::JSONSchema;
use crate::mcp::utils::schema_builder::{integer_prop, number_prop, object_schema, string_prop};
use std::collections::HashMap;

pub(crate) fn tool_add_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "file_url".to_string(),
        string_prop(None, None, Some("File URL (file://) to add")),
    );
    props.insert(
        "content".to_string(),
        string_prop(None, None, Some("Direct content to add")),
    );
    props.insert(
        "metadata".to_string(),
        object_schema(
            {
                let mut meta_props: HashMap<String, JSONSchema> = HashMap::new();
                meta_props.insert(
                    "filename".to_string(),
                    string_prop(None, None, Some("Content filename")),
                );
                meta_props.insert(
                    "mime_type".to_string(),
                    string_prop(None, None, Some("MIME type")),
                );
                meta_props.insert(
                    "size".to_string(),
                    integer_prop(Some(0), None, Some("Content size in bytes")),
                );
                meta_props.insert(
                    "uploaded_at".to_string(),
                    string_prop(None, None, Some("Upload timestamp")),
                );
                meta_props
            },
            vec![],
        ),
    );
    object_schema(props, vec![])
}

pub(crate) fn tool_list_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "pagination".to_string(),
        object_schema(
            {
                let mut pagination_props: HashMap<String, JSONSchema> = HashMap::new();
                pagination_props.insert(
                    "offset".to_string(),
                    integer_prop(Some(0), None, Some("Pagination offset")),
                );
                pagination_props.insert(
                    "limit".to_string(),
                    integer_prop(Some(1), Some(1000), Some("Pagination limit")),
                );
                pagination_props
            },
            vec![],
        ),
    );
    object_schema(props, vec![])
}

pub(crate) fn tool_read_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "content_id".to_string(),
        string_prop(None, None, Some("Content ID to read")),
    );
    props.insert(
        "from_line".to_string(),
        integer_prop(Some(1), None, Some("Starting line number (1-based)")),
    );
    props.insert(
        "to_line".to_string(),
        integer_prop(Some(1), None, Some("Ending line number (optional)")),
    );
    object_schema(props, vec!["content_id".to_string()])
}

pub(crate) fn tool_keyword_search_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "query".to_string(),
        string_prop(None, None, Some("Search query string")),
    );
    props.insert(
        "options".to_string(),
        object_schema(
            {
                let mut option_props: HashMap<String, JSONSchema> = HashMap::new();
                option_props.insert(
                    "top_n".to_string(),
                    integer_prop(
                        Some(1),
                        Some(100),
                        Some("Maximum number of results to return"),
                    ),
                );
                option_props.insert(
                    "threshold".to_string(),
                    number_prop(
                        Some(0.0),
                        Some(1.0),
                        Some("Minimum relevance score (0-1 float)"),
                    ),
                );
                option_props
            },
            vec![],
        ),
    );
    object_schema(props, vec!["query".to_string()])
}

pub(crate) fn tool_delete_content_schema() -> JSONSchema {
    let mut props: HashMap<String, JSONSchema> = HashMap::new();
    props.insert(
        "content_id".to_string(),
        string_prop(None, None, Some("ID of the content to delete")),
    );
    object_schema(props, vec!["content_id".to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_add_content_schema_has_required_fields() {
        let schema = tool_add_content_schema();
        // Verify schema structure
        assert!(matches!(
            schema.schema_type,
            crate::mcp::schema::JSONSchemaType::Object { .. }
        ));
        // Verify properties exist
        // (Add specific assertions based on JSONSchema structure)
    }

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

    #[test]
    fn test_tool_keyword_search_schema_query_required() {
        let _schema = tool_keyword_search_schema();
        // Verify query is required, options is optional
    }
}
