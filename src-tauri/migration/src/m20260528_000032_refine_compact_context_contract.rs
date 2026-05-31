use crate::helpers::{column_exists, table_exists};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !table_exists(manager, "compact_contexts").await? {
            return Ok(());
        }

        let has_from_id = column_exists(manager, "compact_contexts", "from_id").await?;
        let has_condensed_count =
            column_exists(manager, "compact_contexts", "condensed_count").await?;
        if !has_from_id && has_condensed_count {
            return Ok(());
        }

        let db = manager.get_connection();
        if table_exists(manager, "compact_contexts_new").await? {
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new("compact_contexts_new"))
                        .to_owned(),
                )
                .await?;
        }

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;
        let migration_result = async {
            manager
                .create_table(
                    Table::create()
                        .table(Alias::new("compact_contexts_new"))
                        .col(
                            ColumnDef::new(CompactContexts::Id)
                                .string()
                                .not_null()
                                .primary_key(),
                        )
                        .col(
                            ColumnDef::new(CompactContexts::SessionId)
                                .string()
                                .not_null()
                                .unique_key(),
                        )
                        .col(ColumnDef::new(CompactContexts::ToId).string().not_null())
                        .col(ColumnDef::new(CompactContexts::CondensedCount).integer())
                        .col(ColumnDef::new(CompactContexts::Summary).string().not_null())
                        .col(
                            ColumnDef::new(CompactContexts::CreatedAt)
                                .big_integer()
                                .not_null(),
                        )
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk-compact_contexts-session_id")
                                .from(Alias::new("compact_contexts_new"), CompactContexts::SessionId)
                                .to(Sessions::Table, Sessions::Id)
                                .on_delete(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await?;

            db.execute_unprepared(
                "INSERT INTO compact_contexts_new (id, session_id, to_id, condensed_count, summary, created_at)
                 SELECT id, session_id, to_id, NULL, summary, created_at
                 FROM compact_contexts",
            )
            .await?;

            db.execute_unprepared("DROP TABLE IF EXISTS compact_contexts")
                .await?;
            db.execute_unprepared("ALTER TABLE compact_contexts_new RENAME TO compact_contexts")
                .await?;

            Ok::<(), DbErr>(())
        }
        .await;
        let reenable_result = db.execute_unprepared("PRAGMA foreign_keys = ON").await;
        migration_result?;
        reenable_result?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !table_exists(manager, "compact_contexts").await? {
            return Ok(());
        }

        Err(DbErr::Custom(
            "Cannot safely rollback m20260528_000032_refine_compact_context_contract: \
             the previous compact_contexts.from_id boundary is not recoverable from the \
             new schema. Reconstruct or drop compact_contexts manually before downgrading."
                .to_string(),
        ))
    }
}

#[derive(DeriveIden)]
enum CompactContexts {
    Id,
    SessionId,
    ToId,
    CondensedCount,
    Summary,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Id,
}
