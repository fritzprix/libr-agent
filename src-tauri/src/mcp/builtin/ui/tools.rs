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

/// Receive user response from prompt UI
pub fn reply_prompt_tool() -> MCPTool {
    MCPTool {
        name: "replyPrompt".to_string(),
        title: Some("Reply Prompt".to_string()),
        description: "Receive user response from prompt UI (automatically called by UI action)"
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "messageId".to_string(),
                    string_prop_required("ID of the prompt being replied to"),
                ),
                (
                    "answer".to_string(),
                    string_prop(None, None, Some("User answer")),
                ),
                (
                    "cancelled".to_string(),
                    boolean_prop(Some("Whether the user cancelled the prompt")),
                ),
            ],
            vec!["messageId".to_string()],
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

/// Resume from wait state
pub fn resume_from_wait_tool() -> MCPTool {
    MCPTool {
        name: "resumeFromWait".to_string(),
        title: Some("Resume From Wait".to_string()),
        description: "Resume from wait state".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "resumeInstruction".to_string(),
                    string_prop_required("Resume instruction"),
                ),
                (
                    "startedAt".to_string(),
                    number_prop(None, None, Some("Timestamp when wait started")),
                ),
            ],
            vec!["resumeInstruction".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Display circuit breaker UI when agent is looping
pub fn circuit_break_tool() -> MCPTool {
    MCPTool {
        name: "circuitBreak".to_string(),
        title: Some("Circuit Break".to_string()),
        description: "Display circuit breaker UI when agent is looping".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "toolName".to_string(),
                    string_prop_required("Name of the tool that is looping"),
                ),
                (
                    "repetitionCount".to_string(),
                    number_prop(None, None, Some("Number of repetitions detected")),
                ),
                (
                    "args".to_string(),
                    string_prop(None, None, Some("Tool arguments as string")),
                ),
            ],
            vec!["toolName".to_string(), "repetitionCount".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Resume from circuit breaker
pub fn resume_circuit_break_tool() -> MCPTool {
    MCPTool {
        name: "resumeCircuitBreak".to_string(),
        title: Some("Resume Circuit Break".to_string()),
        description: "Resume from circuit breaker".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "toolName".to_string(),
                    string_prop_required("Name of the tool that was looping"),
                ),
                (
                    "repetitionCount".to_string(),
                    number_prop(None, None, Some("Number of repetitions that were detected")),
                ),
            ],
            vec!["toolName".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Returns all UI tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        prompt_user_tool(),
        reply_prompt_tool(),
        visualize_data_tool(),
        wait_for_user_resume_tool(),
        resume_from_wait_tool(),
        circuit_break_tool(),
        resume_circuit_break_tool(),
    ]
}
