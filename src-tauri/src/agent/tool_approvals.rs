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
