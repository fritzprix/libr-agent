use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add 'source' column to 'knowledge' table
        manager
            .alter_table(
                Table::alter()
                    .table(Knowledge::Table)
                    .add_column(ColumnDef::new(Knowledge::Source).string())
                    .to_owned(),
            )
            .await?;

        // 2. Drop old FTS triggers
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_ai")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_ad")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_au")
            .await?;

        // 3. Drop old FTS table
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS knowledge_fts")
            .await?;

        // 4. Create new FTS table with source and tags
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts
                USING fts5(title, content, tags, source, content=knowledge, content_rowid=id)
                "#,
            )
            .await?;

        // 5. Rebuild FTS index
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                INSERT INTO knowledge_fts(rowid, title, content, tags, source)
                SELECT id, title, content, tags, source FROM knowledge;
                "#,
            )
            .await?;

        // 6. Create new triggers
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_ai AFTER INSERT ON knowledge BEGIN
                    INSERT INTO knowledge_fts(rowid, title, content, tags, source)
                    VALUES (new.id, new.title, new.content, new.tags, new.source);
                END
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_ad AFTER DELETE ON knowledge BEGIN
                    INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content, tags, source)
                    VALUES('delete', old.id, old.title, old.content, old.tags, old.source);
                END
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_au AFTER UPDATE ON knowledge BEGIN
                    INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content, tags, source)
                    VALUES('delete', old.id, old.title, old.content, old.tags, old.source);
                    INSERT INTO knowledge_fts(rowid, title, content, tags, source)
                    VALUES (new.id, new.title, new.content, new.tags, new.source);
                END
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Revert triggers
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_ai")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_ad")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_au")
            .await?;

        // Revert FTS table
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS knowledge_fts")
            .await?;

        // Recreate old FTS table (without source/tags if they weren't there originally)
        // Based on m20260105, only title and content were in FTS.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts
                USING fts5(title, content, content=knowledge, content_rowid=id)
                "#,
            )
            .await?;

        // Rebuild old FTS index
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                INSERT INTO knowledge_fts(rowid, title, content)
                SELECT id, title, content FROM knowledge;
                "#,
            )
            .await?;

        // Restore old triggers (approximate)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_ai AFTER INSERT ON knowledge BEGIN
                    INSERT INTO knowledge_fts(rowid, title, content)
                    VALUES (new.id, new.title, new.content);
                END
                "#,
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_ad AFTER DELETE ON knowledge BEGIN
                    INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content)
                    VALUES('delete', old.id, old.title, old.content);
                END
                "#,
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_au AFTER UPDATE ON knowledge BEGIN
                    INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content)
                    VALUES('delete', old.id, old.title, old.content);
                    INSERT INTO knowledge_fts(rowid, title, content)
                    VALUES (new.id, new.title, new.content);
                END
                "#,
            )
            .await?;

        // Drop source column
        // SQLite supports DROP COLUMN in newer versions.
        // Assuming environment supports it.
        /* manager
        .alter_table(
            Table::alter()
                .table(Knowledge::Table)
                .drop_column(Knowledge::Source)
                .to_owned(),
        )
        .await?; */

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Knowledge {
    Table,
    Source,
}
