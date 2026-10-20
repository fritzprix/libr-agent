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

    // Persist once for download/externalize reuse (best-effort; never fail the capture).
    let file_uri = {
        use base64::{engine::general_purpose as b64, Engine as _};
        match b64::STANDARD.decode(&image_data) {
            Ok(bytes) => match crate::session::get_session_manager() {
                Ok(session_manager) => {
                    let workspace_dir =
                        session_manager.get_session_workspace_dir_by_id(&server.agent_session_id);
                    crate::mcp::builtin::media::persist_media_to_temp_file(
                        &workspace_dir,
                        &server.agent_session_id,
                        &bytes,
                        "png",
                    )
                    .map_err(|e| {
                        log::warn!("Failed to persist browser screenshot to temp file: {e}");
                        e
                    })
                    .ok()
                }
                Err(e) => {
                    log::warn!(
                        "Skipping screenshot temp persist; session manager unavailable: {e}"
                    );
                    None
                }
            },
            Err(e) => {
                log::warn!("Failed to decode base64 screenshot data for temp persist: {e}");
                None
            }
        }
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
