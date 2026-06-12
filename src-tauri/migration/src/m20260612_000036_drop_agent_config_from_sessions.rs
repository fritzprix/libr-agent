use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // Final backfill before dropping the legacy blob column.
        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions
            SET assistant_id = COALESCE(
                assistant_id,
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

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .drop_column(Sessions::AgentConfig)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .add_column(ColumnDef::new(Sessions::AgentConfig).text().null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    AgentConfig,
}
