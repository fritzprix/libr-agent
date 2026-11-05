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

/// Which template names reuse a stored value and which the user must enter.
///
/// A value is reused only for the same server and name, when the target is unchanged
/// ([`same_target`]) and the call does not ask to enter it again.
async fn plan_secrets(
    store: &dyn SecretStore,
    config: &MCPServerConfig,
    existing: Option<&crate::entity::mcp_server::Model>,
    reenter: &BTreeSet<String>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let names = template_names(&config.transport)?;
    let mut ask = Vec::new();
    let mut reuse = Vec::new();
    let mut unreadable: Option<String> = None;
    for name in &names {
        let reusable = match existing {
            Some(model) if !reenter.contains(name) => {
                let old_target_same = serde_json::from_str::<MCPServerConfig>(&model.config)
                    .is_ok_and(|old| same_target(&old.transport, &config.transport));
                if !old_target_same
                    || !parse_secret_names(model.secret_names.as_deref()).contains(name)
                {
                    false
                } else {
                    match store.get(&model.id, name).await {
                        Ok(value) => value.is_some(),
                        // A value that cannot be read is not reused. The cause (a lost
                        // key file, a locked keychain, a database error) is reported
                        // with the full name list below.
                        Err(error) => {
                            unreadable = Some(error);
                            false
                        }
                    }
                }
            }
            _ => false,
        };
        if reusable {
            reuse.push(name.clone());
        } else {
            ask.push(name.clone());
        }
    }
    if let Some(error) = unreadable {
        let server = config.name.as_deref().unwrap_or("unnamed");
        return Err(format!(
            "The stored secret values of MCP server '{server}' cannot be read, so enter every secret again: {}. Call tool__updateServer with reenterSecrets listing all of them. Nothing was saved. Cause: {error}",
            names.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
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
        UserPromptOutcome::Answered(value) => clean_secret(&value, name),
        other => Err(secret_outcome_message(&other, name)),
    }
}

/// Trim a pasted value. A value with a control character inside (for example a line
/// break) would be dropped as an invalid header, so it is refused.
fn clean_secret(value: &str, name: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("The secret '{name}' was empty. Nothing was saved."));
    }
    if value.chars().any(char::is_control) {
        return Err(format!(
            "The secret '{name}' contains a line break or another control character. Nothing was saved."
        ));
    }
    Ok(value.to_string())
}

/// Write the values. Returns the store's warning, if any.
async fn store_values(
    store: &dyn SecretStore,
    id: &str,
    entered: &[(String, String)],
) -> Result<Option<String>, String> {
    let mut warning = None;
    for (name, value) in entered {
        if let Some(notice) = store.set(id, name, value).await? {
            warning = Some(notice);
        }
    }
    Ok(warning)
}

/// Write the entered values to the store and record every used name on the row.
async fn store_secrets(
    repo: &dyn MCPServerRepository,
    store: &dyn SecretStore,
    id: &str,
    entered: &[(String, String)],
    reused: &[String],
) -> Result<Option<String>, String> {
    let warning = store_values(store, id, entered).await?;
    record_secret_names(repo, id, entered, reused).await?;
    Ok(warning)
}

async fn record_secret_names(
    repo: &dyn MCPServerRepository,
    id: &str,
    entered: &[(String, String)],
    reused: &[String],
) -> Result<(), String> {
    let names: BTreeSet<String> = entered
        .iter()
        .map(|(name, _)| name.clone())
        .chain(reused.iter().cloned())
        .collect();
    repo.set_secret_names(id, secret_names_column(&names))
        .await
        .map_err(|e| format!("Failed to save secret names: {}", e))
}

struct WrittenServer {
    id: String,
    name: String,
    target_changed: bool,
    storage_warning: Option<String>,
}

