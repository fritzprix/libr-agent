use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .add_column(ColumnDef::new(Sessions::OrgId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .add_column(ColumnDef::new(Sessions::OrgName).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .add_column(ColumnDef::new(Sessions::OrgRootSessionId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_org_id")
                    .table(Sessions::Table)
                    .col(Sessions::OrgId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_org_root_session_id")
                    .table(Sessions::Table)
                    .col(Sessions::OrgRootSessionId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sessions_org_root_session_id")
                    .table(Sessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_sessions_org_id")
                    .table(Sessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .drop_column(Sessions::OrgRootSessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .drop_column(Sessions::OrgName)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sessions::Table)
                    .drop_column(Sessions::OrgId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    OrgId,
    OrgName,
    OrgRootSessionId,
}
