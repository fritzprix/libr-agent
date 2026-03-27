use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        list_tool(),
        read_session_tool(),
        read_message_tool(),
        search_tool(),
    ]
}

fn list_tool() -> MCPTool {
    MCPTool {
        name: "list".to_string(),
        title: Some("List History Sessions".to_string()),
        description: "List historical sessions with pagination and lightweight filters. Use this to find session IDs before reading session details."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "agentId".to_string(),
                    string_prop(None, None, Some("Optional agent/assistant configuration ID filter.")),
                ),
                (
                    "from".to_string(),
                    string_prop(None, None, Some("Optional start timestamp (ISO-8601).")),
                ),
                (
                    "to".to_string(),
                    string_prop(None, None, Some("Optional end timestamp (ISO-8601).")),
                ),
                (
                    "status".to_string(),
                    enum_prop(
                        vec!["idle", "busy", "paused", "error"],
                        "idle",
                        Some("Optional session status filter."),
                    ),
                ),
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number (default: 1).")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(
                        Some(1),
                        Some(100),
                        Some("Items per page (default: 20, max: 100)."),
                    ),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn read_session_tool() -> MCPTool {
    MCPTool {
        name: "readSession".to_string(),
        title: Some("Read Session".to_string()),
        description: "Read one session's metadata plus a paginated message list. Returns message previews only; use readMessage for full content."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionId".to_string(),
                    string_prop_required("Exact session ID to inspect."),
                ),
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Message list page number (default: 1).")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(
                        Some(1),
                        Some(100),
                        Some("Messages per page (default: 50, max: 100)."),
                    ),
                ),
            ],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn read_message_tool() -> MCPTool {
    MCPTool {
        name: "readMessage".to_string(),
        title: Some("Read Message".to_string()),
        description: "Read one message object with paginated content. Use this when a message body is too large for a preview."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "messageId".to_string(),
                    string_prop_required("Exact message ID to inspect."),
                ),
                (
                    "offsetChars".to_string(),
                    integer_prop(
                        Some(0),
                        None,
                        Some("Character offset into the rendered message content."),
                    ),
                ),
                (
                    "maxChars".to_string(),
                    integer_prop(
                        Some(1),
                        Some(3000),
                        Some("Maximum characters to return (hard-capped at 3000)."),
                    ),
                ),
            ],
            vec!["messageId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn search_tool() -> MCPTool {
    MCPTool {
        name: "search".to_string(),
        title: Some("Search History".to_string()),
        description:
            "Search message history and return bounded snippets plus IDs for follow-up reads."
                .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop_required("Search query text."),
                ),
                (
                    "agentId".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional agent/assistant configuration ID filter."),
                    ),
                ),
                (
                    "sessionId".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional exact session ID to constrain the search."),
                    ),
                ),
                (
                    "from".to_string(),
                    string_prop(None, None, Some("Optional start timestamp (ISO-8601).")),
                ),
                (
                    "to".to_string(),
                    string_prop(None, None, Some("Optional end timestamp (ISO-8601).")),
                ),
                (
                    "roles".to_string(),
                    array_schema(
                        enum_prop_required(
                            vec!["user", "assistant", "tool", "system"],
                            "Message role filter",
                        ),
                        Some("Optional role filters."),
                    ),
                ),
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number (default: 1).")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(
                        Some(1),
                        Some(100),
                        Some("Items per page (default: 20, max: 100)."),
                    ),
                ),
            ],
            vec!["query".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
