//! Saving and deleting MCP server rows, with their secrets and verification.

mod container_check;
mod secrets;
mod server_row;

use crate::mcp::secret_templates::{secret_store, SecretStore};
use crate::mcp::session_isolation::exec_target::StdioExecTarget;
use crate::mcp::types::MCPServerConfig;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::services::{McpServerService, SaveVerification};
use crate::state::get_mcp_server_repository;
pub(super) use container_check::ContainerCheck;
use container_check::{
    check_in_container, tests_in_container, verification_plan, VerificationPlan,
};
use secrets::{ask_secret, plan_secrets};
use server_row::{find_existing, write_server, WrittenServer};
use std::collections::BTreeSet;

pub(super) struct SavedServer {
    pub id: String,
    /// Name the row is stored under; tool calls must use this, not the requested name.
    pub name: String,
    /// Set when the save was tested in a session container instead of the global probe.
    pub container_check: Option<ContainerCheck>,
    /// Names of the `{{secret:NAME}}` values the user entered, and of those reused.
    pub entered_secrets: Vec<String>,
    pub reused_secrets: Vec<String>,
    /// Set when the values were stored without the OS keychain.
    pub storage_warning: Option<String>,
}

impl SavedServer {
    /// Result line listing the secret names only, never a value.
    pub fn secrets_line(&self) -> String {
        let mut parts = Vec::new();
        if !self.entered_secrets.is_empty() {
            parts.push(format!("entered {}", self.entered_secrets.join(", ")));
        }
        if !self.reused_secrets.is_empty() {
            parts.push(format!("reused {}", self.reused_secrets.join(", ")));
        }
        let mut line = if parts.is_empty() {
            String::new()
        } else {
            format!("\n• Secrets: {}", parts.join("; "))
        };
        if let Some(warning) = &self.storage_warning {
            line.push_str(&format!("\n• Warning: {warning}"));
        }
        line
    }
}

/// Names of the stored secrets that saving `config` would reuse without asking, by the
/// same rule as [`save_server_config`]. Empty when the lookup fails.
pub(super) async fn reused_secret_names(
    repo: &dyn MCPServerRepository,
    store: &dyn SecretStore,
    config: &MCPServerConfig,
    reenter: &BTreeSet<String>,
) -> Vec<String> {
    let Some(raw_name) = config.name.as_deref() else {
        return Vec::new();
    };
    let Ok(existing) = find_existing(repo, raw_name).await else {
        return Vec::new();
    };
    plan_secrets(store, config, existing.as_ref(), reenter)
        .await
        .map(|(_, reused)| reused)
        .unwrap_or_default()
}

pub(super) async fn save_server_config(
    config: &MCPServerConfig,
    target: &StdioExecTarget,
    session_id: Option<&str>,
    reenter: &BTreeSet<String>,
) -> Result<SavedServer, String> {
    let repo = get_mcp_server_repository();
    let raw_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;
    let sanitized_name = sanitize_mcp_server_name(raw_name);

    // Save-first (pending). Connectivity runs asynchronously so registration is not blocked
    // by cold npx/uvx downloads. Callers can use tool__verifyServer for an explicit check.
    let existing = find_existing(repo, raw_name).await?;

    // Collect every value before any write, so a cancel or timeout saves nothing.
    let store = secret_store();
    let (to_ask, reused_secrets) = plan_secrets(store, config, existing.as_ref(), reenter).await?;
    let label_name = existing
        .as_ref()
        .map_or(sanitized_name.as_str(), |model| model.name.as_str());
    let mut entered = Vec::with_capacity(to_ask.len());
    for name in to_ask {
        let value = ask_secret(session_id, &name, label_name).await?;
        entered.push((name, value));
    }
    let verification = if tests_in_container(target, config) {
        SaveVerification::Unverified
    } else {
        SaveVerification::Pending
    };

    let WrittenServer {
        id,
        name: stored_name,
        target_changed: needs_probe,
        storage_warning,
    } = write_server(
        repo,
        store,
        existing,
        config,
        verification,
        &entered,
        &reused_secrets,
    )
    .await?;
    // A new value must reach the server even when the stored transport is unchanged.
    let needs_probe = needs_probe || !entered.is_empty();

    let container_check = match verification_plan(needs_probe, target, config) {
        VerificationPlan::None => None,
        VerificationPlan::BackgroundProbe => {
            McpServerService::schedule_background_probe(id.clone());
            None
        }
        VerificationPlan::SessionContainer => {
            let mut probe_config = config.clone();
            probe_config.name = Some(stored_name.clone());
            let verify = McpServerService::verify_config_at(probe_config, target, &id);
            Some(check_in_container(repo, &id, &stored_name, verify).await?)
        }
    };

    Ok(SavedServer {
        id,
        name: stored_name,
        container_check,
        entered_secrets: entered.into_iter().map(|(name, _)| name).collect(),
        reused_secrets,
        storage_warning,
    })
}

pub(super) async fn delete_server_config_db(id_or_name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();

    // Try ID first, then name (raw and sanitized for legacy whitespace rows)
    let mut server = repo.get(&id_or_name).await.map_err(|e| e.to_string())?;
    if server.is_none() {
        server = repo
            .get_by_name(&id_or_name)
            .await
            .map_err(|e| e.to_string())?;
    }
    if server.is_none() {
        let sanitized = sanitize_mcp_server_name(&id_or_name);
        if sanitized != id_or_name {
            server = repo
                .get_by_name(&sanitized)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    let server = server.ok_or_else(|| format!("MCP server '{}' not found", id_or_name))?;

    McpServerService::delete_server_config(repo, &server.id).await
}

#[cfg(test)]
mod tests;
