use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CHANNEL_PERMISSION_ID_ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
const CHANNEL_PERMISSION_ID_LENGTH: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolApprovalsConfig {
    #[serde(default)]
    pub requires_approval: Vec<String>,
}

pub async fn is_approval_required(tool_name: &str) -> bool {
    // Determine path
    let config_path = crate::commands::workspace_commands::get_app_data_dir()
        .await
        .map(|p| PathBuf::from(p).join("tool_approvals.json"))
        .unwrap_or_else(|_| PathBuf::from("tool_approvals.json"));

    // fallback: default sensitive tools list
    let default_config_str = include_str!("../mcp/builtin/workspace/sensitive_tools.json");

    let config: ToolApprovalsConfig = if let Ok(content) = fs::read_to_string(&config_path) {
        serde_json::from_str(&content)
            .unwrap_or_else(|_| serde_json::from_str(default_config_str).unwrap_or_default())
    } else {
        serde_json::from_str(default_config_str).unwrap_or_default()
    };

    for pattern in &config.requires_approval {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            if tool_name.starts_with(prefix) {
                return true;
            }
        } else if pattern == tool_name {
            return true;
        }
    }
    // Default false if no match
    false
}

pub fn generate_channel_permission_request_id() -> String {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    let mut output = String::with_capacity(CHANNEL_PERMISSION_ID_LENGTH);

    for byte in bytes.iter().take(CHANNEL_PERMISSION_ID_LENGTH) {
        let index = (*byte as usize) % CHANNEL_PERMISSION_ID_ALPHABET.len();
        output.push(CHANNEL_PERMISSION_ID_ALPHABET[index] as char);
    }

    output
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
    let mut preview = String::new();
    let mut chars = trimmed.chars();

    for _ in 0..MAX_CHARS {
        if let Some(ch) = chars.next() {
            preview.push(ch);
        } else {
            return preview;
        }
    }

    if chars.next().is_some() {
        preview.push('…');
    }

    preview
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_config_loads_properly() {
        let default_config_str = include_str!("../mcp/builtin/workspace/sensitive_tools.json");
        let config: ToolApprovalsConfig =
            serde_json::from_str(default_config_str).unwrap_or_default();

        assert!(!config.requires_approval.is_empty());
        assert!(config
            .requires_approval
            .contains(&"workspace__writeFile".to_string()));
    }

    #[test]
    fn test_pattern_matching() {
        let config = ToolApprovalsConfig {
            requires_approval: vec![
                "workspace__writeFile".to_string(),
                "filesystem__*".to_string(),
            ],
        };

        // Exact match
        let mut requires = false;
        for pattern in &config.requires_approval {
            if pattern == "workspace__writeFile" {
                requires = true;
            }
        }
        assert!(requires);

        // Wildcard match
        requires = false;
        let tool_name = "filesystem__deleteFile";
        for pattern in &config.requires_approval {
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if tool_name.starts_with(prefix) {
                    requires = true;
                }
            }
        }
        assert!(requires);

        // No match
        requires = false;
        let tool_name = "browser__navigate";
        for pattern in &config.requires_approval {
            if pattern == tool_name {
                requires = true;
            }
        }
        assert!(!requires);
    }
}
