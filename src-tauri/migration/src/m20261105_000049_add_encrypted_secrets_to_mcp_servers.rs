use sea_orm_migration::prelude::*;

/// The `{NAME: value}` map of a server, AES-256-GCM encrypted with the app key file, used
/// when the OS keychain is not available (#2570). base64 of nonce plus ciphertext.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .add_column(ColumnDef::new(McpServers::EncryptedSecrets).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpServers::Table)
                    .drop_column(McpServers::EncryptedSecrets)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum McpServers {
    #[sea_orm(iden = "mcp_servers")]
    Table,
    EncryptedSecrets,
}