/// Write the row and the secrets. On failure a new row is removed, and an existing row
/// keeps its previous config and secret names.
async fn write_server(
    repo: &dyn MCPServerRepository,
    store: &dyn SecretStore,
    existing: Option<crate::entity::mcp_server::Model>,
    config: &MCPServerConfig,
    verification: SaveVerification,
    entered: &[(String, String)],
    reused: &[String],
) -> Result<WrittenServer, String> {
    let has_secrets = !entered.is_empty() || !reused.is_empty();
    match existing {
        Some(existing) => {
            // Values before the row: a keychain failure must leave the row unchanged, and
            // the update below deletes the values the new config no longer uses.
            let storage_warning = store_values(store, &existing.id, entered).await?;

            // Keep legacy display names (including whitespace) intact.
            let mut config = config.clone();
            config.name = Some(existing.name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;

            let existing_config_val: serde_json::Value = serde_json::from_str(&existing.config)
                .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
            let requires_reverification = existing_config_val.get("transport")
                != config_value.get("transport")
                || existing_config_val.get("authentication") != config_value.get("authentication");

            let updated = McpServerService::update_server_config_with_store(
                repo,
                store,
                existing.id.clone(),
                None,
                Some(config_value),
                verification,
            )
            .await?;

            if has_secrets {
                if let Err(error) = record_secret_names(repo, &updated.id, entered, reused).await {
                    restore_row(repo, &existing, existing_config_val).await;
                    return Err(error);
                }
            }
            Ok(WrittenServer {
                id: updated.id,
                name: existing.name,
                target_changed: requires_reverification,
                storage_warning,
            })
        }
        None => {
            let sanitized_name = sanitize_mcp_server_name(config.name.as_deref().unwrap_or(""));
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
            let mut storage_warning = None;
            if has_secrets {
                match store_secrets(repo, store, &created.id, entered, reused).await {
                    Ok(warning) => storage_warning = warning,
                    Err(error) => {
                        let _ = McpServerService::delete_server_config_with_store(
                            repo,
                            store,
                            &created.id,
                        )
                        .await;
                        return Err(error);
                    }
                }
            }
            Ok(WrittenServer {
                id: created.id,
                name: created.name,
                target_changed: true,
                storage_warning,
            })
        }
    }
}

/// Put back the config and secret names that a failed update replaced.
async fn restore_row(
    repo: &dyn MCPServerRepository,
    previous: &crate::entity::mcp_server::Model,
    previous_config: serde_json::Value,
) {
    if let Err(e) = repo.update(&previous.id, None, Some(previous_config)).await {
        log::warn!(
            "Failed to restore config of MCP server '{}': {}",
            previous.id,
            e
        );
    }
    if let Err(e) = repo
        .set_secret_names(&previous.id, previous.secret_names.clone())
        .await
    {
        log::warn!(
            "Failed to restore secret names of MCP server '{}': {}",
            previous.id,
            e
        );
    }
}

/// The stored row for `raw_name`. Prefers the sanitized name; falls back to the raw name
/// for legacy rows that still contain spaces.
async fn find_existing(
    repo: &dyn MCPServerRepository,
    raw_name: &str,
) -> Result<Option<crate::entity::mcp_server::Model>, String> {
    let sanitized_name = sanitize_mcp_server_name(raw_name);
    let query_error = |e| format!("DB query error while saving server config: {}", e);
    match repo
        .get_by_name(&sanitized_name)
        .await
        .map_err(query_error)?
    {
        Some(model) => Ok(Some(model)),
        None if sanitized_name != raw_name => repo.get_by_name(raw_name).await.map_err(query_error),
        None => Ok(None),
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
    fn a_pasted_secret_is_trimmed_and_control_characters_are_refused() {
        assert_eq!(
            clean_secret("  ghp_abc\r\n", "GITHUB_PAT").unwrap(),
            "ghp_abc"
        );
        assert!(clean_secret(" \n", "GITHUB_PAT").is_err());
        assert!(clean_secret("ghp_\nabc", "GITHUB_PAT").is_err());
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
                .await
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
        let (ask, reuse) = plan_secrets(secret_store(), &same_host, Some(&model), &none)
            .await
            .unwrap();
        assert!(ask.is_empty());
        assert_eq!(reuse, vec!["EXA_API_KEY".to_string()]);

        let reenter = BTreeSet::from(["EXA_API_KEY".to_string()]);
        let (ask, reuse) = plan_secrets(secret_store(), &same_host, Some(&model), &reenter)
            .await
            .unwrap();
        assert_eq!(ask, vec!["EXA_API_KEY".to_string()]);
        assert!(reuse.is_empty());

        for moved in [
            "http://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}",
            "https://evil.example/mcp?exaApiKey={{secret:EXA_API_KEY}}",
            "https://mcp.exa.ai:8443/mcp?exaApiKey={{secret:EXA_API_KEY}}",
        ] {
            let (ask, reuse) =
                plan_secrets(secret_store(), &exa_config(moved), Some(&model), &none)
                    .await
                    .unwrap();
            assert_eq!(ask, vec!["EXA_API_KEY".to_string()], "{moved}");
            assert!(reuse.is_empty(), "{moved}");
        }

        let (ask, _) = plan_secrets(secret_store(), &exa_config(EXA_URL), None, &none)
            .await
            .unwrap();
        assert_eq!(ask, vec!["EXA_API_KEY".to_string()]);
    }

    #[tokio::test]
    async fn the_preview_lookup_names_the_secrets_an_update_reuses() {
        let repo = repo().await;
        saved_exa(&repo).await;
        let none = BTreeSet::new();

        let reused = reused_secret_names(&repo, secret_store(), &exa_config(EXA_URL), &none).await;
        assert_eq!(reused, vec!["EXA_API_KEY".to_string()]);

        let moved = exa_config("https://evil.example/mcp?exaApiKey={{secret:EXA_API_KEY}}");
        assert!(reused_secret_names(&repo, secret_store(), &moved, &none)
            .await
            .is_empty());
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
        assert_eq!(
            secret_store().get(&model.id, "EXA_API_KEY").await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn delete_removes_the_stored_secrets() {
        let repo = repo().await;
        let model = saved_exa(&repo).await;

        McpServerService::delete_server_config(&repo, &model.id)
            .await
            .expect("delete");

        assert_eq!(
            secret_store().get(&model.id, "EXA_API_KEY").await.unwrap(),
            None
        );
    }

    struct SetFailsStore;
    #[async_trait::async_trait]
    impl SecretStore for SetFailsStore {
        async fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        async fn set(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
            Err("keychain is locked".to_string())
        }
        async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_failed_keychain_write_on_update_leaves_the_row_as_it_was() {
        let repo = repo().await;
        let model = saved_exa(&repo).await;
        let entered = vec![("OTHER_KEY".to_string(), "new-value".to_string())];
        let new_config = exa_config("https://mcp.exa.ai/v2?otherKey={{secret:OTHER_KEY}}");

        let error = write_server(
            &repo,
            &SetFailsStore,
            Some(model.clone()),
            &new_config,
            SaveVerification::Pending,
            &entered,
            &[],
        )
        .await
        .err()
        .expect("write fails");

        assert!(error.contains("keychain is locked"), "{error}");
        let stored = repo.get(&model.id).await.unwrap().expect("record");
        assert_eq!(stored.config, model.config);
        assert_eq!(stored.secret_names, model.secret_names);
        assert_eq!(
            secret_store()
                .get(&model.id, "EXA_API_KEY")
                .await
                .unwrap()
                .as_deref(),
            Some("exa-secret-value")
        );
    }

    #[tokio::test]
    async fn a_failed_keychain_write_on_create_removes_the_new_row() {
        let repo = repo().await;
        let entered = vec![("EXA_API_KEY".to_string(), "value".to_string())];

        let result = write_server(
            &repo,
            &SetFailsStore,
            None,
            &exa_config(EXA_URL),
            SaveVerification::Pending,
            &entered,
            &[],
        )
        .await;

        assert!(result.is_err());
        assert!(repo.get_by_name("exa").await.unwrap().is_none());
    }

    fn value_at<'a>(config: &'a serde_json::Value, pointer: &str) -> &'a str {
        config
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn the_github_bearer_preset_saves_the_value_and_starts_with_it() {
        let repo = repo().await;
        let config: MCPServerConfig = serde_json::from_value(serde_json::json!({
            "name": "github",
            "transport": {
                "type": "http",
                "url": "https://api.githubcopilot.com/mcp/",
                "headers": { "Authorization": "Bearer {{secret:GITHUB_PAT}}" }
            }
        }))
        .expect("config");
        let entered = vec![("GITHUB_PAT".to_string(), "ghp_value".to_string())];

        let written = write_server(
            &repo,
            secret_store(),
            None,
            &config,
            SaveVerification::Pending,
            &entered,
            &[],
        )
        .await
        .expect("write");

        let row = repo.get(&written.id).await.unwrap().expect("record");
        assert!(row.config.contains("Bearer {{secret:GITHUB_PAT}}"));
        assert!(!row.config.contains("ghp_value"));
        let filled =
            crate::mcp::secret_templates::fill_secrets(secret_store(), &written.id, &config)
                .await
                .expect("filled");
        let started = serde_json::to_value(&filled.config).unwrap();
        assert_eq!(
            value_at(&started, "/transport/headers/Authorization"),
            "Bearer ghp_value"
        );
    }

    #[tokio::test]
    async fn without_a_keychain_a_save_starts_and_updates_through_the_encrypted_column() {
        use crate::mcp::secret_fallback::{
            FileSecretStore, LayeredSecretStore, FALLBACK_WARNING, KEY_FILE_NAME,
        };
        use crate::mcp::secret_templates::{fill_secrets, MemorySecretStore};

        let repo = repo().await;
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::unavailable();
        let store = LayeredSecretStore::new(
            &keychain,
            FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
        );
        let entered = vec![("EXA_API_KEY".to_string(), "exa-secret-value".to_string())];
        let write = |config: MCPServerConfig| {
            let (store, repo, entered) = (&store, &repo, &entered);
            async move {
                write_server(
                    repo,
                    store,
                    None,
                    &config,
                    SaveVerification::Pending,
                    entered,
                    &[],
                )
                .await
                .expect("write")
            }
        };

        let first = write(exa_config(EXA_URL)).await;
        let mut other = exa_config(EXA_URL);
        other.name = Some("exa2".to_string());
        let second = write(other).await;

        assert_eq!(first.storage_warning.as_deref(), Some(FALLBACK_WARNING));
        assert_eq!(second.storage_warning, None, "the warning shows once");
        let row = repo.get(&first.id).await.unwrap().expect("record");
        let sealed = row.encrypted_secrets.clone().expect("encrypted_secrets");
        assert!(!sealed.contains("exa-secret-value"));
        assert!(!row.config.contains("exa-secret-value"));
        assert_eq!(keychain.get(&first.id, "EXA_API_KEY").await.unwrap(), None);

        let filled = fill_secrets(&store, &first.id, &exa_config(EXA_URL))
            .await
            .expect("start decrypts");
        let started = serde_json::to_value(&filled.config).unwrap();
        assert_eq!(
            value_at(&started, "/transport/url"),
            "https://mcp.exa.ai/mcp?exaApiKey=exa-secret-value"
        );

        let plain =
            serde_json::to_value(exa_config("https://mcp.exa.ai/mcp?exaApiKey=plain")).unwrap();
        McpServerService::update_server_config_with_store(
            &repo,
            &store,
            first.id.clone(),
            None,
            Some(plain),
            SaveVerification::Pending,
        )
        .await
        .expect("update");
        let updated = repo.get(&first.id).await.unwrap().expect("record");
        assert_eq!(updated.encrypted_secrets, None);
        assert_eq!(updated.secret_names, None);

        McpServerService::delete_server_config_with_store(&repo, &store, &second.id)
            .await
            .expect("delete");
        assert!(repo.get(&second.id).await.unwrap().is_none());
    }

    /// A key file that is gone leaves every name of the row unreadable, so all of them must
    /// be entered again, and doing that must recover the row.
    #[tokio::test]
    async fn a_lost_key_file_needs_every_name_entered_again() {
        use crate::mcp::secret_fallback::{FileSecretStore, KEY_FILE_NAME};

        let two_secrets = "https://mcp.exa.ai/mcp?a={{secret:EXA_API_KEY}}&b={{secret:OTHER_KEY}}";
        let config = exa_config(two_secrets);
        let repo = repo().await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let store = FileSecretStore::new(&repo, key_path.clone());
        let created = McpServerService::create_server_config(
            &repo,
            "exa".to_string(),
            serde_json::to_value(&config).unwrap(),
            SaveVerification::Pending,
        )
        .await
        .expect("create");
        let entered = vec![
            ("EXA_API_KEY".to_string(), "one".to_string()),
            ("OTHER_KEY".to_string(), "two".to_string()),
        ];
        store_secrets(&repo, &store, &created.id, &entered, &[])
            .await
            .expect("store");
        std::fs::remove_file(&key_path).unwrap();
        let model = repo.get(&created.id).await.unwrap().expect("record");

        let only_one = BTreeSet::from(["EXA_API_KEY".to_string()]);
        let error = plan_secrets(&store, &config, Some(&model), &only_one)
            .await
            .expect_err("the other name cannot be read");
        assert!(error.contains("EXA_API_KEY"), "{error}");
        assert!(error.contains("OTHER_KEY"), "{error}");
        assert!(error.contains("The MCP secret key file"), "{error}");

        let both = BTreeSet::from(["EXA_API_KEY".to_string(), "OTHER_KEY".to_string()]);
        let (ask, reuse) = plan_secrets(&store, &config, Some(&model), &both)
            .await
            .expect("both names are entered again");
        assert_eq!(
            ask,
            vec!["EXA_API_KEY".to_string(), "OTHER_KEY".to_string()]
        );
        assert!(reuse.is_empty());

        let entered_again = vec![
            ("EXA_API_KEY".to_string(), "one-new".to_string()),
            ("OTHER_KEY".to_string(), "two-new".to_string()),
        ];
        write_server(
            &repo,
            &store,
            Some(model),
            &config,
            SaveVerification::Pending,
            &entered_again,
            &[],
        )
        .await
        .expect("write");
        assert_eq!(
            store
                .get(&created.id, "EXA_API_KEY")
                .await
                .unwrap()
                .as_deref(),
            Some("one-new")
        );
        assert_eq!(
            store
                .get(&created.id, "OTHER_KEY")
                .await
                .unwrap()
                .as_deref(),
            Some("two-new")
        );
    }

    #[test]
    fn the_result_line_lists_names_only() {
        let saved = SavedServer {
            id: "id".to_string(),
            name: "exa".to_string(),
            container_check: None,
            entered_secrets: vec!["EXA_API_KEY".to_string()],
            reused_secrets: vec!["OTHER".to_string()],
            storage_warning: None,
        };
        assert_eq!(
            saved.secrets_line(),
            "\n• Secrets: entered EXA_API_KEY; reused OTHER"
        );

        let fallback = SavedServer {
            storage_warning: Some("keychain is not available".to_string()),
            ..saved
        };
        assert!(fallback
            .secrets_line()
            .ends_with("\n• Warning: keychain is not available"));
    }
}
