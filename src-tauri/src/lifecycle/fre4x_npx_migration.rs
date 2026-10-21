//! One-time data migration: pin stored npx `@fre4x/*` args to `@latest`.
//!
//! Reads JSON configs from `mcp_servers` and rewrites unversioned fre4x package
//! arguments for npx-based stdio transports.

use log::{info, warn};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, Set};

use crate::entity::mcp_server;
use crate::mcp::fre4x_npx_args::migrate_mcp_server_config_json;

pub async fn run_fre4x_npx_migrations(db: &DatabaseConnection) {
    info!("🔄 Running @fre4x npx @latest migration...");

    if let Err(e) = migrate_mcp_servers(db).await {
        warn!(
            "fre4x_npx_migration: failed to migrate mcp_servers (non-fatal): {}",
            e
        );
    }

    info!("✅ @fre4x npx @latest migration completed.");
}

async fn migrate_mcp_servers(db: &DatabaseConnection) -> Result<(), String> {
    let servers = mcp_server::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut update_count = 0;

    for model in servers {
        let Some(new_config_str) = migrate_mcp_server_config_json(&model.config) else {
            continue;
        };

        let id = model.id.clone();
        let name = model.name.clone();
        let mut active_model = model.into_active_model();
        active_model.config = Set(new_config_str);
        if let Err(e) = active_model.update(db).await {
            warn!("fre4x_npx_migration: failed to update MCP server {}: {}", id, e);
            continue;
        }

        update_count += 1;
        info!(
            "fre4x_npx_migration: updated MCP server '{}' ({}) npx @fre4x args to @latest",
            name, id
        );
    }

    if update_count > 0 {
        info!(
            "Migrated {} MCP server(s) to @fre4x/*@latest npx args.",
            update_count
        );
    }

    Ok(())
}
