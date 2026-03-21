use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Create a simple data visualization
pub fn visualize_data_tool() -> MCPTool {
    let data_item_schema = object_prop(
        vec![
            (
                "label".to_string(),
                string_prop_required("Data point label"),
            ),
            (
                "value".to_string(),
                number_prop(None, None, Some("Data point value")),
            ),
        ],
        vec!["label".to_string(), "value".to_string()],
        None,
    );

    MCPTool {
        name: "visualizeData".to_string(),
        title: Some("Visualize Data".to_string()),
        description: "Create a simple data visualization (bar or line chart).".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "type".to_string(),
                    enum_prop_required(vec!["bar", "line"], "Type of chart to create"),
                ),
                (
                    "data".to_string(),
                    array_schema(data_item_schema, Some("Data points")),
                ),
            ],
            vec!["type".to_string(), "data".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Render arbitrary content (HTML or Markdown) with interactive elements
pub fn present_interactive_tool() -> MCPTool {
    let interaction_schema = object_prop(
        vec![
            (
                "type".to_string(),
                enum_prop_required(
                    vec!["text", "select", "multiselect"],
                    "Type of interaction UI to display",
                ),
            ),
            (
                "prompt".to_string(),
                string_prop_required("The question or instruction to show the user"),
            ),
            (
                "options".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Options for select/multiselect (required for those types)"),
                ),
            ),
        ],
        vec!["type".to_string(), "prompt".to_string()],
        None,
    );

    MCPTool {
        name: "presentInteractive".to_string(),
        title: Some("Present Interactive Content".to_string()),
        description: "Render HTML or Markdown content with integrated interactive elements (text input, select, or multiselect).

Use this as the default UI presentation tool.
- For display-only content, omit `interaction`
- For content plus immediate user response, include `interaction`

Parameters:
- `content`: The HTML or Markdown string to render (required)
 - `format`: 'html' | 'markdown' | 'auto' (default: 'auto' — treated as Markdown unless explicitly set to 'html')
- `title`: Optional title shown above the content
- `interaction`: Configuration for the interactive section (prompt, type, options)"
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "content".to_string(),
                    string_prop_required("The content string to render (HTML or Markdown)"),
                ),
                (
                    "format".to_string(),
                    enum_prop(
                        vec!["html", "markdown", "auto"],
                        "auto",
                        Some("Content format. 'auto' defaults to Markdown; use 'html' for raw HTML rendering"),
                    ),
                ),
                (
                    "title".to_string(),
                    string_prop(None, None, Some("Optional title displayed above the content")),
                ),
                (
                    "interaction".to_string(),
                    interaction_schema,
                ),
            ],
            vec!["content".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Returns all UI tools intended for the AI agent
/// Note: Internal callback tools (getUserAnswer, circuitBreak, resumeCircuitBreak)
/// are NOT included here to prevent the AI from hallucinating calls to them.
/// `presentInteractive` is the single AI-facing entry point for UI rendering.
pub fn all_tools() -> Vec<MCPTool> {
    vec![present_interactive_tool()]
}
