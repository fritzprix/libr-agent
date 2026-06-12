use crate::mcp::types::MCPResult;
use crate::commands::dataset_commands::{export_dataset as run_export_dataset, ExportFormat, DatasetFilter};
use serde_json::Value;

pub async fn export_dataset(args: Value) -> Result<MCPResult, String> {
    let session_ids: Option<Vec<String>> = args.get("sessionIds")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let format: ExportFormat = args.get("format")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| "Missing or invalid 'format' parameter".to_string())?;

    let output_path: String = args.get("outputPath")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| "Missing or invalid 'outputPath' parameter".to_string())?;

    let filters: Option<DatasetFilter> = args.get("filters")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let result = run_export_dataset(session_ids, format, output_path, filters).await?;

    Ok(MCPResult::success(&format!(
        "Successfully exported dataset: {} sessions, {} messages written to {}",
        result.session_count, result.message_count, result.output_path
    )))
}
