use crate::mcp::builtin::browser::BrowserServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, invalid_input_error, missing_param_error, operation_failed_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

pub async fn evaluate_js(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        match guard.clone() {
            Some(id) => id,
            None => {
                return Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    "No active browser session found for this agent",
                    ToolGroup::Browser,
                )
                .guidance(vec![
                    "Use createSession FIRST to start a browser session".to_string(),
                    "Wait for createSession to return a success message before evaluating JavaScript".to_string(),
                    "Use navigateToUrl after createSession if you need to inspect a specific page".to_string(),
                ])
                .to_mcp_result());
            }
        }
    };
    let script = args
        .get("script")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .ok_or_else(|| missing_param_error("script", ToolGroup::Browser));

    let script = match script {
        Ok(script) => script,
        Err(result) => return Ok(result),
    };

    if script.is_empty() {
        return Ok(invalid_input_error(
            "JavaScript code cannot be empty",
            ToolGroup::Browser,
        ));
    }

    let result = match service.execute_script(&session_id, script).await {
        Ok(result) => result,
        Err(error) => {
            return Ok(operation_failed_error(
                "Evaluate JavaScript",
                &error,
                vec![
                    "Verify the script uses valid JavaScript syntax".to_string(),
                    "Wrap complex return values with JSON.stringify(...) before returning them"
                        .to_string(),
                    "Use getConsoleLogs to inspect page-side errors after execution".to_string(),
                ],
                ToolGroup::Browser,
            ));
        }
    };

    let hint = SuccessHint::new(
        format!("JavaScript executed\n\nResult:\n{}", result),
        vec![
            "Use getConsoleLogs if you need page-side error output".to_string(),
            "Use getPageContent to verify page state after DOM changes".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

pub async fn get_console_logs(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        match guard.clone() {
            Some(id) => id,
            None => {
                return Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    "No active browser session found for this agent",
                    ToolGroup::Browser,
                )
                .guidance(vec![
                    "Use createSession FIRST to start a browser session".to_string(),
                    "Wait for createSession to return a success message before reading console logs".to_string(),
                    "Use navigateToUrl after createSession if you need logs from a specific page".to_string(),
                ])
                .to_mcp_result());
            }
        }
    };
    let max_entries = args
        .get("maxEntries")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as u32;

    let logs = match service
        .get_console_logs(&session_id, Some(max_entries))
        .await
    {
        Ok(logs) => logs,
        Err(error) => {
            return Ok(operation_failed_error(
                "Get console logs",
                &error,
                vec![
                    "Verify the browser session is still active".to_string(),
                    "Use evaluateJS to reproduce the issue before reading logs again".to_string(),
                    "Use navigateToUrl or createSession to reset the page if logging has stalled"
                        .to_string(),
                ],
                ToolGroup::Browser,
            ));
        }
    };

    let formatted = format_logs_as_text(&logs);
    let hint = SuccessHint::new(
        formatted,
        vec![
            "Use evaluateJS to inspect page state around the logged errors".to_string(),
            "Use getPageContent if you need the rendered page context".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

fn format_logs_as_text(logs: &[crate::browser_sidecar::ConsoleEntry]) -> String {
    if logs.is_empty() {
        return "No console logs found for this session.".to_string();
    }

    let mut lines = Vec::new();
    for entry in logs {
        let time_str = if entry.timestamp > 0.0 {
            let timestamp_secs = if entry.timestamp > 5_000_000_000.0 {
                entry.timestamp / 1000.0
            } else {
                entry.timestamp
            };
            let seconds = timestamp_secs.trunc() as i64;
            let nanoseconds = (timestamp_secs.fract() * 1_000_000_000.0).abs() as u32;
            if let Some(dt) = chrono::DateTime::from_timestamp(seconds, nanoseconds) {
                dt.format("%H:%M:%S%.3f").to_string()
            } else {
                format!("{:.3}", entry.timestamp)
            }
        } else {
            format!("{:.3}", entry.timestamp)
        };

        lines.push(format!(
            "[{}] [{}] {}",
            time_str,
            entry.level.to_uppercase(),
            entry.text
        ));
    }
    lines.join("\n")
}
