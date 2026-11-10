use super::export::export_dataset;
use crate::mcp::types::{MCPContent, MCPResult};
use serde_json::json;

fn error_text(result: Result<MCPResult, String>) -> String {
    let result = result.expect("bad arguments must not be an internal error");
    assert_eq!(result.is_error, Some(true));
    let text: String = result
        .content
        .iter()
        .flatten()
        .filter_map(|item| match item {
            MCPContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!text.to_lowercase().contains("system error"), "{text}");
    text
}

#[tokio::test]
async fn missing_format_names_the_param() {
    let text = error_text(export_dataset(json!({ "outputPath": "/tmp/out.json" })).await);
    assert!(text.contains("'format'"), "{text}");
}

#[tokio::test]
async fn invalid_format_is_reported_as_invalid() {
    let args = json!({ "format": "csv", "outputPath": "/tmp/out.json" });
    let text = error_text(export_dataset(args).await);
    assert!(text.contains("Invalid 'format' parameter"), "{text}");
}

#[tokio::test]
async fn missing_output_path_names_the_param() {
    let text = error_text(export_dataset(json!({ "format": "alpaca" })).await);
    assert!(text.contains("'outputPath'"), "{text}");
}

#[tokio::test]
async fn non_string_output_path_is_reported_as_invalid() {
    let args = json!({ "format": "alpaca", "outputPath": 3 });
    let text = error_text(export_dataset(args).await);
    assert!(text.contains("Invalid 'outputPath' parameter"), "{text}");
}
