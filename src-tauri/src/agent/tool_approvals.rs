use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

    // We can read it dynamically to pick up changes without restarting,
    // or cache it. Given it's a small JSON, reading it per tool execution
    // or at least when needed is fine. Let's just read it dynamically for now
    // to allow easy updates by the user.
    let config: ToolApprovalsConfig = if let Ok(content) = fs::read_to_string(&config_path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        ToolApprovalsConfig::default()
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
