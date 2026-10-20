use crate::mcp::builtin::browser::BrowserServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, invalid_input_error, operation_failed_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeScreenshotArgs {
    #[serde(default)]
    full_page: bool,
}

pub async fn take_screenshot(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let args = match serde_json::from_value::<TakeScreenshotArgs>(args) {
        Ok(args) => args,
        Err(error) => {
            return Ok(invalid_input_error(
                &format!("Invalid screenshot arguments: {error}"),
                ToolGroup::Browser,
            ));
        }
    };

    let service = server.get_browser_service()?;
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|error| error.to_string())?;
        guard.clone()
    };

    let Some(browser_session_id) = browser_session_id else {
        return Ok(guided_error(
            ErrorCategory::ResourceNotFound,
            "No active browser session found for this agent",
            ToolGroup::Browser,
        )
        .with_guidance(vec![
            "Use browser__createSession FIRST to start a browser session".to_string(),
            "Wait for browser__createSession to return a success message before taking a screenshot"
                .to_string(),
        ])
        .to_mcp_result());
    };

    let image_data = match service
        .take_screenshot(&browser_session_id, args.full_page)
        .await
    {
        Ok(image_data) => image_data,
        Err(error) => {
            return Ok(operation_failed_error(
                "Take screenshot",
                &error,
                vec![
                    "Verify the browser session is active".to_string(),
                    "Use fullPage: false for a viewport capture if the full page is too large"
                        .to_string(),
                    "Use browser__getPageContent for text-only page inspection".to_string(),
                ],
                ToolGroup::Browser,
            ));
        }
    };

    if image_data.is_empty() {
        return Ok(operation_failed_error(
            "Take screenshot",
            "The browser returned an empty PNG payload",
            vec![
                "Verify the browser session is active".to_string(),
                "Try taking the screenshot again after the page finishes loading".to_string(),
            ],
            ToolGroup::Browser,
        ));
    }

    let capture_scope = if args.full_page {
        "full page"
    } else {
        "viewport"
    };
    let base64_character_count = image_data.len();
    let result_text = format!(
        "Screenshot captured successfully ({capture_scope}, {base64_character_count} Base64 characters, image/png)."
    );

    // Persist to temp file for efficient download (avoids re-sending base64 over IPC)
    let file_uri = {
        use base64::{engine::general_purpose as b64, Engine as _};
        let png_bytes = b64::STANDARD.decode(&image_data).ok();
        png_bytes.and_then(|bytes| {
            let temp_dir = std::env::temp_dir();
            let timestamp = chrono::Utc::now().timestamp_millis();
            let file_name = format!("libragent-browser-screenshot-{}.png", timestamp);
            let temp_path = temp_dir.join(file_name);
            std::fs::write(&temp_path, &bytes).ok()?;
            url::Url::from_file_path(&temp_path).ok().map(|u| u.to_string())
        })
    };

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text { text: result_text },
            MCPContent::Image {
                data: Some(image_data),
                uri: file_uri,
                mime_type: "image/png".to_string(),
            },
        ]),
        structured_content: Some(json!({
            "format": "png",
            "mimeType": "image/png",
            "fullPage": args.full_page,
            "base64Characters": base64_character_count,
        })),
        is_error: Some(false),
    })
}
