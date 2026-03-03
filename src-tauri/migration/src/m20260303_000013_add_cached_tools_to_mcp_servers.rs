use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .add_column(
                        ColumnDef::new(McpServers::CachedTools)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .drop_column(McpServers::CachedTools)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum McpServers {
    #[sea_orm(iden = "mcp_servers")]
    Table,
    CachedTools,
}
