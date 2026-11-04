use crate::agent::state::{secret_outcome_message, UserPromptOutcome, UserPromptPayload};
use crate::mcp::secret_templates::{
    parse_secret_names, same_target, secret_names_column, secret_store, template_names, SecretStore,
};
use crate::mcp::session_isolation::exec_target::StdioExecTarget;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::services::{McpServerService, SaveVerification};
use crate::state::get_mcp_server_repository;
use std::collections::BTreeSet;

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
    /// Names of the `{{secret:NAME}}` values the user entered, and of those reused.
    pub entered_secrets: Vec<String>,
    pub reused_secrets: Vec<String>,
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
        if parts.is_empty() {
            String::new()
        } else {
            format!("\n• Secrets (OS keychain): {}", parts.join("; "))
        }
    }
}

/// Which template names reuse a stored value and which the user must enter.
///
/// A value is reused only for the same server and name, when the target is unchanged
/// ([`same_target`]) and the call does not ask to enter it again.
fn plan_secrets(
    store: &dyn SecretStore,
    config: &MCPServerConfig,
    existing: Option<&crate::entity::mcp_server::Model>,
    reenter: &BTreeSet<String>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let names = template_names(&config.transport)?;
    let mut ask = Vec::new();
    let mut reuse = Vec::new();
    for name in names {
        let reusable = match existing {
            Some(model) if !reenter.contains(&name) => {
                let old_target_same = serde_json::from_str::<MCPServerConfig>(&model.config)
                    .is_ok_and(|old| same_target(&old.transport, &config.transport));
                old_target_same
                    && parse_secret_names(model.secret_names.as_deref()).contains(&name)
                    && store.get(&model.id, &name)?.is_some()
            }
            _ => false,
        };
        if reusable {
            reuse.push(name);
        } else {
            ask.push(name);
        }
    }
    Ok((ask, reuse))
}

/// Ask the user for one secret through the shared prompt (#2543). Cancel, Stop, and
/// timeout return an error, so nothing is saved.
async fn ask_secret(session_id: Option<&str>, name: &str, server: &str) -> Result<String, String> {
    let session_id = session_id.ok_or_else(|| {
        format!("The secret '{name}' must be entered by the user, but this call has no session to ask in. Nothing was saved.")
    })?;
    let manager = crate::state::try_get_channel_dispatch_agent()
        .ok_or_else(|| "The user prompt is not available yet. Nothing was saved.".to_string())?;
    let outcome = manager
        .request_user_prompt(
            session_id,
            UserPromptPayload::Secret {
                variable_name: name.to_string(),
                label: format!("{name} for MCP server '{server}'"),
                reuse_note: None,
            },
        )
        .await?;
    match outcome {
        UserPromptOutcome::Answered(value) if !value.is_empty() => Ok(value),
        UserPromptOutcome::Answered(_) => {
            Err(format!("The secret '{name}' was empty. Nothing was saved."))
        }
        other => Err(secret_outcome_message(&other, name)),
    }
}

