use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

pub fn export_dataset_tool() -> MCPTool {
    let filters_schema = object_prop(
        vec![
            (
                "minTurns".to_string(),
                integer_prop(Some(0), None, Some("Minimum conversation turn count to include")),
            ),
            (
                "maxTurns".to_string(),
                integer_prop(Some(0), None, Some("Maximum conversation turn count to include")),
            ),
            (
                "excludeErrors".to_string(),
                boolean_prop(Some("If true, exclude sessions with error status")),
            ),
            (
                "excludeShort".to_string(),
                boolean_prop(Some("If true, exclude sessions with less than 2 turns")),
            ),
            (
                "minTokens".to_string(),
                integer_prop(Some(0), None, Some("Minimum estimated token count to include")),
            ),
        ],
        vec![],
        Some("Quality filters for dataset extraction"),
    );

    MCPTool {
        name: "export_dataset".to_string(),
        title: Some("Export Dataset".to_string()),
        description: "Export conversational logs from LibrAgent into structured formats for model fine-tuning.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionIds".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Optional list of session IDs to export. If omitted, exports all sessions."),
                    ),
                ),
                (
                    "format".to_string(),
                    enum_prop_required(
                        vec!["llamaFactory", "alpaca", "shareGpt", "openAiJsonl"],
                        "The target export format. 'llamaFactory' and 'shareGpt' export ShareGPT-style JSON datasets.",
                    ),
                ),
                (
                    "outputPath".to_string(),
                    string_prop_required("The absolute local file path where the dataset will be saved."),
                ),
                ("filters".to_string(), filters_schema),
            ],
            vec!["format".to_string(), "outputPath".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn all_tools() -> Vec<MCPTool> {
    vec![export_dataset_tool()]
}
