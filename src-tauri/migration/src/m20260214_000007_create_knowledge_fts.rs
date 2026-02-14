use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Create FTS5 virtual table for knowledge full-text search
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(title, content, tags, source, content='knowledge', content_rowid='id');".to_owned(),
        ))
        .await?;

        // Create trigger to sync FTS on INSERT
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TRIGGER IF NOT EXISTS knowledge_ai AFTER INSERT ON knowledge BEGIN
              INSERT INTO knowledge_fts(rowid, title, content, tags, source) VALUES (new.id, new.title, new.content, new.tags, new.source);
            END;".to_owned(),
        ))
        .await?;

        // Create trigger to sync FTS on UPDATE
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TRIGGER IF NOT EXISTS knowledge_au AFTER UPDATE ON knowledge BEGIN
              UPDATE knowledge_fts SET title = new.title, content = new.content, tags = new.tags, source = new.source WHERE rowid = new.id;
            END;".to_owned(),
        ))
        .await?;

        // Create trigger to sync FTS on DELETE
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TRIGGER IF NOT EXISTS knowledge_ad AFTER DELETE ON knowledge BEGIN
              DELETE FROM knowledge_fts WHERE rowid = old.id;
            END;"
                .to_owned(),
        ))
        .await?;

        // Populate FTS table with existing knowledge entries
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "INSERT INTO knowledge_fts(rowid, title, content, tags, source)
             SELECT id, title, content, tags, source FROM knowledge;"
                .to_owned(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Drop triggers
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TRIGGER IF EXISTS knowledge_ad;".to_owned(),
        ))
        .await?;

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TRIGGER IF EXISTS knowledge_au;".to_owned(),
        ))
        .await?;

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TRIGGER IF EXISTS knowledge_ai;".to_owned(),
        ))
        .await?;

        // Drop FTS table
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TABLE IF EXISTS knowledge_fts;".to_owned(),
        ))
        .await?;

        Ok(())
    }
}
