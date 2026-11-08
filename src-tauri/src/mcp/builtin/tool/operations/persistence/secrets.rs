//! Planning, asking for, and storing the `{{secret:NAME}}` values of a saved server.

use crate::agent::state::{secret_outcome_message, UserPromptOutcome, UserPromptPayload};
use crate::mcp::secret_templates::{
    parse_secret_names, same_target, secret_names_column, template_names, SecretStore,
};
use crate::mcp::types::MCPServerConfig;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use std::collections::BTreeSet;

/// Which template names reuse a stored value and which the user must enter.
///
/// A value is reused only for the same server and name, when the target is unchanged
/// ([`same_target`]) and the call does not ask to enter it again.
pub(super) async fn plan_secrets(
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
pub(super) async fn ask_secret(
    session_id: Option<&str>,
    name: &str,
    server: &str,
) -> Result<String, String> {
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
pub(super) fn clean_secret(value: &str, name: &str) -> Result<String, String> {
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
pub(super) async fn store_values(
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
pub(super) async fn store_secrets(
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

pub(super) async fn record_secret_names(
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
