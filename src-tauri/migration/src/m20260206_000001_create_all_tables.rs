use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Sessions
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Sessions::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Sessions::Name).string())
                    .col(ColumnDef::new(Sessions::Status).string().not_null())
                    .col(ColumnDef::new(Sessions::AgentConfig).string())
                    .col(
                        ColumnDef::new(Sessions::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sessions::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Messages
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Messages::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Messages::SessionId).string().not_null())
                    .col(ColumnDef::new(Messages::Role).string().not_null())
                    .col(ColumnDef::new(Messages::Content).text().not_null())
                    .col(ColumnDef::new(Messages::ToolCalls).string())
                    .col(ColumnDef::new(Messages::ToolCallId).string())
                    .col(ColumnDef::new(Messages::IsStreaming).integer())
                    .col(ColumnDef::new(Messages::Thinking).string())
                    .col(ColumnDef::new(Messages::ThinkingSignature).string())
                    .col(ColumnDef::new(Messages::AssistantId).string())
                    .col(ColumnDef::new(Messages::Attachments).string())
                    .col(ColumnDef::new(Messages::ToolUse).string())
                    .col(ColumnDef::new(Messages::Source).string())
                    .col(ColumnDef::new(Messages::Error).string())
                    .col(
                        ColumnDef::new(Messages::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Messages::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-messages-session_id")
                            .from(Messages::Table, Messages::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. MCP Servers (Special Handling)
        let has_mcp_table = manager.has_table(McpServers::Table.to_string()).await?;

        match has_mcp_table {
            false => {
                // Fresh install
                manager
                    .create_table(
                        Table::create()
                            .table(McpServers::Table)
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
            }
            true => {
                // Check if 'id' column exists
                // Note: The most reliable way to check for column existence without failing
                // is querying information schema or just trying to select it.
                // Since this is SQLite, we can check PRAGMA table_info or select.
                let has_id = match db
                    .execute(Statement::from_string(
                        db.get_database_backend(),
                        "SELECT id FROM mcp_servers LIMIT 1".to_owned(),
                    ))
                    .await
                {
                    Ok(_) => true,
                    Err(_) => false,
                };

                if !has_id {
                    log::info!("Migrating existing mcp_servers table...");

                    // Step 1: Add id column (nullable temporarily)
                    manager
                        .alter_table(
                            Table::alter()
                                .table(McpServers::Table)
                                .add_column(ColumnDef::new(McpServers::Id).string().null())
                                .to_owned(),
                        )
                        .await?;

                    // Step 2: Populate IDs for existing rows
                    let query_result = db
                        .query_all(
                            db.get_database_backend().build(
                                &sea_orm::sea_query::Query::select()
                                    .from(McpServers::Table)
                                    .columns([McpServers::Name])
                                    .to_owned(),
                            ),
                        )
                        .await?;

                    let mut name_to_id_map = std::collections::HashMap::new();

                    for row in query_result {
                        let name: String = row.try_get("", "name")?;
                        let id = cuid2::create_id();
                        name_to_id_map.insert(name.clone(), id.clone());

                        db.execute_unprepared(&format!(
                            "UPDATE mcp_servers SET id = '{}' WHERE name = '{}'",
                            id.replace("'", "''"),
                            name.replace("'", "''")
                        ))
                        .await?;
                    }

                    // Step 3: Create new table with correct schema
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

                    // Step 4: Copy data
                    db.execute_unprepared(
                    "INSERT INTO mcp_servers_new (id, name, config, tool_count, created_at, updated_at)
                     SELECT id, name, config, tool_count, created_at, updated_at FROM mcp_servers"
                ).await?;

                    // Step 5: Replace old table
                    manager
                        .drop_table(Table::drop().table(McpServers::Table).to_owned())
                        .await?;

                    db.execute_unprepared("ALTER TABLE mcp_servers_new RENAME TO mcp_servers")
                        .await?;

                    // Step 6: Migrate assistant configs
                    let query_assistants = db
                        .query_all(
                            db.get_database_backend().build(
                                &sea_orm::sea_query::Query::select()
                                    .from(Assistants::Table)
                                    .columns([Assistants::Id, Assistants::Name, Assistants::Config])
                                    .to_owned(),
                            ),
                        )
                        .await?;

                    for row in query_assistants {
                        let assistant_id: String = row.try_get("", "id")?;
                        let config_str: String = row.try_get("", "config")?;

                        if let Ok(mut config) =
                            serde_json::from_str::<serde_json::Value>(&config_str)
                        {
                            if let Some(mcp_ids) =
                                config.get_mut("mcpServerIds").and_then(|v| v.as_array_mut())
                            {
                                let mut migrated = false;
                                for id_value in mcp_ids.iter_mut() {
                                    if let Some(name) = id_value.as_str() {
                                        if let Some(new_id) = name_to_id_map.get(name) {
                                            *id_value = serde_json::json!(new_id);
                                            migrated = true;
                                        }
                                    }
                                }

                                if migrated {
                                    // Remove invalid refs
                                    mcp_ids.retain(|v| {
                                        v.as_str()
                                            .map(|s| name_to_id_map.values().any(|id| id == s))
                                            .unwrap_or(false)
                                    });

                                    let new_config = serde_json::to_string(&config).map_err(|e| {
                                        DbErr::Custom(format!("Failed to serialize: {}", e))
                                    })?;

                                    db.execute_unprepared(&format!(
                                        "UPDATE assistants SET config = '{}' WHERE id = '{}'",
                                        new_config.replace("'", "''"),
                                        assistant_id.replace("'", "''")
                                    ))
                                    .await?;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Playbooks
        manager
            .create_table(
                Table::create()
                    .table(Playbooks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Playbooks::Id).string().not_null())
                    .col(
                        ColumnDef::new(Playbooks::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(Playbooks::Id)
                            .col(Playbooks::AssistantId),
                    )
                    .col(ColumnDef::new(Playbooks::Goal).string().not_null())
                    .col(ColumnDef::new(Playbooks::InitialCommand).string())
                    .col(ColumnDef::new(Playbooks::Workflow).string().not_null())
                    .col(ColumnDef::new(Playbooks::SuccessCriteria).string())
                    .col(
                        ColumnDef::new(Playbooks::IsBookmarked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Playbooks::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Playbooks::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 5. Knowledge
        manager
            .create_table(
                Table::create()
                    .table(Knowledge::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Knowledge::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Knowledge::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Knowledge::Title).string().not_null())
                    .col(ColumnDef::new(Knowledge::Content).string().not_null())
                    .col(ColumnDef::new(Knowledge::Source).string())
                    .col(ColumnDef::new(Knowledge::Tags).string())
                    .col(
                        ColumnDef::new(Knowledge::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Knowledge::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .name("idx-knowledge-assistant_id")
                            .col(Knowledge::AssistantId),
                    )
                    .to_owned(),
            )
            .await?;

        // 6. Settings
        manager
            .create_table(
                Table::create()
                    .table(Settings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Settings::Key)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Settings::Value).string().not_null())
                    .col(
                        ColumnDef::new(Settings::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Settings::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 7. Planning Goals
        manager
            .create_table(
                Table::create()
                    .table(PlanningGoals::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlanningGoals::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlanningGoals::SessionId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlanningGoals::GoalText)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlanningGoals::Status).string().not_null())
                    .col(
                        ColumnDef::new(PlanningGoals::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 8. Planning Todos
        manager
            .create_table(
                Table::create()
                    .table(PlanningTodos::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlanningTodos::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlanningTodos::SessionId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlanningTodos::Content).string().not_null())
                    .col(ColumnDef::new(PlanningTodos::Description).string())
                    .col(ColumnDef::new(PlanningTodos::Priority).string().not_null())
                    .col(ColumnDef::new(PlanningTodos::ParentId).big_integer())
                    .col(
                        ColumnDef::new(PlanningTodos::IsChecked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(PlanningTodos::Status).string().not_null())
                    .col(
                        ColumnDef::new(PlanningTodos::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlanningTodos::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-planning_todos-parent_id")
                            .from(PlanningTodos::Table, PlanningTodos::ParentId)
                            .to(PlanningTodos::Table, PlanningTodos::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // 9. Planning Scratchpad
        manager
            .create_table(
                Table::create()
                    .table(PlanningScratchpad::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlanningScratchpad::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlanningScratchpad::SessionId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlanningScratchpad::Content)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlanningScratchpad::Title).string())
                    .col(ColumnDef::new(PlanningScratchpad::Source).string())
                    .col(ColumnDef::new(PlanningScratchpad::Tags).string())
                    .col(
                        ColumnDef::new(PlanningScratchpad::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlanningScratchpad::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlanningScratchpad::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PlanningTodos::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PlanningGoals::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Settings::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Knowledge::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Playbooks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(McpServers::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    Name,
    Status,
    AgentConfig,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    SessionId,
    Role,
    Content,
    ToolCalls,
    ToolCallId,
    IsStreaming,
    Thinking,
    ThinkingSignature,
    AssistantId,
    Attachments,
    ToolUse,
    Source,
    Error,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum McpServers {
    Table,
    #[sea_orm(iden = "mcp_servers_new")]
    TableNew,
    Id,
    Name,
    Config,
    ToolCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Playbooks {
    Table,
    Id,
    AssistantId,
    Goal,
    InitialCommand,
    Workflow,
    SuccessCriteria,
    IsBookmarked,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Knowledge {
    Table,
    Id,
    AssistantId,
    Title,
    Content,
    Source,
    Tags,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Settings {
    Table,
    Key,
    Value,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PlanningGoals {
    Table,
    Id,
    SessionId,
    GoalText,
    Status,
    CreatedAt,
}

#[derive(DeriveIden)]
enum PlanningTodos {
    Table,
    Id,
    SessionId,
    Content,
    Description,
    Priority,
    ParentId,
    IsChecked,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PlanningScratchpad {
    Table,
    Id,
    SessionId,
    Content,
    Title,
    Source,
    Tags,
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
