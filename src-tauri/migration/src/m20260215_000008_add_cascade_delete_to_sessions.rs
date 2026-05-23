use crate::helpers::table_exists;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

async fn recreate_sessions_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_sessions_parent_session_id")
                .table(Sessions::Table)
                .col(Sessions::ParentSessionId)
                .if_not_exists()
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_sessions_lineage_id")
                .table(Sessions::Table)
                .col(Sessions::LineageId)
                .if_not_exists()
                .to_owned(),
        )
        .await
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite doesn't support adding FK constraints to existing columns directly.
        // We need to recreate the table with FK constraint.

        let db = manager.get_connection();
        let sessions_exists = table_exists(manager, "sessions").await?;
        let sessions_new_exists = table_exists(manager, "sessions_new").await?;

        // Recover from a previously interrupted migration that already dropped the
        // original table and left the rebuilt temp table behind.
        if !sessions_exists && sessions_new_exists {
            db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;
            let recover_result = async {
                db.execute_unprepared("ALTER TABLE sessions_new RENAME TO sessions")
                    .await?;
                recreate_sessions_indexes(manager).await
            }
            .await;
            let reenable_result = db.execute_unprepared("PRAGMA foreign_keys = ON").await;
            recover_result?;
            reenable_result?;
            return Ok(());
        }

        if sessions_new_exists {
            manager
                .drop_table(Table::drop().table(Alias::new("sessions_new")).to_owned())
                .await?;
        }

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // Step 1: Create new temp table with FK constraint
        let migration_result = async {
            manager
                .create_table(
                    Table::create()
                        .table(Alias::new("sessions_new"))
                        .col(
                            ColumnDef::new(Sessions::Id)
                                .string()
                                .not_null()
                                .primary_key(),
                        )
                        .col(ColumnDef::new(Sessions::Name).string())
                        .col(ColumnDef::new(Sessions::Status).string().not_null())
                        .col(ColumnDef::new(Sessions::AgentConfig).string())
                        .col(ColumnDef::new(Sessions::ParentSessionId).string())
                        .col(ColumnDef::new(Sessions::LineageId).string())
                        .col(ColumnDef::new(Sessions::Depth).integer())
                        .col(ColumnDef::new(Sessions::MaxDepth).integer())
                        .col(ColumnDef::new(Sessions::MaxFanout).integer())
                        .col(ColumnDef::new(Sessions::Provider).string())
                        .col(ColumnDef::new(Sessions::Model).string())
                        .col(ColumnDef::new(Sessions::CreatedAt).big_integer().not_null())
                        .col(ColumnDef::new(Sessions::UpdatedAt).big_integer().not_null())
                        // Add FK constraint with CASCADE
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk-sessions-parent_session_id")
                                .from(Alias::new("sessions_new"), Sessions::ParentSessionId)
                                .to(Alias::new("sessions_new"), Sessions::Id)
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await?;

            // Step 2: Copy data from old table
            // Use dynamic SQL to handle cases where some columns might not exist yet
            let copy_result = db
                .execute_unprepared(
                    "INSERT INTO sessions_new (id, name, status, agent_config, parent_session_id, 
                     lineage_id, depth, max_depth, max_fanout, provider, model, 
                     created_at, updated_at)
                     SELECT id, name, status, agent_config, parent_session_id, 
                            lineage_id, depth, max_depth, max_fanout, 
                            COALESCE(provider, NULL), COALESCE(model, NULL),
                            created_at, updated_at 
                     FROM sessions",
                )
                .await;

            // If copy fails due to missing columns, try simpler version
            if copy_result.is_err() {
                db.execute_unprepared(
                    "INSERT INTO sessions_new (id, name, status, agent_config, parent_session_id, 
                     lineage_id, depth, max_depth, max_fanout, created_at, updated_at)
                     SELECT id, name, status, agent_config, parent_session_id, 
                            lineage_id, depth, max_depth, max_fanout, created_at, updated_at 
                     FROM sessions",
                )
                .await?;
            }

            // Step 3: Drop old table
            db.execute_unprepared("DROP TABLE IF EXISTS sessions")
                .await?;

            // Step 4: Rename new table
            db.execute_unprepared("ALTER TABLE sessions_new RENAME TO sessions")
                .await?;

            // Step 5: Recreate indexes
            recreate_sessions_indexes(manager).await
        }
        .await;

        let reenable_result = db.execute_unprepared("PRAGMA foreign_keys = ON").await;
        migration_result?;
        reenable_result?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rollback: recreate table without FK constraint
        let db = manager.get_connection();
        if table_exists(manager, "sessions_old").await? {
            manager
                .drop_table(Table::drop().table(Alias::new("sessions_old")).to_owned())
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sessions_old"))
                    .col(
                        ColumnDef::new(Sessions::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Sessions::Name).string())
                    .col(ColumnDef::new(Sessions::Status).string().not_null())
                    .col(ColumnDef::new(Sessions::AgentConfig).string())
                    .col(ColumnDef::new(Sessions::ParentSessionId).string())
                    .col(ColumnDef::new(Sessions::LineageId).string())
                    .col(ColumnDef::new(Sessions::Depth).integer())
                    .col(ColumnDef::new(Sessions::MaxDepth).integer())
                    .col(ColumnDef::new(Sessions::MaxFanout).integer())
                    .col(ColumnDef::new(Sessions::Provider).string())
                    .col(ColumnDef::new(Sessions::Model).string())
                    .col(ColumnDef::new(Sessions::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Sessions::UpdatedAt).big_integer().not_null())
                    // No FK constraint
                    .to_owned(),
            )
            .await?;

        // Copy data with fallback for missing columns
        let copy_result = db
            .execute_unprepared(
                "INSERT INTO sessions_old (id, name, status, agent_config, parent_session_id, 
                 lineage_id, depth, max_depth, max_fanout, provider, model, 
                 created_at, updated_at)
                 SELECT id, name, status, agent_config, parent_session_id, 
                        lineage_id, depth, max_depth, max_fanout, 
                        COALESCE(provider, NULL), COALESCE(model, NULL),
                        created_at, updated_at 
                 FROM sessions",
            )
            .await;

        if copy_result.is_err() {
            db.execute_unprepared(
                "INSERT INTO sessions_old (id, name, status, agent_config, parent_session_id, 
                 lineage_id, depth, max_depth, max_fanout, created_at, updated_at)
                 SELECT id, name, status, agent_config, parent_session_id, 
                        lineage_id, depth, max_depth, max_fanout, created_at, updated_at 
                 FROM sessions",
            )
            .await?;
        }

        db.execute_unprepared("DROP TABLE IF EXISTS sessions")
            .await?;

        db.execute_unprepared("ALTER TABLE sessions_old RENAME TO sessions")
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_parent_session_id")
                    .table(Sessions::Table)
                    .col(Sessions::ParentSessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_lineage_id")
                    .table(Sessions::Table)
                    .col(Sessions::LineageId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Id,
    Name,
    Status,
    AgentConfig,
    ParentSessionId,
    LineageId,
    Depth,
    MaxDepth,
    MaxFanout,
    Provider,
    Model,
    CreatedAt,
    UpdatedAt,
}
