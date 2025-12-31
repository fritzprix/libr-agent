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
    let url = args
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("about:blank");

    // Check if session already exists
    {
        let id_lock = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        if let Some(id) = id_lock.as_ref() {
            let warning = ErrorGuidance::new(
                ErrorCategory::DuplicateResource,
                format!("Session already exists: {}", id),
                ToolGroup::Browser,
            );
            return Ok(warning.to_mcp_result());
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

    let hint = SuccessHint::new(
        format!("Browser session created: {}. {}", id, status_msg),
        vec![
            "Use navigateToUrl to load a webpage".to_string(),
            "Use extractWebContent to see the initial page".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}
