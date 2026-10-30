use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::OnceCell;

use crate::agent::state::PendingApprovalKind;
use crate::mcp::builtin::workspace::code_execution::shell::policy::{
    evaluate_shell_policy, is_shell_tool_name, ShellPolicyAction, ShellPolicyContext,
};

static TOOL_APPROVALS_CONFIG: OnceCell<ToolApprovalsConfig> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolApprovalsConfig {
    #[serde(default)]
    pub requires_approval: Vec<String>,
    /// Tools that still require human confirmation in YOLO mode (not in unsafe).
    #[serde(default)]
    pub requires_hard_approval: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionPolicyDecision {
    Allow,
    RequireApproval(ToolApprovalRequest),
    RequireHardApproval(ToolApprovalRequest),
    /// Runs as root. Every execution mode asks the user, also YOLO and Unsafe.
    RequireRootApproval(ToolApprovalRequest),
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
    /// Block that Unsafe mode does not bypass.
    pub enforced_in_unsafe: bool,
}

pub fn approval_request_for_runtime(
    decision: &ToolExecutionPolicyDecision,
    yolo_enabled: bool,
    unsafe_enabled: bool,
) -> Option<&ToolApprovalRequest> {
    if let ToolExecutionPolicyDecision::RequireRootApproval(request) = decision {
        return Some(request);
    }
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
    match decision {
        ToolExecutionPolicyDecision::Block(blocked)
            if !unsafe_enabled || blocked.enforced_in_unsafe =>
        {
            Some(blocked)
        }
        _ => None,
    }
}

pub fn pending_approval_is_auto_approvable_in_yolo(approval_kind: PendingApprovalKind) -> bool {
    matches!(approval_kind, PendingApprovalKind::Standard)
}

pub fn pending_approval_is_auto_approvable_in_unsafe(approval_kind: PendingApprovalKind) -> bool {
    !matches!(approval_kind, PendingApprovalKind::Root)
}

/// True for a `runShell` call with `asRoot: true`.
pub fn requests_root_execution(tool_name: &str, args: &serde_json::Value) -> bool {
    tool_name.rsplit("__").next() == Some("runShell")
        && args.get("asRoot").and_then(|value| value.as_bool()) == Some(true)
}

/// Error for an entry point that runs tools without an approval card (the MCP endpoint and
/// direct builtin calls): a root command needs the user's approval in the app.
pub const ROOT_EXECUTION_NEEDS_APPROVAL: &str =
    "runShell with asRoot needs the user's approval in a LibrAgent agent session; this entry point cannot ask for it";

pub async fn is_approval_required(tool_name: &str) -> bool {
    let config = get_tool_approvals_config().await;
    requires_approval_by_config(config, tool_name)
        || requires_hard_approval_by_config(config, tool_name)
}

pub async fn is_hard_approval_required(tool_name: &str) -> bool {
    let config = get_tool_approvals_config().await;
    requires_hard_approval_by_config(config, tool_name)
}

pub async fn evaluate_tool_execution_policy(
    tool_name: &str,
    args: &serde_json::Value,
) -> ToolExecutionPolicyDecision {
    let requires_hard_approval = is_hard_approval_required(tool_name).await;
    let requires_approval = is_approval_required(tool_name).await;

    if is_shell_tool_name(tool_name) {
        // runShell accepts `script` as an alternative to `command`; evaluate whichever was sent.
        let command = args
            .get("command")
            .and_then(|value| value.as_str())
            .or_else(|| args.get("script").and_then(|value| value.as_str()))
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

        if requests_root_execution(tool_name, args) {
            return match decision.action {
                // A blocked root command stays blocked in Unsafe mode too.
                ShellPolicyAction::Block => {
                    ToolExecutionPolicyDecision::Block(BlockedToolExecution {
                        message: decision.reason,
                        enforced_in_unsafe: true,
                    })
                }
                _ => ToolExecutionPolicyDecision::RequireRootApproval(ToolApprovalRequest {
                    description: format!("Run as root: {}", decision.description),
                    input_preview: decision.input_preview,
                }),
            };
        }

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
                enforced_in_unsafe: decision.enforced_in_unsafe,
            }),
        };
    }

    if let Some(message) = invalid_mcp_server_transport(tool_name, args) {
        return ToolExecutionPolicyDecision::Block(BlockedToolExecution {
            message,
            enforced_in_unsafe: false,
        });
    }

    if requires_hard_approval && !is_catalog_preset_install(tool_name, args) {
        let arguments = args.to_string();
        let preview_source = mcp_server_transport_summary(tool_name, args).unwrap_or(arguments);
        return ToolExecutionPolicyDecision::RequireHardApproval(ToolApprovalRequest {
            description: build_channel_permission_description(tool_name, &preview_source),
            input_preview: build_channel_permission_input_preview(&preview_source),
        });
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

fn requires_approval_by_config(config: &ToolApprovalsConfig, tool_name: &str) -> bool {
    pattern_list_matches(&config.requires_approval, tool_name)
}

fn requires_hard_approval_by_config(config: &ToolApprovalsConfig, tool_name: &str) -> bool {
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

fn is_mcp_server_mutation(tool_name: &str) -> bool {
    tool_name == "tool__registerServer" || tool_name == "tool__updateServer"
}

/// Owner policy (#2180): registering a curated catalog preset with its exact
/// transport needs no hard approval; any other source still does. Must agree
/// with `install_provenance` so the stored provenance matches the gate taken.
fn is_catalog_preset_install(tool_name: &str, args: &serde_json::Value) -> bool {
    tool_name == "tool__registerServer"
        && args.get("authentication").is_none()
        && args
            .get("transport")
            .and_then(crate::mcp::builtin::tool::matching_catalog_preset)
            .is_some()
}

/// Rejects malformed transports before the user is asked to approve a call
/// that would fail anyway.
fn invalid_mcp_server_transport(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    if !is_mcp_server_mutation(tool_name) {
        return None;
    }

    const EXPECTED: &str = "Expected stdio: {\"type\":\"stdio\",\"command\":\"npx\",\"args\":[\"-y\",\"<package>\"]} or http: {\"type\":\"http\",\"url\":\"https://...\"}. Fix the transport and call again; the user has not been asked to approve yet.";

    let transport = match args.get("transport") {
        Some(value) => value,
        None => return Some(format!("Missing 'transport'. {EXPECTED}")),
    };

    match serde_json::from_value::<crate::mcp::types::TransportConfig>(transport.clone()) {
        Ok(crate::mcp::types::TransportConfig::Stdio { command, .. })
            if command.trim().is_empty() =>
        {
            Some(format!(
                "stdio transport has an empty 'command'. {EXPECTED}"
            ))
        }
        Ok(crate::mcp::types::TransportConfig::Http { url, .. }) if url.trim().is_empty() => {
            Some(format!("http transport has an empty 'url'. {EXPECTED}"))
        }
        Ok(_) => None,
        Err(error) => Some(format!("Invalid transport: {error}. {EXPECTED}")),
    }
}

/// Leads with the launch command or URL so truncated relay previews still show
/// what will run. Env and header values are omitted because previews can be
/// forwarded to external channels.
fn mcp_server_transport_summary(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    if !is_mcp_server_mutation(tool_name) {
        return None;
    }

    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("?");
    let transport = args.get("transport")?;
    let transport_type = transport
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");

    let object_keys = |field: &str| -> Option<String> {
        let keys = transport
            .get(field)?
            .as_object()?
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        (!keys.is_empty()).then(|| keys.join(","))
    };

    let summary = if transport_type == "stdio" {
        let command = transport
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let args_line = transport
            .get("args")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let mut line = format!("[{name}] stdio: {command}");
        if !args_line.is_empty() {
            line.push(' ');
            line.push_str(&args_line);
        }
        if let Some(env_keys) = object_keys("env") {
            line.push_str(&format!(" | env keys: {env_keys}"));
        }
        line
    } else {
        let url = transport.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let mut line = format!("[{name}] {transport_type}: {url}");
        if let Some(header_keys) = object_keys("headers") {
            line.push_str(&format!(" | header keys: {header_keys}"));
        }
        line
    };

    Some(summary)
}

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
        assert!(config
            .requires_approval
            .contains(&"workspace__editObject".to_string()));
        assert!(config
            .requires_approval
            .contains(&"scheduled_task__createScheduledTask".to_string()));
        assert!(config
            .requires_approval
            .contains(&"scheduled_task__scheduleCallback".to_string()));
        assert!(config
            .requires_approval
            .contains(&"scheduled_task__updateScheduledTask".to_string()));
        assert!(config
            .requires_approval
            .contains(&"scheduled_task__toggleScheduledTask".to_string()));
        assert!(config
            .requires_approval
            .contains(&"scheduled_task__deleteScheduledTask".to_string()));
        assert!(config
            .requires_hard_approval
            .contains(&"media__deployAssistPlugin".to_string()));
        assert!(!config
            .requires_approval
            .contains(&"media__deployAssistPlugin".to_string()));
    }

    #[tokio::test]
    async fn browser_create_session_allows_without_special_profile_gate() {
        let decision = evaluate_tool_execution_policy(
            "browser__createSession",
            &serde_json::json!({
                "url": "https://example.com"
            }),
        )
        .await;

        assert!(matches!(decision, ToolExecutionPolicyDecision::Allow));
    }

    #[tokio::test]
    async fn run_shell_script_is_evaluated_by_shell_policy() {
        let decision = evaluate_tool_execution_policy(
            "workspace__runShell",
            &serde_json::json!({ "script": "echo start\neval \"$PAYLOAD\"\n" }),
        )
        .await;

        match decision {
            ToolExecutionPolicyDecision::RequireHardApproval(request) => {
                assert!(
                    request.input_preview.contains("eval"),
                    "preview should show the script: {}",
                    request.input_preview
                );
            }
            _ => panic!("script with eval must require hard approval"),
        }
    }

    #[tokio::test]
    async fn scheduled_task_mutating_tools_require_approval() {
        for tool_name in [
            "scheduled_task__createScheduledTask",
            "scheduled_task__scheduleCallback",
            "scheduled_task__updateScheduledTask",
            "scheduled_task__toggleScheduledTask",
            "scheduled_task__deleteScheduledTask",
        ] {
            assert!(
                is_approval_required(tool_name).await,
                "{tool_name} should require approval"
            );
        }

        for tool_name in [
            "scheduled_task__listScheduledTasks",
            "scheduled_task__getScheduledTask",
        ] {
            assert!(
                !is_approval_required(tool_name).await,
                "{tool_name} should not require approval"
            );
        }
    }

    #[tokio::test]
    async fn scheduled_task_create_policy_returns_require_approval() {
        let decision = evaluate_tool_execution_policy(
            "scheduled_task__createScheduledTask",
            &serde_json::json!({
                "name": "Daily sync",
                "cronExpression": "0 9 * * *",
                "assistantId": "assistant-1",
                "message": "Run daily sync"
            }),
        )
        .await;

        assert!(matches!(
            decision,
            ToolExecutionPolicyDecision::RequireApproval(_)
        ));
    }

    #[tokio::test]
    async fn mcp_server_mutations_require_hard_approval() {
        for tool_name in ["tool__registerServer", "tool__updateServer"] {
            assert!(
                is_hard_approval_required(tool_name).await,
                "{tool_name} should require hard approval"
            );
        }

        for tool_name in [
            "tool__listServers",
            "tool__verifyServer",
            "tool__deleteServer",
        ] {
            assert!(
                !is_hard_approval_required(tool_name).await,
                "{tool_name} should not require hard approval"
            );
        }
    }

    #[tokio::test]
    async fn register_server_preview_leads_with_command_and_hides_env_values() {
        let decision = evaluate_tool_execution_policy(
            "tool__registerServer",
            &serde_json::json!({
                "name": "stocks",
                "description": "x".repeat(400),
                "transport": {
                    "type": "stdio",
                    "command": "npx",
                    "args": ["-y", "@fre4x/stocks@latest"],
                    "env": { "API_KEY": "super-secret-value" }
                }
            }),
        )
        .await;

        let ToolExecutionPolicyDecision::RequireHardApproval(request) = decision else {
            panic!("expected hard approval, got {decision:?}");
        };
        assert!(request
            .input_preview
            .starts_with("[stocks] stdio: npx -y @fre4x/stocks@latest"));
        assert!(request.input_preview.contains("env keys: API_KEY"));
        assert!(!request.input_preview.contains("super-secret-value"));
    }

    #[tokio::test]
    async fn register_server_skips_hard_approval_only_for_exact_catalog_preset() {
        let catalog = serde_json::json!({
            "name": "jules",
            "transport": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@fre4x/jules@latest"],
                "env": { "JULES_API_KEY": "user-key" }
            }
        });
        let decision = evaluate_tool_execution_policy("tool__registerServer", &catalog).await;
        assert!(
            !matches!(
                decision,
                ToolExecutionPolicyDecision::RequireHardApproval(_)
            ),
            "catalog preset should not need hard approval, got {decision:?}"
        );

        let mut arbitrary = catalog.clone();
        arbitrary["transport"]["args"] = serde_json::json!(["-y", "some-random-package"]);
        let mut with_auth = catalog.clone();
        with_auth["authentication"] = serde_json::json!({ "type": "oauth2.1" });
        for args in [arbitrary, with_auth] {
            let decision = evaluate_tool_execution_policy("tool__registerServer", &args).await;
            assert!(
                matches!(
                    decision,
                    ToolExecutionPolicyDecision::RequireHardApproval(_)
                ),
                "non-catalog install must need hard approval, got {decision:?}"
            );
        }

        let decision = evaluate_tool_execution_policy("tool__updateServer", &catalog).await;
        assert!(matches!(
            decision,
            ToolExecutionPolicyDecision::RequireHardApproval(_)
        ));
    }

    #[tokio::test]
    async fn register_server_with_malformed_transport_is_blocked_before_approval() {
        let missing_command = serde_json::json!({
            "name": "everything",
            "description": "test",
            "transport": {
                "type": "stdio",
                "args": ["-y", "@modelcontextprotocol/server-everything"],
                "env": {},
                "url": "",
                "enableSSE": false
            }
        });
        let empty_url = serde_json::json!({
            "name": "remote",
            "transport": { "type": "http", "url": " " }
        });

        for (tool_name, args) in [
            ("tool__registerServer", missing_command),
            ("tool__updateServer", empty_url),
        ] {
            let decision = evaluate_tool_execution_policy(tool_name, &args).await;
            let ToolExecutionPolicyDecision::Block(blocked) = decision else {
                panic!("{tool_name}: expected block, got {decision:?}");
            };
            assert!(blocked.message.contains("not been asked to approve"));
        }
    }

    #[tokio::test]
    async fn update_server_http_preview_shows_url_and_header_keys() {
        let decision = evaluate_tool_execution_policy(
            "tool__updateServer",
            &serde_json::json!({
                "name": "remote",
                "transport": {
                    "type": "http",
                    "url": "https://mcp.example.com/mcp",
                    "headers": { "Authorization": "Bearer token" }
                }
            }),
        )
        .await;

        let ToolExecutionPolicyDecision::RequireHardApproval(request) = decision else {
            panic!("expected hard approval, got {decision:?}");
        };
        assert_eq!(
            request.input_preview,
            "[remote] http: https://mcp.example.com/mcp | header keys: Authorization"
        );
    }

    #[test]
    fn test_pattern_matching() {
        let config = ToolApprovalsConfig {
            requires_approval: vec![
                "workspace__writeFile".to_string(),
                "filesystem__*".to_string(),
            ],
            requires_hard_approval: vec![],
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

    #[test]
    fn unsafe_mode_skips_blocks_except_enforced_ones() {
        let bypassable = ToolExecutionPolicyDecision::Block(BlockedToolExecution {
            message: "blocked".to_string(),
            enforced_in_unsafe: false,
        });
        let enforced = ToolExecutionPolicyDecision::Block(BlockedToolExecution {
            message: "blocked".to_string(),
            enforced_in_unsafe: true,
        });

        assert!(blocked_execution_for_runtime(&bypassable, false).is_some());
        assert!(blocked_execution_for_runtime(&bypassable, true).is_none());
        assert!(blocked_execution_for_runtime(&enforced, true).is_some());
    }

    #[tokio::test]
    async fn a_root_command_asks_for_approval_in_every_mode() {
        let decision = evaluate_tool_execution_policy(
            "workspace__runShell",
            &serde_json::json!({ "command": "apt-get install -y nodejs", "asRoot": true }),
        )
        .await;
        let ToolExecutionPolicyDecision::RequireRootApproval(request) = &decision else {
            panic!("asRoot must require root approval, got {decision:?}");
        };
        assert!(request.description.starts_with("Run as root"));

        for (yolo, unsafe_mode) in [(false, false), (true, false), (false, true), (true, true)] {
            assert!(
                approval_request_for_runtime(&decision, yolo, unsafe_mode).is_some(),
                "yolo={yolo} unsafe={unsafe_mode}"
            );
        }
        assert!(!pending_approval_is_auto_approvable_in_yolo(
            PendingApprovalKind::Root
        ));
        assert!(!pending_approval_is_auto_approvable_in_unsafe(
            PendingApprovalKind::Root
        ));
    }

    #[tokio::test]
    async fn a_blocked_root_command_stays_blocked_in_unsafe_mode() {
        let decision = evaluate_tool_execution_policy(
            "workspace__runShell",
            &serde_json::json!({ "command": "rm -rf /", "asRoot": true }),
        )
        .await;
        assert!(blocked_execution_for_runtime(&decision, true).is_some());
    }

    #[test]
    fn only_run_shell_with_as_root_true_requests_root() {
        let root = serde_json::json!({ "command": "id", "asRoot": true });
        assert!(requests_root_execution("workspace__runShell", &root));
        assert!(requests_root_execution("runShell", &root));
        assert!(!requests_root_execution("workspace__spawnProcess", &root));
        assert!(!requests_root_execution(
            "workspace__runShell",
            &serde_json::json!({ "command": "id", "asRoot": false })
        ));
    }
}
