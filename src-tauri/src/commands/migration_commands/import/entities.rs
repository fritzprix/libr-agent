use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::commands::migration_commands::models::{
    AssistantRecord, McpServerRecord, MigrationSectionReport,
};
use crate::entity::{assistant, mcp_server};

/// Delete every assistant row that is NOT plugin-owned (#2923 D2, gatekeeper
/// G1.1). Used by the replace ("overwrite") import instead of a blanket
/// `delete_many`, so a replace import can never wipe the rows a plugin owns;
/// those are removed only by the plugin's uninstall flow.
pub(crate) async fn delete_non_plugin_assistants(
    txn: &sea_orm::DatabaseTransaction,
) -> Result<u64, sea_orm::DbErr> {
    use crate::services::skill_service::{is_plugin_agent_id, plugin_source_of_config};

    let owned: Vec<String> = assistant::Entity::find()
        .all(txn)
        .await?
        .into_iter()
        .filter(|row| is_plugin_agent_id(&row.id) || plugin_source_of_config(&row.config).is_some())
        .map(|row| row.id)
        .collect();

    let mut delete = assistant::Entity::delete_many();
    if !owned.is_empty() {
        delete = delete.filter(assistant::Column::Id.is_not_in(owned));
    }
    Ok(delete.exec(txn).await?.rows_affected)
}

pub(crate) async fn import_assistants_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<AssistantRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    use crate::services::skill_service::{is_plugin_agent_id, plugin_source_of_config};

    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();

        // #2923 (D2, gatekeeper G1.1): an import may not create or overwrite
        // plugin-owned assistants, and may not spoof the ownership marker.
        // Same rule #2924 applied to MCP servers: skip with an error entry.
        let incoming_config: Option<serde_json::Value> = serde_json::from_str(&r.config).ok();
        if let Err(reason) = crate::services::skill_service::reject_plugin_agent_mutation(
            &id,
            incoming_config.as_ref(),
        ) {
            errors.push(format!("Skipped assistant '{}': {}", r.name, reason));
            continue;
        }

        let existing = assistant::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // An existing plugin-owned row is read-only no matter which
        // conflict strategy is asked for.
        if let Some(existing_model) = existing.as_ref() {
            if is_plugin_agent_id(&existing_model.id)
                || plugin_source_of_config(&existing_model.config).is_some()
            {
                errors.push(format!(
                    "Skipped assistant '{}': owned by a plugin (read-only)",
                    r.name
                ));
                continue;
            }
        }

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        if let Some(existing_model) = existing {
            let mut active: assistant::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.config = Set(r.config);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update assistant '{}': {}", id, e));
            } else {
                success += 1;
            }
        } else {
            let active = assistant::ActiveModel {
                id: Set(r.id),
                name: Set(r.name),
                config: Set(r.config),
                created_at: Set(r.created_at),
                updated_at: Set(now),
            };
            if let Err(e) = assistant::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert assistant '{}': {}", id, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

pub(crate) async fn import_mcp_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<McpServerRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();
        let existing = mcp_server::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        // #2924 (D2): an import must never overwrite a plugin-owned server;
        // those rows change only with their plugin.
        if let Some(existing_model) = existing.as_ref() {
            if let Some(owner) = crate::mcp::types::plugin_source_of(&existing_model.config) {
                errors.push(format!(
                    "Skipped MCP server '{}': owned by plugin '{}' (read-only)",
                    r.name, owner
                ));
                continue;
            }
        }

        let now = chrono::Utc::now().timestamp_millis();
        if let Some(existing_model) = existing {
            let mut active: mcp_server::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.config = Set(r.config);
            active.tool_count = Set(r.tool_count);
            active.cached_tools = Set(r.cached_tools);
            active.verification_status = Set(r.verification_status);
            active.last_verification_error = Set(r.last_verification_error);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update MCP server '{}': {}", id, e));
            } else {
                success += 1;
            }
        } else {
            let active = mcp_server::ActiveModel {
                id: Set(r.id),
                name: Set(r.name),
                config: Set(r.config),
                tool_count: Set(r.tool_count),
                cached_tools: Set(r.cached_tools),
                verification_status: Set(r.verification_status),
                last_verification_error: Set(r.last_verification_error),
                secret_names: Set(None),
                encrypted_secrets: Set(None),
                created_at: Set(r.created_at),
                updated_at: Set(now),
            };
            if let Err(e) = mcp_server::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert MCP server '{}': {}", id, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}
