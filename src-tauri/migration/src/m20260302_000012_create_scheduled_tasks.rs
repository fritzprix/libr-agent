use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScheduledTasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScheduledTasks::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(string(ScheduledTasks::Name))
                    .col(string(ScheduledTasks::CronExpression))
                    .col(string(ScheduledTasks::AssistantId))
                    // Supports @playbook:name and @skill:name mention syntax,
                    // resolved at execution time via resolve_message_references
                    .col(text(ScheduledTasks::Message))
                    .col(string_null(ScheduledTasks::SessionId))
                    .col(boolean(ScheduledTasks::Enabled).default(true))
                    .col(big_integer_null(ScheduledTasks::LastRunAt))
                    .col(big_integer_null(ScheduledTasks::NextRunAt))
                    .col(big_integer(ScheduledTasks::CreatedAt))
                    .col(big_integer(ScheduledTasks::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScheduledTasks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    Id,
    Name,
    CronExpression,
    AssistantId,
    Message,
    SessionId,
    Enabled,
    LastRunAt,
    NextRunAt,
    CreatedAt,
    UpdatedAt,
}
