use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Playbooks::Table)
                    .add_column(
                        ColumnDef::new(Playbooks::IsBookmarked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Playbooks::Table)
                    .name("idx_playbooks_bookmarked")
                    .col(Playbooks::IsBookmarked)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_playbooks_bookmarked")
                    .table(Playbooks::Table)
                    .to_owned(),
            )
            .await?;

        // Note: SQLite does not support dropping columns in older versions,
        // but SeaORM might handle it or we accept it might fail on downgrade on some sqlite versions.
        manager
            .alter_table(
                Table::alter()
                    .table(Playbooks::Table)
                    .drop_column(Playbooks::IsBookmarked)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Playbooks {
    Table,
    IsBookmarked,
}
