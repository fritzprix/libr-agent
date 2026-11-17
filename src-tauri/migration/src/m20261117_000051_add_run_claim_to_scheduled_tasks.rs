use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;

/// Per-task execution claim for scheduled tasks (#2857).
///
/// Every GUI process on one profile hosts its own scheduler, so a due task
/// row alone was not exclusive: two backends could list the same due row and
/// both fire it. A run now claims the task with a conditional single-row
/// `UPDATE` before executing it (the same pattern as session ownership,
/// #2856): the claim is granted only while `run_claim_run` is free or its
/// `run_claim_expires_at` has lapsed, so at most one live process executes
/// each due task. The holder releases the claim right after the run, and a
/// crashed holder loses it once the expiry lapses.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "scheduled_tasks", "run_claim_run").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .add_column(ColumnDef::new(ScheduledTasks::RunClaimRun).text().null())
                        .to_owned(),
                )
                .await?;
        }
        if !column_exists(manager, "scheduled_tasks", "run_claim_expires_at").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .add_column(
                            ColumnDef::new(ScheduledTasks::RunClaimExpiresAt)
                                .big_integer()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "scheduled_tasks", "run_claim_expires_at").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .drop_column(ScheduledTasks::RunClaimExpiresAt)
                        .to_owned(),
                )
                .await?;
        }
        if column_exists(manager, "scheduled_tasks", "run_claim_run").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .drop_column(ScheduledTasks::RunClaimRun)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    RunClaimRun,
    RunClaimExpiresAt,
}
