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

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    // Proactive URL validation
    const MAX_URL_LENGTH: usize = 2048;

    if url.len() > MAX_URL_LENGTH {
        return Ok(invalid_input_error(
            &format!(
                "URL exceeds maximum length of {} characters",
                MAX_URL_LENGTH
            ),
            ToolGroup::Browser,
        ));
    }

    if url.starts_with("file://") {
        return Ok(invalid_input_error(
            "Local file URLs are not supported for security. Use http:// or https:// URLs only",
            ToolGroup::Browser,
        ));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("about:") {
        return Ok(invalid_input_error(
            &format!(
                "Invalid URL format: '{}'. Must start with http://, https://, or about:",
                url
            ),
            ToolGroup::Browser,
        ));
    }

    let result = match service.navigate_to_url(&browser_session_id, url).await {
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

    let suggestions = if result.contains("load wait timed out") {
        vec![
            "Navigation timed out waiting for page load. The page may still be usable."
                .to_string(),
            "Try extractWebContent to see if content is available despite the timeout.".to_string(),
            "If the page is blank or broken, create a new session and try a different URL."
                .to_string(),
        ]
    } else if result.contains("(HTTP 403)") || result.contains("(HTTP 401)") {
        vec![
            "The page is blocking access (Forbidden/Unauthorized). Abandon this page.".to_string(),
            "Do NOT try to login or bypass checks.".to_string(),
            "Search for the information on a different, public website.".to_string(),
        ]
    } else if result.contains("(HTTP 404)") {
        vec![
            "The page was not found (404). Check the URL.".to_string(),
            "Search for the content on the site's homepage or use a search engine.".to_string(),
        ]
    } else if result.contains("(HTTP 5") {
        // Covers 500, 502, 503, etc.
        vec![
            "The website is experiencing server errors (5xx). Abandon this page.".to_string(),
            "Try finding the information on a different website.".to_string(),
        ]
    } else if result.contains("Network Error") {
        vec![
            "A network error occurred. Check the URL and internet connection.".to_string(),
            "The site may be down or unreachable.".to_string(),
        ]
    } else if result.contains("(HTTP ") {
        // Fallback for other HTTP errors (e.g. 418, 429)
        vec!["The site returned an error. Consider finding an alternative source.".to_string()]
    } else {
        vec![
            "Extract page content with extractWebContent to see what's on the page".to_string(),
            "List interactive elements with listInteractable".to_string(),
        ]
    };

    let hint = SuccessHint::new(result, suggestions);
    Ok(hint.to_mcp_result())
}

pub async fn navigate_back(server: &BrowserServer, _args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let result = match service.navigate_back(&browser_session_id).await {
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

pub async fn navigate_forward(server: &BrowserServer, _args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let result = match service.navigate_forward(&browser_session_id).await {
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

pub async fn get_current_url(server: &BrowserServer, _args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let result = match service
        .execute_script(&browser_session_id, "window.location.href")
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

pub async fn get_page_title(server: &BrowserServer, _args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let result = match service
        .execute_script(&browser_session_id, "document.title")
        .await
    {
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
