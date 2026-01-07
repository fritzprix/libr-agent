use crate::mcp::builtin::browser::BrowserServer;
use crate::mcp::builtin::error_guidance::{
    operation_failed_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

pub async fn close_session(server: &BrowserServer, _args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    let id_opt = {
        let lock = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        lock.clone()
    };

    if let Some(id) = id_opt {
        match service.close_session(&id).await {
            Ok(_) => {}
            Err(e) => {
                return Ok(operation_failed_error(
                    "Close browser session",
                    &e,
                    vec![
                        "Verify the browser session is still active".to_string(),
                        "Use createSession to start a new session if needed".to_string(),
                    ],
                    ToolGroup::Browser,
                ))
            }
        }
        {
            let mut lock = server
                .browser_session_id
                .write()
                .map_err(|e| e.to_string())?;
            *lock = None;
        }

        let hint = SuccessHint::new(
            "Browser session closed",
            vec!["Use createSession to start a new browser session".to_string()],
        );
        Ok(hint.to_mcp_result())
    } else {
        let warning = ErrorGuidance::new(
            ErrorCategory::InvalidState,
            "No active browser session to close",
            ToolGroup::Browser,
        );
        Ok(warning.to_mcp_result())
    }
}

pub async fn create_session(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let url_param = args.get("url").and_then(|v| v.as_str());
    let url = url_param.unwrap_or("https://www.google.com");

    // Check if a session already exists. If so, close it to ensure a fresh session.
    // This allows "resetting" the session if it gets into a bad state.
    {
        let id_opt = {
            let lock = server
                .browser_session_id
                .read()
                .map_err(|e| e.to_string())?;
            lock.clone()
        };

        if let Some(id) = id_opt {
            // Attempt to close existing session, ignore errors as we're forcing a new one
            let _ = service.close_session(&id).await;

            // Clear the ID from state
            let mut lock = server
                .browser_session_id
                .write()
                .map_err(|e| e.to_string())?;
            *lock = None;
        }
    }

    let (id, status_msg) = match service
        .create_browser_session(url, Some(&format!("Agent {}", server.agent_session_id)))
        .await
    {
        Ok(res) => res,
        Err(e) => {
            return Ok(operation_failed_error(
                "Create browser session",
                &e,
                vec![
                    "Verify the URL format is valid (must include http:// or https://)".to_string(),
                    "Check browser service is available and running".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    {
        let mut id_lock = server
            .browser_session_id
            .write()
            .map_err(|e| e.to_string())?;
        *id_lock = Some(id.clone());
    }

    let (message, suggestions) = if url_param.is_some() {
        if status_msg.contains("load wait timed out") {
            (
                format!("Browser session created: {}. {}", id, status_msg),
                vec![
                    "Try creating a new session with 'createSession' to reset the state"
                        .to_string(),
                    "The site might be blocking automated access or is too slow".to_string(),
                ],
            )
        } else {
            (
                format!("Browser session created: {}. Page loaded: {}", id, url),
                vec![
                    "Use extractWebContent to read the page content".to_string(),
                    "Use listInteractable to see interactive elements".to_string(),
                ],
            )
        }
    } else {
        (
            format!("Browser session created: {}. {}", id, status_msg),
            vec!["Use navigateToUrl to load a webpage".to_string()],
        )
    };

    let hint = SuccessHint::new(message, suggestions);
    Ok(hint.to_mcp_result())
}
