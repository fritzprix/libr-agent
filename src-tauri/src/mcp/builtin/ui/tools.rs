use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

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
        description: "Render HTML or Markdown content with optional interactive elements.

Use this as the default UI presentation tool.
- For display-only content, omit `interaction`
- For user response after content display, include `interaction`"
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
