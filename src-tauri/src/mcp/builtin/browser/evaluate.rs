use crate::mcp::builtin::browser::BrowserServer;
use crate::mcp::types::MCPResult;
use serde_json::Value;

pub async fn evaluate_js(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = server.get_active_session_id()?;
    let script = args
        .get("script")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required field: script".to_string())?;

    let result = service.execute_script(&session_id, script).await?;

    Ok(MCPResult::success(&format!("Result:\n{}", result)))
}

pub async fn get_console_logs(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = server.get_active_session_id()?;
    let max_entries = args
        .get("maxEntries")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as u32;

    let logs = service
        .get_console_logs(&session_id, Some(max_entries))
        .await?;

    let formatted = format_logs_as_text(&logs);
    Ok(MCPResult::success(&formatted))
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
