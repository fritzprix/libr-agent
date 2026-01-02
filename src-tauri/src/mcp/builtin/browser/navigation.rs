use crate::mcp::builtin::browser::{handle_browser_op_error, BrowserServer};
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

pub async fn navigate_to_url(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return Ok(missing_param_error("url", ToolGroup::Browser)),
    };
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
    };

    // Proactive URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("about:") {
        return Ok(invalid_input_error(
            &format!(
                "Invalid URL format: '{}'. Must start with http://, https://, or about:",
                url
            ),
            ToolGroup::Browser,
        ));
    }

    let result = match service.navigate_to_url(session_id, url).await {
        Ok(res) => res,
        Err(e) => {
            return Ok(handle_browser_op_error(
                "Navigate to URL",
                e,
                vec![
                    "Verify the URL format is valid (must include http:// or https://)",
                    "Use createSession to start a new browser session",
                    "Check if the session still exists",
                    "Try checking if the page loaded using getPageTitle",
                ],
            ))
        }
    };

    // Invalidate cache after navigation
    server.invalidate_cache();

    let hint = SuccessHint::new(
        result,
        vec![
            "Extract page content with extractWebContent to see what's on the page".to_string(),
            "List interactive elements with listInteractable".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

pub async fn navigate_back(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
    };

    let result = match service.navigate_back(session_id).await {
        Ok(res) => res,
        Err(e) => {
            return Ok(handle_browser_op_error(
                "Navigate back",
                e,
                vec![
                    "Ensure there is a previous page in history",
                    "Use getCurrentUrl to check current page",
                ],
            ))
        }
    };

    // Invalidate cache after navigation
    server.invalidate_cache();

    let hint = SuccessHint::new(
        result,
        vec!["Extract content with extractWebContent to see the previous page".to_string()],
    );
    Ok(hint.to_mcp_result())
}

pub async fn navigate_forward(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
    };

    let result = match service.navigate_forward(session_id).await {
        Ok(res) => res,
        Err(e) => {
            return Ok(handle_browser_op_error(
                "Navigate forward",
                e,
                vec![
                    "Ensure there is a next page in history",
                    "Use getCurrentUrl to check current page",
                ],
            ))
        }
    };

    // Invalidate cache after navigation
    server.invalidate_cache();

    let hint = SuccessHint::new(
        result,
        vec!["Extract content with extractWebContent to see the next page".to_string()],
    );
    Ok(hint.to_mcp_result())
}

pub async fn get_current_url(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
    };

    let result = match service
        .execute_script(session_id, "window.location.href")
        .await
    {
        Ok(res) => res,
        Err(e) => {
            return Ok(operation_failed_error(
                "Get current URL",
                &e,
                vec![
                    "Verify the browser session is active".to_string(),
                    "Use createSession to start a new session if needed".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    let hint = SuccessHint::new(
        result,
        vec!["Navigate to a different URL with navigateToUrl if needed".to_string()],
    );
    Ok(hint.to_mcp_result())
}

pub async fn get_page_title(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
    };

    let result = match service.execute_script(session_id, "document.title").await {
        Ok(res) => res,
        Err(e) => {
            return Ok(operation_failed_error(
                "Get page title",
                &e,
                vec![
                    "Verify the browser session is active".to_string(),
                    "Ensure the page has fully loaded".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    let hint = SuccessHint::new(
        result,
        vec![
            "Extract full page content with extractWebContent to see what's on this page"
                .to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}
