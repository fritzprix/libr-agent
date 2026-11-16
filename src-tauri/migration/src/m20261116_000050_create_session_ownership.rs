use sea_orm_migration::prelude::*;

/// Cross-process session ownership (#2856).
///
/// One row per session currently opened by a GUI backend process. The
/// owner is identified by its run UUID, pid, and OS process start
/// marker (`owner_started_marker`, platform units, equality only);
/// `heartbeat_at` (Unix ms) is refreshed by the owning backend while it
/// lives, and a stale or dead owner loses the lock to the next claimer.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_table(SessionOwnership::Table.to_string())
            .await?
        {
            return Ok(());
        }

        manager
            .create_table(
                Table::create()
                    .table(SessionOwnership::Table)
                    .col(
                        ColumnDef::new(SessionOwnership::SessionId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SessionOwnership::OwnerRun)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SessionOwnership::OwnerPid)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SessionOwnership::OwnerStartedMarker)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SessionOwnership::HeartbeatAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-session_ownership-session_id")
                            .from(SessionOwnership::Table, SessionOwnership::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_table(SessionOwnership::Table.to_string())
            .await?
        {
            return Ok(());
        }

        manager
            .drop_table(Table::drop().table(SessionOwnership::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SessionOwnership {
    #[sea_orm(iden = "session_ownership")]
    Table,
    SessionId,
    OwnerRun,
    OwnerPid,
    OwnerStartedMarker,
    HeartbeatAt,
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Id,
}
