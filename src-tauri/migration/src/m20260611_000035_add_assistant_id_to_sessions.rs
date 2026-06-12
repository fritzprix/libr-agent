use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .add_column(ColumnDef::new(Sessions::AssistantId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_assistant_id")
                    .table(Sessions::Table)
                    .col(Sessions::AssistantId)
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions
            SET assistant_id = COALESCE(
                json_extract(agent_config, '$.id'),
                json_extract(agent_config, '$.assistantId'),
                json_extract(agent_config, '$.assistant_id')
            )
            WHERE assistant_id IS NULL
              AND agent_config IS NOT NULL
              AND agent_config != '';
            "#
            .to_string(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sessions_assistant_id")
                    .table(Sessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .drop_column(Sessions::AssistantId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    AssistantId,
}
