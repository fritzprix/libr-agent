use crate::mcp::session_isolation::exec_target::StdioExecTarget;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::services::{McpServerService, SaveVerification};
use crate::state::get_mcp_server_repository;

/// Result of testing a saved stdio server in the container of a Docker session.
pub(super) enum ContainerCheck {
    Passed { tool_count: usize },
    Failed(String),
}

pub(super) struct SavedServer {
    pub id: String,
    /// Name the row is stored under; tool calls must use this, not the requested name.
    pub name: String,
    /// Set when the save was tested in a session container instead of the global probe.
    pub container_check: Option<ContainerCheck>,
}

pub(super) async fn save_server_config(
    config: &MCPServerConfig,
    target: &StdioExecTarget,
) -> Result<SavedServer, String> {
    let repo = get_mcp_server_repository();
    let raw_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;
    let sanitized_name = sanitize_mcp_server_name(raw_name);

    // Save-first (pending). Connectivity runs asynchronously so registration is not blocked
    // by cold npx/uvx downloads. Callers can use tool__verifyServer for an explicit check.
    // Prefer sanitized name; fall back to raw for legacy rows that still contain spaces.
    let existing = match repo.get_by_name(&sanitized_name).await {
        Ok(Some(model)) => Some(model),
        Ok(None) if sanitized_name != *raw_name => repo
            .get_by_name(raw_name)
            .await
            .map_err(|e| format!("DB query error while saving server config: {}", e))?,
        Ok(None) => None,
        Err(e) => return Err(format!("DB query error while saving server config: {}", e)),
    };

    let verification = if tests_in_container(target, config) {
        SaveVerification::Unverified
    } else {
        SaveVerification::Pending
    };

    let (id, stored_name, needs_probe) = match existing {
        Some(existing) => {
            // Keep legacy display names (including whitespace) intact.
            let mut config = config.clone();
            config.name = Some(existing.name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;

            let existing_config_val: serde_json::Value = serde_json::from_str(&existing.config)
                .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
            let requires_reverification = existing_config_val.get("transport")
                != config_value.get("transport")
                || existing_config_val.get("authentication") != config_value.get("authentication");

            let updated = McpServerService::update_server_config(
                repo,
                existing.id.clone(),
                None,
                Some(config_value),
                verification,
            )
            .await?;

            (updated.id, existing.name, requires_reverification)
        }
        None => {
            let mut config = config.clone();
            config.name = Some(sanitized_name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
            let created = McpServerService::create_server_config(
                repo,
                sanitized_name,
                config_value,
                verification,
            )
            .await?;
            (created.id, created.name, true)
        }
    };

    let container_check = match verification_plan(needs_probe, target, config) {
        VerificationPlan::None => None,
        VerificationPlan::BackgroundProbe => {
            McpServerService::schedule_background_probe(id.clone());
            None
        }
        VerificationPlan::SessionContainer => {
            let mut probe_config = config.clone();
            probe_config.name = Some(stored_name.clone());
            let verify = McpServerService::verify_config_at(probe_config, target);
            Some(check_in_container(repo, &id, &stored_name, verify).await?)
        }
    };

    Ok(SavedServer {
        id,
        name: stored_name,
        container_check,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum VerificationPlan {
    None,
    /// Global probe on the host; it sets `success` or `error` on the record.
    BackgroundProbe,
    /// Test in the Docker session container; only the response gets the result.
    SessionContainer,
}

fn verification_plan(
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

fn tests_in_container(target: &StdioExecTarget, config: &MCPServerConfig) -> bool {
    matches!(config.transport, TransportConfig::Stdio { .. })
        && target.container_session().is_some()
}

/// A container result belongs to one session, so a failure must not change the global record.
/// The record was saved unverified (not `pending`: no background probe resolves it).
async fn check_in_container(
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
mod tests {
    use super::*;
    use crate::mcp::session_isolation::exec_target::test_docker_session;
    use crate::models::workspace_isolation::DockerWorkspaceConfig;
    use crate::repositories::mcp_server_repository::SqliteMCPServerRepository;
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    async fn repo() -> SqliteMCPServerRepository {
        crate::lifecycle::database::register_sqlite_vec();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        SqliteMCPServerRepository::new(db)
    }

    fn stdio_config() -> MCPServerConfig {
        serde_json::from_value(serde_json::json!({
            "name": "srv",
            "transport": { "type": "stdio", "command": "npx", "args": ["-y", "srv"] }
        }))
        .expect("config")
    }

    fn docker_target() -> StdioExecTarget {
        StdioExecTarget::from_session(test_docker_session(DockerWorkspaceConfig {
            image: Some("python:3.12-slim".to_string()),
            attach_container: None,
            workdir: None,
            manage_lifecycle: None,
            env: Default::default(),
            port_bindings: Vec::new(),
        }))
    }

    #[test]
    fn docker_session_register_tests_in_container_not_background_probe() {
        let config = stdio_config();
        assert_eq!(
            verification_plan(true, &docker_target(), &config),
            VerificationPlan::SessionContainer
        );
        assert_eq!(
            verification_plan(true, &StdioExecTarget::Host, &config),
            VerificationPlan::BackgroundProbe
        );
        assert_eq!(
            verification_plan(false, &docker_target(), &config),
            VerificationPlan::None
        );
    }

    #[test]
    fn docker_session_http_server_keeps_background_probe() {
        let config: MCPServerConfig = serde_json::from_value(serde_json::json!({
            "name": "web",
            "transport": { "type": "http", "url": "http://localhost:1/mcp" }
        }))
        .expect("config");
        assert_eq!(
            verification_plan(true, &docker_target(), &config),
            VerificationPlan::BackgroundProbe
        );
    }

    #[tokio::test]
    async fn unverified_save_never_writes_pending() {
        let repo = repo().await;
        let created = McpServerService::create_server_config(
            &repo,
            "srv".to_string(),
            serde_json::to_value(stdio_config()).unwrap(),
            SaveVerification::Unverified,
        )
        .await
        .expect("create");
        assert_eq!(created.verification_status, None);
    }

    #[tokio::test]
    async fn unverified_update_drops_the_old_cached_tools() {
        let repo = repo().await;
        let created = McpServerService::create_server_config(
            &repo,
            "srv".to_string(),
            serde_json::to_value(stdio_config()).unwrap(),
            SaveVerification::Pending,
        )
        .await
        .expect("create");
        repo.update_cached_tools(&created.id, 3, "[]".to_string())
            .await
            .expect("cache");

        let new_config = serde_json::json!({
            "name": "srv",
            "transport": { "type": "stdio", "command": "uvx", "args": ["srv"] }
        });
        let updated = McpServerService::update_server_config(
            &repo,
            created.id.clone(),
            None,
            Some(new_config),
            SaveVerification::Unverified,
        )
        .await
        .expect("update");

        assert_eq!(updated.verification_status, None);
        assert_eq!(updated.cached_tools, None);
        assert_eq!(updated.tool_count, None);
    }

    #[tokio::test]
    async fn failed_container_check_leaves_no_global_result() {
        let repo = repo().await;
        let created = McpServerService::create_server_config(
            &repo,
            "srv".to_string(),
            serde_json::to_value(stdio_config()).unwrap(),
            SaveVerification::Unverified,
        )
        .await
        .expect("create");

        let check = check_in_container(&repo, &created.id, "srv", async {
            Err("The command 'npx' is not in the session container.".to_string())
        })
        .await
        .expect("check");

        assert!(matches!(check, ContainerCheck::Failed(ref e) if e.contains("'npx'")));
        let stored = repo.get(&created.id).await.unwrap().expect("record");
        assert_eq!(stored.verification_status, None);
        assert_eq!(stored.last_verification_error, None);
        assert_eq!(stored.cached_tools, None);
        assert_eq!(stored.tool_count, None);
    }
}
