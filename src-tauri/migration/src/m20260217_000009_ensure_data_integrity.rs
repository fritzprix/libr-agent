use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // ✅ Step 1: Fill NULL values in existing data
        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions 
            SET model = 'gpt-4' 
            WHERE model IS NULL OR model = '';
            "#
            .to_string(),
        ))
        .await?;

        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions 
            SET provider = 'openai' 
            WHERE provider IS NULL OR provider = '';
            "#
            .to_string(),
        ))
        .await?;

        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions 
            SET depth = 0 
            WHERE depth IS NULL;
            "#
            .to_string(),
        ))
        .await?;

        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions 
            SET max_depth = 10 
            WHERE max_depth IS NULL;
            "#
            .to_string(),
        ))
        .await?;

        db.execute(Statement::from_string(
            backend,
            r#"
            UPDATE sessions 
            SET max_fanout = 5 
            WHERE max_fanout IS NULL;
            "#
            .to_string(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Data migration은 rollback 불가능
        Ok(())
    }
}
