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
                        ColumnDef::new(McpServers::ToolCount).integer().null(), // Nullable - will be populated during verification/connection
                    )
                    .to_owned(),
            )
            .await?;

        // Add index for filtering/sorting by tool count
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .table(McpServers::Table)
                    .name("idx_mcp_servers_tool_count")
                    .col(McpServers::ToolCount)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_mcp_servers_tool_count")
                    .table(McpServers::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .drop_column(McpServers::ToolCount)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum McpServers {
    Table,
    ToolCount,
}
