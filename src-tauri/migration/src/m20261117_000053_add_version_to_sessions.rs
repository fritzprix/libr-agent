use crate::helpers::column_exists;
use crate::session_version::{SESSION_VERSION_TRIGGERS, SESSION_VERSION_TRIGGER_NAMES};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

/// Message-change counter for `sessions` (#2878).
///
/// The headless `--session` write-back decides merge-or-fork from a fork base
/// captured at run start. Counting messages and comparing the newest id
/// misses in-place edits of existing rows, which merged a run into a profile
/// it never saw. `sessions.version` is bumped by triggers on every message
/// insert, update (including upsert-on-conflict), and delete, so any change
/// to the session's history moves the counter.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "sessions", "version").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Sessions::Table)
                        .add_column(
                            ColumnDef::new(Sessions::Version)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
        }

        let db = manager.get_connection();
        for sql in SESSION_VERSION_TRIGGERS {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                sql.to_owned(),
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for name in SESSION_VERSION_TRIGGER_NAMES {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("DROP TRIGGER IF EXISTS {name};"),
            ))
            .await?;
        }

        if column_exists(manager, "sessions", "version").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Sessions::Table)
                        .drop_column(Sessions::Version)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Version,
}
