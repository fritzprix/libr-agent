use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_table(PendingQueue::Table.to_string()).await? {
            return Ok(());
        }

        manager
            .create_table(
                Table::create()
                    .table(PendingQueue::Table)
                    .col(
                        ColumnDef::new(PendingQueue::MessageId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PendingQueue::SessionId).string().not_null())
                    .col(
                        ColumnDef::new(PendingQueue::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PendingQueue::QueueSeq)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-pending_queue-session_id")
                            .from(PendingQueue::Table, PendingQueue::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-pending_queue-message_id")
                            .from(PendingQueue::Table, PendingQueue::MessageId)
                            .to(Messages::Table, Messages::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_pending_queue_seq_unique")
                    .table(PendingQueue::Table)
                    .col(PendingQueue::QueueSeq)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_pending_queue_session_seq")
                    .table(PendingQueue::Table)
                    .col(PendingQueue::SessionId)
                    .col(PendingQueue::QueueSeq)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table(PendingQueue::Table.to_string()).await? {
            return Ok(());
        }

        manager
            .drop_table(Table::drop().table(PendingQueue::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PendingQueue {
    #[sea_orm(iden = "pending_queue")]
    Table,
    MessageId,
    SessionId,
    CreatedAt,
    QueueSeq,
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Messages {
    #[sea_orm(iden = "messages")]
    Table,
    Id,
}
