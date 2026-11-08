//! Tool execution policy: decides whether a tool call runs, needs a standard or hard approval,
//! runs as root in a managed container, or is blocked.

mod channel;
mod config;
mod mcp_server;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::agent::state::PendingApprovalKind;
use crate::mcp::builtin::workspace::code_execution::shell::policy::{
    evaluate_shell_policy, is_shell_tool_name, ShellPolicyAction, ShellPolicyContext,
};
use crate::services::as_root_requested;

use config::{
    get_tool_approvals_config, requires_approval_by_config, requires_hard_approval_by_config,
};
use mcp_server::{
    invalid_mcp_server_transport, is_catalog_preset_install, is_mcp_server_mutation,
    mcp_server_transport_summary,
};

pub use channel::{
    build_channel_permission_description, build_channel_permission_input_preview,
    find_pending_approval_tool_call_id, generate_channel_permission_request_id,
    parse_channel_permission_behavior,
};
pub use config::ToolApprovalsConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionPolicyDecision {
    Allow,
    RequireApproval(ToolApprovalRequest),
    RequireHardApproval(ToolApprovalRequest),
    /// A root command that needs at most a standard approval without root. Runs without approval
    /// in a LibrAgent-managed container, in every execution mode; refused before execution
    /// elsewhere (see [`root_execution_refusal`]). A root command that needs a hard approval or is
    /// blocked keeps that decision instead.
    RunAsRoot,
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

/// True for a `runShell` call that asks to run as root (see [`as_root_requested`]).
pub fn requests_root_execution(tool_name: &str, args: &serde_json::Value) -> bool {
    tool_name.rsplit("__").next() == Some("runShell") && as_root_requested(args)
}

/// Error for an entry point that runs tools without an approval card (the MCP endpoint and
/// direct builtin calls): a root command needs the user's approval in the app.
pub const ROOT_EXECUTION_NEEDS_APPROVAL: &str =
    "runShell with asRoot needs the user's approval in a LibrAgent agent session; this entry point cannot ask for it";

/// The refusal for a root request that `session` can never run (a host session or an attached
/// container), whatever its approval tier. `None` when the call does not ask for root or the
/// session uses a LibrAgent-managed container. A `session` of `None` (the session is not loaded
/// in memory) is refused as a host session.
pub fn root_execution_refusal(
    tool_name: &str,
    args: &serde_json::Value,
    session: Option<&crate::repositories::SessionMetadata>,
) -> Option<crate::mcp::types::MCPResult> {
    if !requests_root_execution(tool_name, args) {
        return None;
    }
    crate::mcp::builtin::workspace::utils::root_exec_blocker_for(session)
        .map(crate::mcp::builtin::workspace::utils::root_exec_unavailable_result)
}

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

        // In a managed container root runs without the standard approval card (#2427), but keeps
        // the hard approval and the block that the same command gets without root.
        let as_root = requests_root_execution(tool_name, args);

        return match decision.action {
            ShellPolicyAction::Allow | ShellPolicyAction::RequireApproval if as_root => {
                ToolExecutionPolicyDecision::RunAsRoot
            }
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
            // A blocked root command stays blocked in Unsafe mode too.
            ShellPolicyAction::Block => ToolExecutionPolicyDecision::Block(BlockedToolExecution {
                message: decision.reason,
                enforced_in_unsafe: decision.enforced_in_unsafe || as_root,
            }),
        };
    }

    if let Some(message) = invalid_mcp_server_transport(tool_name, args) {
        return ToolExecutionPolicyDecision::Block(BlockedToolExecution {
            message,
            enforced_in_unsafe: false,
        });
    }

    let preset_install = is_catalog_preset_install(tool_name, args);
    let requires_approval = if preset_install {
        requires_approval_by_config(get_tool_approvals_config().await, tool_name)
    } else {
        requires_approval
    };

    if requires_hard_approval && !preset_install {
        let reused = if is_mcp_server_mutation(tool_name) {
            crate::mcp::builtin::tool::secrets_reused_by(args).await
        } else {
            Vec::new()
        };
        let preview_source = mcp_server_transport_summary(tool_name, args, &reused)
            .unwrap_or_else(|| args.to_string());
        return ToolExecutionPolicyDecision::RequireHardApproval(ToolApprovalRequest {
            description: build_channel_permission_description(tool_name, &preview_source),
            input_preview: build_channel_permission_input_preview(&preview_source),
        });
    }

    if requires_approval {
        // A server transport may hold a plain secret, so its preview is the masked summary.
        let preview_source =
            mcp_server_transport_summary(tool_name, args, &[]).unwrap_or_else(|| args.to_string());
        return ToolExecutionPolicyDecision::RequireApproval(ToolApprovalRequest {
            description: build_channel_permission_description(tool_name, &preview_source),
            input_preview: build_channel_permission_input_preview(&preview_source),
        });
    }

    ToolExecutionPolicyDecision::Allow
}
