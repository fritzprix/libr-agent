//! Choosing how a saved server is verified, and testing it in a Docker session container.

use crate::mcp::session_isolation::exec_target::StdioExecTarget;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use crate::repositories::mcp_server_repository::MCPServerRepository;

/// Result of testing a saved stdio server in the container of a Docker session.
pub(in crate::mcp::builtin::tool::operations) enum ContainerCheck {
    Passed { tool_count: usize },
    Failed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum VerificationPlan {
    None,
    /// Global probe on the host; it sets `success` or `error` on the record.
    BackgroundProbe,
    /// Test in the Docker session container; only the response gets the result.
    SessionContainer,
}

pub(super) fn verification_plan(
    needs_probe: bool,
    target: &StdioExecTarget,
    config: &MCPServerConfig,
) -> VerificationPlan {
    if !needs_probe {
        return VerificationPlan::None;
    }
    if tests_in_container(target, config) {
        VerificationPlan::SessionContainer
    } else {
        VerificationPlan::BackgroundProbe
    }
}

pub(super) fn tests_in_container(target: &StdioExecTarget, config: &MCPServerConfig) -> bool {
    matches!(config.transport, TransportConfig::Stdio { .. })
        && target.container_session().is_some()
}

/// A container result belongs to one session, so a failure must not change the global record.
/// The record was saved unverified (not `pending`: no background probe resolves it).
pub(super) async fn check_in_container(
    repo: &dyn MCPServerRepository,
    id: &str,
    name: &str,
    verify: impl std::future::Future<Output = Result<Vec<crate::mcp::MCPTool>, String>>,
) -> Result<ContainerCheck, String> {
    match verify.await {
        Ok(tools) => {
            let tools_json = crate::mcp::utils::serialize_mcp_tools(&tools);
            if let Err(e) = repo
                .update_cached_tools(id, tools.len() as i32, tools_json)
                .await
            {
                log::warn!("Failed to cache tool list for '{}' ({}): {}", name, id, e);
            }
            Ok(ContainerCheck::Passed {
                tool_count: tools.len(),
            })
        }
        Err(error) => Ok(ContainerCheck::Failed(error)),
    }
}
