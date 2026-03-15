use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Display an interactive prompt to the user
pub fn prompt_user_tool() -> MCPTool {
    let options_schema = array_schema(
        string_prop(None, None, None),
        Some("Options for select/multiselect (required for those types)"),
    );

    MCPTool {
        name: "promptUser".to_string(),
        title: Some("Prompt User".to_string()),
        description: "Display an interactive prompt to the user (text input, select, or multiselect). Use this to gather user input interactively.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "prompt".to_string(),
                    string_prop_required("The question or instruction to show the user"),
                ),
                (
                    "type".to_string(),
                    enum_prop_required(
                        vec!["text", "select", "multiselect"],
                        "Type of prompt UI to display",
                    ),
                ),
                (
                    "options".to_string(),
                    options_schema,
                ),
            ],
            vec!["prompt".to_string(), "type".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

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

/// Display wait UI with continue button
pub fn wait_for_user_resume_tool() -> MCPTool {
    MCPTool {
        name: "waitForUserResume".to_string(),
        title: Some("Wait For User Resume".to_string()),
        description: "Display wait UI with continue button".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "message".to_string(),
                    string_prop_required("Message to display"),
                ),
                (
                    "resumeInstruction".to_string(),
                    string_prop_required("Instruction for resuming"),
                ),
            ],
            vec!["message".to_string(), "resumeInstruction".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Render arbitrary content (HTML or Markdown) as a visual panel
pub fn present_content_tool() -> MCPTool {
    MCPTool {
        name: "presentContent".to_string(),
        title: Some("Present Content".to_string()),
        description: "Render HTML or Markdown content as a visual panel in the chat.

Use this to display:
- Rich reports, summaries, or analysis results
- Formatted Markdown output (headings, lists, code blocks, tables)
- Custom HTML layouts

Parameters:
- `content`: The HTML or Markdown string to render (required)
- `format`: 'html' | 'markdown' | 'auto' (default: 'auto' — detects format from content)
- `title`: Optional title shown above the rendered content"
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
                        Some("Content format. 'auto' detects from content (default: 'auto')"),
                    ),
                ),
                (
                    "title".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional title displayed above the content"),
                    ),
                ),
            ],
            vec!["content".to_string()],
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
- `format`: 'html' | 'markdown' | 'auto' (default: 'auto')
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
                        Some("Content format. 'auto' detects from content"),
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
/// Note: Internal callback tools (getUserAnswer, resumeFromWait, circuitBreak, resumeCircuitBreak)
/// are NOT included here to prevent the AI from hallucinating calls to them.
/// Legacy split tools like `promptUser` and `presentContent` remain callable for compatibility,
/// but are intentionally hidden from the public AI-facing surface in favor of `presentInteractive`.
pub fn all_tools() -> Vec<MCPTool> {
    vec![wait_for_user_resume_tool(), present_interactive_tool()]
}
