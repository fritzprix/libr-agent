//! Channel permission requests: ids, descriptions, input previews and allow/deny parsing.

use std::collections::HashMap;

pub fn generate_channel_permission_request_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub fn build_channel_permission_description(tool_name: &str, arguments: &str) -> String {
    let trimmed_arguments = arguments.trim();

    if trimmed_arguments.is_empty() {
        format!("Claude requested approval to run tool {}", tool_name)
    } else {
        format!(
            "Claude requested approval to run tool {} with the provided arguments",
            tool_name
        )
    }
}

pub fn build_channel_permission_input_preview(arguments: &str) -> String {
    const MAX_CHARS: usize = 200;

    let trimmed = arguments.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= MAX_CHARS {
        return trimmed.to_string();
    }

    let head = chars.iter().take(140).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{head} … {tail}")
}

pub fn parse_channel_permission_behavior(behavior: &str) -> Result<bool, String> {
    match behavior {
        "allow" => Ok(true),
        "deny" => Ok(false),
        other => Err(format!(
            "Invalid channel permission behavior: {} (expected 'allow' or 'deny')",
            other
        )),
    }
}

pub fn find_pending_approval_tool_call_id(
    approvals: &HashMap<String, crate::agent::state::PendingApprovalData>,
    request_id: &str,
) -> Option<String> {
    approvals.iter().find_map(|(tool_call_id, data)| {
        (data.request_id.as_deref() == Some(request_id)).then(|| tool_call_id.clone())
    })
}
