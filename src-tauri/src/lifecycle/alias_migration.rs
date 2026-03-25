//! One-time data migration: normalise legacy builtin service alias names in the DB.
//!
//! ## What this does
//!
//! It reads the JSON configs from `assistants` and `sessions` tables, and if
//! `allowedBuiltInServiceAliases` or `localServices` contain legacy names
//! (like "assistant", "content_store", "mcp_manager"), it converts them to
//! their canonical counterparts ("agent", "attachments", "tool"), then saves the row.

use log::{info, warn};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, Set};
use serde_json::Value;

use crate::entity::{assistant, session};

pub async fn run_alias_migrations(db: &DatabaseConnection) {
    info!("🔄 Running builtin service alias migration...");

    if let Err(e) = migrate_assistants(db).await {
        warn!(
            "alias_migration: failed to migrate assistants (non-fatal): {}",
            e
        );
    }

    if let Err(e) = migrate_sessions(db).await {
        warn!(
            "alias_migration: failed to migrate sessions (non-fatal): {}",
            e
        );
    }

    info!("✅ Builtin service alias migration completed.");
}

fn canonicalize_alias(alias: &str) -> Option<&'static str> {
    match alias.trim().to_lowercase().as_str() {
        "assistant" | "assistant_manager" | "swarm" | "session_api" => Some("agent"),
        "mcp_manager" => Some("tool"),
        "content_store" | "contentstore" => Some("attachments"),
        "memory" => Some("scratchpad"),
        // Already canonical or unknown
        _ => None,
    }
}

fn migrate_json_config(config_str: &str) -> Option<String> {
    let mut config: Value = match serde_json::from_str(config_str) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let mut changed = false;

    // Helper closure to process an array field
    let mut process_array_field = |field_name: &str| {
        if let Some(Value::Array(arr)) = config.get_mut(field_name) {
            for item in arr.iter_mut() {
                if let Value::String(s) = item {
                    if let Some(new_alias) = canonicalize_alias(s) {
                        *item = Value::String(new_alias.to_string());
                        changed = true;
                    }
                }
            }
        }
    };

    process_array_field("allowedBuiltInServiceAliases");
    process_array_field("localServices");

    if changed {
        serde_json::to_string(&config).ok()
    } else {
        None
    }
}

async fn migrate_assistants(db: &DatabaseConnection) -> Result<(), String> {
    let assistants = assistant::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut update_count = 0;

    for model in assistants {
        if let Some(new_config_str) = migrate_json_config(&model.config) {
            let id = model.id.clone();
            let mut active_model = model.into_active_model();
            active_model.config = Set(new_config_str);
            if let Err(e) = active_model.update(db).await {
                warn!("Failed to update assistant {}: {}", id, e);
            } else {
                update_count += 1;
            }
        }
    }

    if update_count > 0 {
        info!(
            "Migrated {} assistant(s) to new canonical service aliases.",
            update_count
        );
    }

    Ok(())
}

async fn migrate_sessions(db: &DatabaseConnection) -> Result<(), String> {
    let sessions = session::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut update_count = 0;

    for model in sessions {
        if let Some(agent_config_str) = &model.agent_config {
            if let Some(new_config_str) = migrate_json_config(agent_config_str) {
                let id = model.id.clone();
                let mut active_model = model.into_active_model();
                active_model.agent_config = Set(Some(new_config_str));
                if let Err(e) = active_model.update(db).await {
                    warn!("Failed to update session {}: {}", id, e);
                } else {
                    update_count += 1;
                }
            }
        }
    }

    if update_count > 0 {
        info!(
            "Migrated {} session(s) to new canonical service aliases.",
            update_count
        );
    }

    Ok(())
}
