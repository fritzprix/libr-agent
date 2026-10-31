use serde_json::Value;

use crate::commands::dataset_commands::{
    export_dataset as run_export_dataset, DatasetFilter, ExportFormat,
};
use crate::mcp::builtin::error_guidance::{invalid_input_error, missing_param_error, ToolGroup};
use crate::mcp::types::MCPResult;

/// Export conversational logs into structured formats for fine-tuning.
pub async fn export_dataset(args: Value) -> Result<MCPResult, String> {
    let session_ids: Option<Vec<String>> = args
        .get("sessionIds")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let Some(format_value) = args.get("format") else {
        return Ok(missing_param_error("format", ToolGroup::Agent));
    };
    let format: ExportFormat = match serde_json::from_value(format_value.clone()) {
        Ok(format) => format,
        Err(e) => {
            return Ok(invalid_input_error(
                &format!("Invalid 'format' parameter: {e}"),
                ToolGroup::Agent,
            ))
        }
    };

    let Some(output_path_value) = args.get("outputPath") else {
        return Ok(missing_param_error("outputPath", ToolGroup::Agent));
    };
    let Some(output_path) = output_path_value.as_str().map(str::to_string) else {
        return Ok(invalid_input_error(
            "Invalid 'outputPath' parameter: expected a string path",
            ToolGroup::Agent,
        ));
    };

    let filters: Option<DatasetFilter> = args
        .get("filters")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let result = run_export_dataset(session_ids, format, output_path, filters).await?;

    Ok(MCPResult::success(&format!(
        "Successfully exported dataset: {} sessions, {} messages written to {}",
        result.session_count, result.message_count, result.output_path
    )))
}
