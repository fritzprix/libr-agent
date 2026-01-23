use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite doesn't support dropping columns that are part of constraints
        // We need to recreate the table without session_id

        // Drop the session_id index first
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_playbooks_session")
                    .table(Playbooks::Table)
                    .to_owned(),
            )
            .await?;

        // Create new table without session_id
        manager
            .create_table(
                Table::create()
                    .table(PlaybooksNew::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(PlaybooksNew::Id).string().not_null())
                    .col(
                        ColumnDef::new(PlaybooksNew::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlaybooksNew::Goal).string().not_null())
                    .col(ColumnDef::new(PlaybooksNew::InitialCommand).string())
                    .col(ColumnDef::new(PlaybooksNew::Workflow).string().not_null())
                    .col(ColumnDef::new(PlaybooksNew::SuccessCriteria).string())
                    .col(
                        ColumnDef::new(PlaybooksNew::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybooksNew::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybooksNew::IsBookmarked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .primary_key(
                        Index::create()
                            .col(PlaybooksNew::Id)
                            .col(PlaybooksNew::AssistantId),
                    )
                    .to_owned(),
            )
            .await?;

        // Copy data from old table to new (excluding session_id)
        let copy_sql = r#"
            INSERT INTO playbooks_new (id, assistant_id, goal, initial_command, workflow, success_criteria, created_at, updated_at, is_bookmarked)
            SELECT id, assistant_id, goal, initial_command, workflow, success_criteria, created_at, updated_at, is_bookmarked
            FROM playbooks
        "#;
        manager
            .get_connection()
            .execute_unprepared(copy_sql)
            .await?;

        // Drop old table
        manager
            .drop_table(Table::drop().table(Playbooks::Table).to_owned())
            .await?;

        // Rename new table to original name
        let rename_sql = "ALTER TABLE playbooks_new RENAME TO playbooks";
        manager
            .get_connection()
            .execute_unprepared(rename_sql)
            .await?;

        // Recreate indexes (assistant and updated_at)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_assistant")
                    .table(Playbooks::Table)
                    .col(Playbooks::AssistantId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_updated")
                    .table(Playbooks::Table)
                    .col(Playbooks::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_bookmarked")
                    .table(Playbooks::Table)
                    .col(Playbooks::IsBookmarked)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Recreate table with session_id
        manager
            .create_table(
                Table::create()
                    .table(PlaybooksNew::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(PlaybooksNew::Id).string().not_null())
                    .col(
                        ColumnDef::new(PlaybooksNew::SessionId)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(PlaybooksNew::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlaybooksNew::Goal).string().not_null())
                    .col(ColumnDef::new(PlaybooksNew::InitialCommand).string())
                    .col(ColumnDef::new(PlaybooksNew::Workflow).string().not_null())
                    .col(ColumnDef::new(PlaybooksNew::SuccessCriteria).string())
                    .col(
                        ColumnDef::new(PlaybooksNew::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybooksNew::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybooksNew::IsBookmarked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .primary_key(
                        Index::create()
                            .col(PlaybooksNew::Id)
                            .col(PlaybooksNew::AssistantId),
                    )
                    .to_owned(),
            )
            .await?;

        // Copy data back
        let copy_sql = r#"
            INSERT INTO playbooks_new (id, session_id, assistant_id, goal, initial_command, workflow, success_criteria, created_at, updated_at, is_bookmarked)
            SELECT id, '', assistant_id, goal, initial_command, workflow, success_criteria, created_at, updated_at, is_bookmarked
            FROM playbooks
        "#;
        manager
            .get_connection()
            .execute_unprepared(copy_sql)
            .await?;

        // Drop and rename
        manager
            .drop_table(Table::drop().table(Playbooks::Table).to_owned())
            .await?;

        let rename_sql = "ALTER TABLE playbooks_new RENAME TO playbooks";
        manager
            .get_connection()
            .execute_unprepared(rename_sql)
            .await?;

        // Recreate all indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_session")
                    .table(Playbooks::Table)
                    .col(Playbooks::SessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_assistant")
                    .table(Playbooks::Table)
                    .col(Playbooks::AssistantId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_updated")
                    .table(Playbooks::Table)
                    .col(Playbooks::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_bookmarked")
                    .table(Playbooks::Table)
                    .col(Playbooks::IsBookmarked)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Playbooks {
    Table,
    AssistantId,
    UpdatedAt,
    IsBookmarked,
    SessionId,
}

#[derive(DeriveIden)]
enum PlaybooksNew {
    Table,
    Id,
    SessionId,
    AssistantId,
    Goal,
    InitialCommand,
    Workflow,
    SuccessCriteria,
    CreatedAt,
    UpdatedAt,
    IsBookmarked,
}
