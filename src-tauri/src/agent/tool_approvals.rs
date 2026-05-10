use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::sync::OnceCell;

use crate::agent::state::PendingApprovalKind;
use crate::mcp::builtin::workspace::code_execution::shell::policy::{
    evaluate_shell_policy, is_shell_tool_name, ShellPolicyAction, ShellPolicyContext,
};

const CHANNEL_PERMISSION_ID_ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
const CHANNEL_PERMISSION_ID_LENGTH: usize = 5;
static TOOL_APPROVALS_CONFIG: OnceCell<ToolApprovalsConfig> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolApprovalsConfig {
    #[serde(default)]
    pub requires_approval: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionPolicyDecision {
    Allow,
    RequireApproval(ToolApprovalRequest),
    RequireHardApproval(ToolApprovalRequest),
    Block(BlockedToolExecution),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApprovalRequest {
    pub description: String,
    pub input_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedToolExecution {
    pub message: String,
}

pub fn approval_request_for_runtime(
    decision: &ToolExecutionPolicyDecision,
    yolo_enabled: bool,
    unsafe_enabled: bool,
) -> Option<&ToolApprovalRequest> {
    if unsafe_enabled {
        return None;
    }

    match decision {
        ToolExecutionPolicyDecision::RequireApproval(request) if !yolo_enabled => Some(request),
        ToolExecutionPolicyDecision::RequireHardApproval(request) => Some(request),
        _ => None,
    }
}

pub fn blocked_execution_for_runtime(
    decision: &ToolExecutionPolicyDecision,
    unsafe_enabled: bool,
) -> Option<&BlockedToolExecution> {
    if unsafe_enabled {
        return None;
    }

    match decision {
        ToolExecutionPolicyDecision::Block(blocked) => Some(blocked),
        _ => None,
    }
}

pub fn pending_approval_is_auto_approvable_in_yolo(approval_kind: PendingApprovalKind) -> bool {
    matches!(approval_kind, PendingApprovalKind::Standard)
}

pub async fn is_approval_required(tool_name: &str) -> bool {
    let config = get_tool_approvals_config().await;
    requires_approval_by_config(config, tool_name)
}

pub async fn evaluate_tool_execution_policy(
    tool_name: &str,
    args: &serde_json::Value,
) -> ToolExecutionPolicyDecision {
    let requires_approval = is_approval_required(tool_name).await;

    if is_shell_tool_name(tool_name) {
        let command = args
            .get("command")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let environment = args
            .get("env")
            .and_then(|value| value.as_object())
            .map(|object| {
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").to_string()))
                    .collect::<HashMap<_, _>>()
            });
        let decision = evaluate_shell_policy(ShellPolicyContext {
            tool_name,
            command,
            workspace_dir: None,
            current_dir: None,
            environment: environment.as_ref(),
            force_approval: requires_approval,
        });

        return match decision.action {
            ShellPolicyAction::Allow => ToolExecutionPolicyDecision::Allow,
            ShellPolicyAction::RequireApproval => {
                ToolExecutionPolicyDecision::RequireApproval(ToolApprovalRequest {
                    description: decision.description,
                    input_preview: decision.input_preview,
                })
            }
            ShellPolicyAction::RequireHardApproval => {
                ToolExecutionPolicyDecision::RequireHardApproval(ToolApprovalRequest {
                    description: decision.description,
                    input_preview: decision.input_preview,
                })
            }
            ShellPolicyAction::Block => ToolExecutionPolicyDecision::Block(BlockedToolExecution {
                message: decision.reason,
            }),
        };
    }

    if requires_approval {
        let arguments = args.to_string();
        return ToolExecutionPolicyDecision::RequireApproval(ToolApprovalRequest {
            description: build_channel_permission_description(tool_name, &arguments),
            input_preview: build_channel_permission_input_preview(&arguments),
        });
    }

    ToolExecutionPolicyDecision::Allow
}

async fn get_tool_approvals_config() -> &'static ToolApprovalsConfig {
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
    let default_config_str = include_str!("../mcp/builtin/workspace/sensitive_tools.json");

    let config: ToolApprovalsConfig = match fs::read_to_string(&config_path) {
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

fn requires_approval_by_config(config: &ToolApprovalsConfig, tool_name: &str) -> bool {
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
