use sea_orm_migration::prelude::*;

/// JSON array of the `{{secret:NAME}}` names whose values are in the OS keychain (#2548).
/// It never holds a value.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .add_column(ColumnDef::new(McpServers::SecretNames).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .drop_column(McpServers::SecretNames)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum McpServers {
    #[sea_orm(iden = "mcp_servers")]
    Table,
    SecretNames,
}
