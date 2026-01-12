use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(Settings::Value).string().not_null()) // JSON stored as TEXT
                    .col(ColumnDef::new(Settings::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Settings::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Settings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Settings {
    Table,
    Key,
    Value,
    CreatedAt,
    UpdatedAt,
}
