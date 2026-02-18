use sea_orm_migration::{prelude::*, schema::*};

use super::helpers;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create migration_metadata table for enhanced migration tracking
        manager
            .create_table(
                Table::create()
                    .table(MigrationMetadata::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MigrationMetadata::Version)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(string(MigrationMetadata::Checksum))
                    .col(text_null(MigrationMetadata::Description))
                    .col(big_integer(MigrationMetadata::AppliedAt))
                    .col(integer_null(MigrationMetadata::ExecutionTimeMs))
                    .col(boolean(MigrationMetadata::Success).default(true))
                    .to_owned(),
            )
            .await?;

        // Create schema_version table for application version tracking
        manager
            .create_table(
                Table::create()
                    .table(SchemaVersion::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SchemaVersion::Version)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(integer(SchemaVersion::MigrationCount))
                    .col(big_integer(SchemaVersion::AppliedAt))
                    .col(string_null(SchemaVersion::Checksum))
                    .to_owned(),
            )
            .await?;

        // ✅ IDEMPOTENT: migrate seaql_migrations → migration_metadata only when empty.
        // Use the shared helper so COUNT(*) aliasing is consistent across all migrations.
        let existing_count = helpers::count_rows(manager, MigrationMetadata::Table).await?;
        if existing_count == 0 {
            // Check that the source table exists before attempting to copy from it.
            // On a completely fresh DB seaql_migrations may not yet have rows (or may
            // not exist) – either case is fine, we simply skip the backfill.
            let source_exists =
                helpers::table_exists(manager, "seaql_migrations").await?;

            if source_exists {
                // seaql_migrations only has a `version` column (SeaORM standard).
                // Supply sentinel values for the remaining columns:
                //   - checksum: "legacy" (unknown at backfill time)
                //   - description: human-readable note
                //   - applied_at: Unix epoch 0 (timestamp unknown for legacy rows)
                //   - execution_time_ms: 0 (not recorded for legacy rows)
                //   - success: true (they ran successfully if they're in seaql_migrations)
                manager
                    .exec_stmt(
                        Query::insert()
                            .into_table(MigrationMetadata::Table)
                            .columns([
                                MigrationMetadata::Version,
                                MigrationMetadata::Checksum,
                                MigrationMetadata::Description,
                                MigrationMetadata::AppliedAt,
                                MigrationMetadata::ExecutionTimeMs,
                                MigrationMetadata::Success,
                            ])
                            .select_from(
                                Query::select()
                                    .column(Alias::new("version"))
                                    .expr(Expr::value("legacy"))
                                    .expr(Expr::value("Migrated from seaql_migrations"))
                                    .expr(Expr::value(0i64)) // applied_at: epoch 0 (unknown)
                                    .expr(Expr::value(0i32)) // execution_time_ms: unknown
                                    .expr(Expr::value(true))
                                    .from(Alias::new("seaql_migrations"))
                                    .to_owned(),
                            )
                            .unwrap()
                            .to_owned(),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SchemaVersion::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MigrationMetadata::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum MigrationMetadata {
    Table,
    Version,
    Checksum,
    Description,
    AppliedAt,
    ExecutionTimeMs,
    Success,
}

#[derive(DeriveIden)]
enum SchemaVersion {
    Table,
    Version,
    MigrationCount,
    AppliedAt,
    Checksum,
}
