use reqwest::Method;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::mcp::types::{MCPContent, MCPResult};
use super::client;
use super::types::MessageSummaryOptions;

pub fn success_result(text: String, data: Value) -> MCPResult {
    MCPResult {
        content: Some(vec![MCPContent::Text {
            text,
            is_error: None,
        }]),
        structured_content: Some(data),
        is_error: Some(false),
    }
}

pub fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

pub fn resolve_parent_session_id(
    provided_parent: Option<&str>,
    caller_session_id: Option<&str>,
) -> Option<String> {
    match provided_parent
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) if value.eq_ignore_ascii_case("current") => {
            caller_session_id.map(str::to_string)
        }
        Some(value) => Some(value.to_string()),
        None => caller_session_id.map(str::to_string),
    }
}

pub fn extract_session_status(session: &Value) -> String {
    session
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "idle" | "terminated" | "failed" | "error"
    )
}

pub fn read_message_summary_options(args: &Value) -> MessageSummaryOptions {
    let summary_only = args
        .get("summaryOnly")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_raw_preview = args
        .get("includeRawPreview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let preview_limit = args
        .get("previewLimit")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 10) as usize)
        .unwrap_or(3);
    let skip_if_unchanged = args
        .get("skipIfUnchanged")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let min_interval_seconds = args
        .get("minIntervalSeconds")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(120))
        .unwrap_or(5);
    let forced_rest_seconds = args
        .get("forcedRestSeconds")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(300))
        .unwrap_or(20);
    let rapid_call_threshold = args
        .get("rapidCallThreshold")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(2, 10) as u32)
        .unwrap_or(3);

    MessageSummaryOptions {
        summary_only,
        include_raw_preview,
        preview_limit,
        skip_if_unchanged,
        min_interval_seconds,
        forced_rest_seconds,
        rapid_call_threshold,
    }
}

pub async fn wait_until_session_terminal(
    session_id: &str,
    timeout_seconds: u64,
    poll_interval_seconds: u64,
) -> Result<(Value, u64), String> {
    let timeout_seconds = timeout_seconds.clamp(5, 900);
    let poll_interval_seconds = poll_interval_seconds.clamp(1, 30);

    let started_at = Instant::now();
    let mut poll_count: u64 = 0;

    loop {
        let session = client::call_json(
                Method::GET,
                &format!("/api/sessions/{}", session_id),
                None,
                None,
            )
            .await?;

        poll_count = poll_count.saturating_add(1);
        let status = extract_session_status(&session);
        if is_terminal_status(&status) {
            return Ok((session, poll_count));
        }

        if started_at.elapsed() >= Duration::from_secs(timeout_seconds) {
            return Err(format!(
                "waitForSessionIdle timed out after {}s for session {}",
                timeout_seconds, session_id
            ));
        }

        sleep(Duration::from_secs(poll_interval_seconds)).await;
    }
}
