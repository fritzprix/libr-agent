//! Tool approval config: loaded once from `tool_approvals.json` in the app data dir, with the
//! bundled sensitive-tools list as the fallback.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::OnceCell;

static TOOL_APPROVALS_CONFIG: OnceCell<ToolApprovalsConfig> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolApprovalsConfig {
    #[serde(default)]
    pub requires_approval: Vec<String>,
    /// Tools that still require human confirmation in YOLO mode (not in unsafe).
    #[serde(default)]
    pub requires_hard_approval: Vec<String>,
}

pub(super) async fn get_tool_approvals_config() -> &'static ToolApprovalsConfig {
    TOOL_APPROVALS_CONFIG
        .get_or_init(load_tool_approvals_config_uncached)
        .await
}

async fn load_tool_approvals_config_uncached() -> ToolApprovalsConfig {
    // Determine path
    let config_path = crate::commands::workspace_commands::get_app_data_dir()
        .await
        .map(|p| PathBuf::from(p).join("tool_approvals.json"))
        .unwrap_or_else(|_| PathBuf::from("tool_approvals.json"));

    // fallback: default sensitive tools list
    let default_config_str = include_str!("../../mcp/builtin/workspace/sensitive_tools.json");

    let config: ToolApprovalsConfig = match tokio::fs::read_to_string(&config_path).await {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(config) => config,
            Err(error) => {
                log::warn!(
                    "Failed to parse tool approval config at {}: {}. Falling back to bundled defaults.",
                    config_path.display(),
                    error
                );
                serde_json::from_str(default_config_str).unwrap_or_default()
            }
        },
        Err(error) => {
            log::debug!(
                "Tool approval config not loaded from {}: {}. Using bundled defaults.",
                config_path.display(),
                error
            );
            serde_json::from_str(default_config_str).unwrap_or_default()
        }
    };

    config
}

pub(super) fn requires_approval_by_config(config: &ToolApprovalsConfig, tool_name: &str) -> bool {
    pattern_list_matches(&config.requires_approval, tool_name)
}

pub(super) fn requires_hard_approval_by_config(
    config: &ToolApprovalsConfig,
    tool_name: &str,
) -> bool {
    pattern_list_matches(&config.requires_hard_approval, tool_name)
}

fn pattern_list_matches(patterns: &[String], tool_name: &str) -> bool {
    for pattern in patterns {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            if tool_name.starts_with(prefix) {
                return true;
            }
        } else if pattern == tool_name {
            return true;
        }
    }
    false
}
