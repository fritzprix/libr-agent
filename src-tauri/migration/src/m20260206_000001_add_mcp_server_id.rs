use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        log::info!("Starting MCP server ID migration...");

        let db = manager.get_connection();

        // Step 1: Add id column (nullable temporarily)
        log::info!("Step 1: Adding id column...");
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .add_column(ColumnDef::new(McpServers::Id).string().null())
                    .to_owned(),
            )
            .await?;

        // Step 2: Populate IDs for existing rows using cuid2
        log::info!("Step 2: Generating IDs for existing servers...");

        // Get all existing servers
        let query_result = db
            .query_all(
                db.get_database_backend().build(
                    &sea_orm::sea_query::Query::select()
                        .from(McpServers::Table)
                        .columns([McpServers::Name, McpServers::Config])
                        .to_owned(),
                ),
            )
            .await?;

        log::info!(
            "Found {} existing MCP servers to migrate",
            query_result.len()
        );

        // Build name -> id mapping for assistant migration
        let mut name_to_id_map = std::collections::HashMap::new();

        for row in query_result {
            let name: String = row.try_get("", "name")?;
            let id = cuid2::create_id();

            log::debug!("Assigning ID '{}' to server '{}'", id, name);
            name_to_id_map.insert(name.clone(), id.clone());

            // Update the row with the new ID using raw SQL
            db.execute_unprepared(&format!(
                "UPDATE mcp_servers SET id = '{}' WHERE name = '{}'",
                id.replace("'", "''"), // Escape single quotes
                name.replace("'", "''")
            ))
            .await?;
        }

        // Step 3: Create new table with correct schema
        log::info!("Step 3: Creating new table with id as primary key...");
        manager
            .create_table(
                Table::create()
                    .table(McpServers::TableNew)
                    .col(
                        ColumnDef::new(McpServers::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(McpServers::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(McpServers::Config).string().not_null())
                    .col(ColumnDef::new(McpServers::ToolCount).integer())
                    .col(
                        ColumnDef::new(McpServers::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpServers::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 4: Copy data to new table
        log::info!("Step 4: Copying data to new table...");
        db.execute_unprepared(
            "INSERT INTO mcp_servers_new (id, name, config, tool_count, created_at, updated_at)
             SELECT id, name, config, tool_count, created_at, updated_at FROM mcp_servers",
        )
        .await?;

        // Step 5: Drop old table and rename new table
        log::info!("Step 5: Replacing old table...");
        manager
            .drop_table(Table::drop().table(McpServers::Table).to_owned())
            .await?;

        db.execute_unprepared("ALTER TABLE mcp_servers_new RENAME TO mcp_servers")
            .await?;

        // Step 6: Create index on name for lookups
        log::info!("Step 6: Creating index on name column...");
        manager
            .create_index(
                Index::create()
                    .name("idx_mcp_servers_name")
                    .table(McpServers::Table)
                    .col(McpServers::Name)
                    .to_owned(),
            )
            .await?;

        // Step 7: Migrate assistant configs from names to IDs
        log::info!(
            "Step 7: Migrating assistant configs (converting {} server references)...",
            name_to_id_map.len()
        );

        // Get all assistants
        let query_result = db
            .query_all(
                db.get_database_backend().build(
                    &sea_orm::sea_query::Query::select()
                        .from(Assistants::Table)
                        .columns([Assistants::Id, Assistants::Name, Assistants::Config])
                        .to_owned(),
                ),
            )
            .await?;

        log::info!("Found {} assistants to check", query_result.len());

        let mut migrated_count = 0;
        let mut skipped_count = 0;

        for row in query_result {
            let assistant_id: String = row.try_get("", "id")?;
            let assistant_name: String = row.try_get("", "name")?;
            let config_str: String = row.try_get("", "config")?;

            let mut config: serde_json::Value = match serde_json::from_str(&config_str) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!(
                        "Skipping assistant '{}' - invalid JSON: {}",
                        assistant_name,
                        e
                    );
                    skipped_count += 1;
                    continue;
                }
            };

            // Migrate mcpServerIds from names to IDs
            if let Some(mcp_ids) = config
                .get_mut("mcpServerIds")
                .and_then(|v| v.as_array_mut())
            {
                let mut migrated = false;
                let mut invalid_refs = Vec::new();

                for id_value in mcp_ids.iter_mut() {
                    if let Some(name) = id_value.as_str() {
                        if let Some(new_id) = name_to_id_map.get(name) {
                            log::debug!(
                                "Assistant '{}': Converting '{}' -> '{}'",
                                assistant_name,
                                name,
                                new_id
                            );
                            *id_value = serde_json::json!(new_id);
                            migrated = true;
                        } else {
                            log::warn!(
                                "Assistant '{}' references unknown MCP server '{}' - will be removed",
                                assistant_name,
                                name
                            );
                            invalid_refs.push(name.to_string());
                        }
                    }
                }

                if migrated {
                    // Remove invalid references (servers that don't exist)
                    mcp_ids.retain(|v| {
                        v.as_str()
                            .and_then(|s| name_to_id_map.values().any(|id| id == s).then_some(true))
                            .unwrap_or(false)
                    });

                    if !invalid_refs.is_empty() {
                        log::warn!(
                            "Removed {} invalid server references from assistant '{}'",
                            invalid_refs.len(),
                            assistant_name
                        );
                    }

                    // Save updated config
                    let new_config_str = serde_json::to_string(&config)
                        .map_err(|e| DbErr::Custom(format!("Failed to serialize config: {}", e)))?;

                    db.execute_unprepared(&format!(
                        "UPDATE assistants SET config = '{}' WHERE id = '{}'",
                        new_config_str.replace("'", "''"),
                        assistant_id.replace("'", "''")
                    ))
                    .await?;

                    migrated_count += 1;
                    log::info!(
                        "Migrated assistant '{}' (removed {} invalid refs)",
                        assistant_name,
                        invalid_refs.len()
                    );
                }
            }
        }

        log::info!(
            "Migration complete: {} assistants migrated, {} skipped",
            migrated_count,
            skipped_count
        );

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        log::warn!("⚠️  Downgrading MCP server schema - this will BREAK assistant references!");
        log::warn!("⚠️  Assistant mcpServerIds will be reset to empty arrays");

        let db = manager.get_connection();

        // Step 1: Create old schema table (name as PK, no id column)
        manager
            .create_table(
                Table::create()
                    .table(McpServers::TableOld)
                    .col(
                        ColumnDef::new(McpServers::Name)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(McpServers::Config).string().not_null())
                    .col(ColumnDef::new(McpServers::ToolCount).integer())
                    .col(
                        ColumnDef::new(McpServers::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpServers::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 2: Copy data (excluding id column)
        db.execute_unprepared(
            "INSERT INTO mcp_servers_old (name, config, tool_count, created_at, updated_at)
             SELECT name, config, tool_count, created_at, updated_at FROM mcp_servers",
        )
        .await?;

        // Step 3: Drop new table and rename old table
        manager
            .drop_table(Table::drop().table(McpServers::Table).to_owned())
            .await?;

        db.execute_unprepared("ALTER TABLE mcp_servers_old RENAME TO mcp_servers")
            .await?;

        // Step 4: Reset all assistant mcpServerIds to empty arrays
        log::warn!("Resetting all assistant mcpServerIds to empty arrays...");

        let query_result = db
            .query_all(
                db.get_database_backend().build(
                    &sea_orm::sea_query::Query::select()
                        .from(Assistants::Table)
                        .columns([Assistants::Id, Assistants::Config])
                        .to_owned(),
                ),
            )
            .await?;

        for row in query_result {
            let assistant_id: String = row.try_get("", "id")?;
            let config_str: String = row.try_get("", "config")?;

            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&config_str) {
                config["mcpServerIds"] = serde_json::json!([]);

                let new_config_str = serde_json::to_string(&config)
                    .map_err(|e| DbErr::Custom(format!("Failed to serialize: {}", e)))?;

                db.execute_unprepared(&format!(
                    "UPDATE assistants SET config = '{}' WHERE id = '{}'",
                    new_config_str.replace("'", "''"),
                    assistant_id.replace("'", "''")
                ))
                .await?;
            }
        }

        log::info!("Downgrade complete - assistant mcpServerIds have been reset");

        Ok(())
    }
}

#[derive(DeriveIden)]
enum McpServers {
    Table,
    #[sea_orm(iden = "mcp_servers_new")]
    TableNew,
    #[sea_orm(iden = "mcp_servers_old")]
    TableOld,
    Id,
    Name,
    Config,
    ToolCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Assistants {
    Table,
    Id,
    Name,
    Config,
}
