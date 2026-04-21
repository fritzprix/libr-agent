use crate::mcp::builtin::browser::content::BROWSER_CONTENT_STORE;
use crate::mcp::builtin::browser::interaction::create_rich_response;
use crate::mcp::builtin::browser::{handle_browser_op_error, BrowserServer};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, operation_failed_error, ErrorCategory, SuccessHint,
    ToolGroup,
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use createSession FIRST to start a browser session".to_string(),
                "Wait for createSession to return a success message before navigating".to_string(),
            ])
            .to_mcp_result());
        }
    };

    // Proactive URL validation
    const MAX_URL_LENGTH: usize = 2048;

    if url.len() > MAX_URL_LENGTH {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "URL exceeds maximum length of {} characters",
                MAX_URL_LENGTH
            ),
            ToolGroup::Browser,
        )
        .to_mcp_result());
    }

    if url.starts_with("file://") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            "Local file URLs are not supported for security. Use http:// or https:// URLs only",
            ToolGroup::Browser,
        )
        .to_mcp_result());
    }

    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("about:") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid URL format: '{}'. Must start with http://, https://, or about:",
                url
            ),
            ToolGroup::Browser,
        )
        .to_mcp_result());
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

    // Invalidate state cache and content store after navigation
    server.invalidate_cache();
    BROWSER_CONTENT_STORE.clear_session(&browser_session_id);

    // Return HTTP-error-specific recovery guidance when navigation signals a problem.
    // Only fall through to rich-response (title+URL page state) on genuine success.
    if result.contains("load wait timed out") {
        let hint = SuccessHint::new(
            result,
            vec![
                "Navigation timed out waiting for page load. The page may still be usable."
                    .to_string(),
                "Try reading the page content to see if anything loaded despite the timeout."
                    .to_string(),
                "If the page is blank or broken, create a new session and try a different URL."
                    .to_string(),
            ],
        );
        return Ok(hint.to_mcp_result());
    } else if result.contains("(HTTP 403)") || result.contains("(HTTP 401)") {
        let hint = SuccessHint::new(
            result,
            vec![
                "This page is blocking automated access (Forbidden/Unauthorized). Abandon it."
                    .to_string(),
                "Do NOT attempt to log in or bypass the restriction.".to_string(),
                "Search for the same information on a different, public website.".to_string(),
            ],
        );
        return Ok(hint.to_mcp_result());
    } else if result.contains("(HTTP 404)") {
        let hint = SuccessHint::new(
            result,
            vec![
                "The page was not found (404). Verify the URL is correct.".to_string(),
                "Search for the content on the site's homepage or use a search engine.".to_string(),
            ],
        );
        return Ok(hint.to_mcp_result());
    } else if result.contains("(HTTP 5") {
        let hint = SuccessHint::new(
            result,
            vec![
                "The website is experiencing server errors (5xx). Abandon this page.".to_string(),
                "Try finding the same information on a different website.".to_string(),
            ],
        );
        return Ok(hint.to_mcp_result());
    } else if result.contains("Network Error") {
        let hint = SuccessHint::new(
            result,
            vec![
                "A network error occurred. Verify the URL and internet connection.".to_string(),
                "The site may be down or unreachable.".to_string(),
            ],
        );
        return Ok(hint.to_mcp_result());
    } else if result.contains("(HTTP ") {
        let hint = SuccessHint::new(
            result,
            vec!["The site returned an unexpected HTTP error. Consider finding an alternative source.".to_string()],
        );
        return Ok(hint.to_mcp_result());
    }

    // Genuine success: return live page state (title + URL) as verification.
    create_rich_response(&service, &browser_session_id, &result).await
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use createSession FIRST to start a browser session".to_string(),
                "Wait for createSession to return a success message before navigating".to_string(),
            ])
            .to_mcp_result());
        }
    };

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

    // Invalidate state cache and content store after navigation
    server.invalidate_cache();
    BROWSER_CONTENT_STORE.clear_session(&browser_session_id);

    create_rich_response(&service, &browser_session_id, &result).await
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use createSession FIRST to start a browser session".to_string(),
                "Wait for createSession to return a success message before navigating".to_string(),
            ])
            .to_mcp_result());
        }
    };

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

    // Invalidate state cache and content store after navigation
    server.invalidate_cache();
    BROWSER_CONTENT_STORE.clear_session(&browser_session_id);

    create_rich_response(&service, &browser_session_id, &result).await
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use createSession FIRST to start a browser session".to_string(),
                "Wait for createSession to return a success message before navigating".to_string(),
            ])
            .to_mcp_result());
        }
    };

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
        vec![
            "Use getPageContent to inspect the current page.".to_string(),
            "Use navigateToUrl only if you need to replace the current page in this same active session."
                .to_string(),
        ],
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
            "Extract full page content with getPageContent to see what's on this page".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}
