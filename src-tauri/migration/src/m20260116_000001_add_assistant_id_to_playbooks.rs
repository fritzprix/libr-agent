use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Check if assistant_id column already exists
        // This handles the case where the table was created with the updated migration
        let db = manager.get_connection();

        // Query to check if column exists (SQLite-specific)
        let check_query = "SELECT COUNT(*) as count FROM pragma_table_info('playbooks') WHERE name='assistant_id'";

        let result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                check_query.to_string(),
            ))
            .await?;

        if let Some(row) = result {
            let count: i32 = row.try_get("", "count").unwrap_or(0);

            // Only add the column if it doesn't exist
            if count == 0 {
                manager
                    .alter_table(
                        Table::alter()
                            .table(Playbooks::Table)
                            .add_column(
                                ColumnDef::new(Playbooks::AssistantId)
                                    .string()
                                    .not_null()
                                    .default(""), // Temporary default for existing rows
                            )
                            .to_owned(),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove assistant_id column from playbooks table
        // Only if it exists (to handle idempotency)
        let db = manager.get_connection();

        let check_query = "SELECT COUNT(*) as count FROM pragma_table_info('playbooks') WHERE name='assistant_id'";

        let result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                check_query.to_string(),
            ))
            .await?;

        if let Some(row) = result {
            let count: i32 = row.try_get("", "count").unwrap_or(0);

            if count > 0 {
                manager
                    .alter_table(
                        Table::alter()
                            .table(Playbooks::Table)
                            .drop_column(Playbooks::AssistantId)
                            .to_owned(),
                    )
                    .await?;
            }
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Playbooks {
    Table,
    AssistantId,
}