/// Write the entered values to the store and record every used name on the row.
async fn store_secrets(
    repo: &dyn MCPServerRepository,
    store: &dyn SecretStore,
    id: &str,
    entered: &[(String, String)],
    reused: &[String],
) -> Result<(), String> {
    for (name, value) in entered {
        store.set(id, name, value)?;
    }
    let names: BTreeSet<String> = entered
        .iter()
        .map(|(name, _)| name.clone())
        .chain(reused.iter().cloned())
        .collect();
    repo.set_secret_names(id, secret_names_column(&names))
        .await
        .map_err(|e| format!("Failed to save secret names: {}", e))
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

    // Collect every value before any write, so a cancel or timeout saves nothing.
    let store = secret_store();
    let (to_ask, reused_secrets) = plan_secrets(store, config, existing.as_ref(), reenter)?;
    let label_name = existing
        .as_ref()
        .map_or(sanitized_name.as_str(), |model| model.name.as_str());
    let mut entered = Vec::with_capacity(to_ask.len());
    for name in to_ask {
        let value = ask_secret(session_id, &name, label_name).await?;
        entered.push((name, value));
    }
    let is_new = existing.is_none();

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

    if !entered.is_empty() || !reused_secrets.is_empty() {
        if let Err(error) = store_secrets(repo, store, &id, &entered, &reused_secrets).await {
            if is_new {
                let _ = McpServerService::delete_server_config(repo, &id).await;
            }
            return Err(error);
        }
    }
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

    fn exa_config(url: &str) -> MCPServerConfig {
        serde_json::from_value(serde_json::json!({
            "name": "exa",
            "transport": { "type": "http", "url": url }
        }))
        .expect("config")
    }

    const EXA_URL: &str = "https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}";

    /// Save like `save_server_config` does after the prompt: template row, then values.
    async fn saved_exa(repo: &SqliteMCPServerRepository) -> crate::entity::mcp_server::Model {
        let created = McpServerService::create_server_config(
            repo,
            "exa".to_string(),
            serde_json::to_value(exa_config(EXA_URL)).unwrap(),
            SaveVerification::Pending,
        )
        .await
        .expect("create");
        let entered = vec![("EXA_API_KEY".to_string(), "exa-secret-value".to_string())];
        store_secrets(repo, secret_store(), &created.id, &entered, &[])
            .await
            .expect("store");
        repo.get(&created.id).await.unwrap().expect("record")
    }

    #[tokio::test]
    async fn the_row_and_the_list_hold_the_template_and_names_not_the_value() {
        let repo = repo().await;
        let model = saved_exa(&repo).await;

        assert!(model.config.contains("{{secret:EXA_API_KEY}}"));
        assert_eq!(model.secret_names.as_deref(), Some(r#"["EXA_API_KEY"]"#));
        let listed = McpServerService::list_server_configs(&repo).await.unwrap();
        let row = format!("{:?}", listed);
        assert!(!row.contains("exa-secret-value"));
        assert_eq!(
            secret_store()
                .get(&model.id, "EXA_API_KEY")
                .unwrap()
                .as_deref(),
            Some("exa-secret-value")
        );
    }

    #[tokio::test]
    async fn update_reuses_a_value_only_for_the_same_target_without_reentry() {
        let repo = repo().await;
        let model = saved_exa(&repo).await;
        let none = BTreeSet::new();

        let same_host = exa_config("https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}&x=1");
        let (ask, reuse) = plan_secrets(secret_store(), &same_host, Some(&model), &none).unwrap();
        assert!(ask.is_empty());
        assert_eq!(reuse, vec!["EXA_API_KEY".to_string()]);

        let reenter = BTreeSet::from(["EXA_API_KEY".to_string()]);
        let (ask, reuse) =
            plan_secrets(secret_store(), &same_host, Some(&model), &reenter).unwrap();
        assert_eq!(ask, vec!["EXA_API_KEY".to_string()]);
        assert!(reuse.is_empty());

        for moved in [
            "http://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}",
            "https://evil.example/mcp?exaApiKey={{secret:EXA_API_KEY}}",
            "https://mcp.exa.ai:8443/mcp?exaApiKey={{secret:EXA_API_KEY}}",
        ] {
            let (ask, reuse) =
                plan_secrets(secret_store(), &exa_config(moved), Some(&model), &none).unwrap();
            assert_eq!(ask, vec!["EXA_API_KEY".to_string()], "{moved}");
            assert!(reuse.is_empty(), "{moved}");
        }

        let (ask, _) = plan_secrets(secret_store(), &exa_config(EXA_URL), None, &none).unwrap();
        assert_eq!(ask, vec!["EXA_API_KEY".to_string()]);
    }

    #[tokio::test]
    async fn a_ui_edit_to_a_plain_value_removes_the_stored_secret() {
        let repo = repo().await;
        let model = saved_exa(&repo).await;

        let plain =
            serde_json::to_value(exa_config("https://mcp.exa.ai/mcp?exaApiKey=plain")).unwrap();
        McpServerService::update_server_config(
            &repo,
            model.id.clone(),
            None,
            Some(plain),
            SaveVerification::Pending,
        )
        .await
        .expect("update");

        let updated = repo.get(&model.id).await.unwrap().expect("record");
        assert_eq!(updated.secret_names, None);
        assert_eq!(secret_store().get(&model.id, "EXA_API_KEY").unwrap(), None);
    }

    #[tokio::test]
    async fn delete_removes_the_stored_secrets() {
        let repo = repo().await;
        let model = saved_exa(&repo).await;

        McpServerService::delete_server_config(&repo, &model.id)
            .await
            .expect("delete");

        assert_eq!(secret_store().get(&model.id, "EXA_API_KEY").unwrap(), None);
    }

    #[test]
    fn the_result_line_lists_names_only() {
        let saved = SavedServer {
            id: "id".to_string(),
            name: "exa".to_string(),
            container_check: None,
            entered_secrets: vec!["EXA_API_KEY".to_string()],
            reused_secrets: vec!["OTHER".to_string()],
        };
        assert_eq!(
            saved.secrets_line(),
            "\n• Secrets (OS keychain): entered EXA_API_KEY; reused OTHER"
        );
    }
}
