//! Adds `task_category` and makes `cron_expression` nullable for SESSION one-shot callbacks.
//!
//! **Down migration warning:** `down()` copies only `task_category = 'GLOBAL'` rows.
//! Any `SESSION` tasks are dropped during rollback — back up before reverting.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "scheduled_tasks", "task_category").await? {
            return Ok(());
        }

        let conn = manager.get_connection();
        let backend = manager.get_database_backend();

        conn.execute(Statement::from_string(
            backend,
            "PRAGMA foreign_keys = OFF;".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            r#"
            CREATE TABLE scheduled_tasks_new (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                cron_expression TEXT,
                schedule_timezone TEXT NOT NULL DEFAULT 'local',
                assistant_id TEXT NOT NULL,
                group_id TEXT,
                group_name TEXT,
                message TEXT NOT NULL,
                yolo_mode BOOLEAN NOT NULL DEFAULT 0,
                created_by_session_id TEXT,
                session_id TEXT,
                workspace_override TEXT,
                enabled BOOLEAN NOT NULL DEFAULT 1,
                last_run_at BIGINT,
                next_run_at BIGINT,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
                task_category TEXT NOT NULL DEFAULT 'GLOBAL'
            );
            "#
            .to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            r#"
            INSERT INTO scheduled_tasks_new (
                id, name, cron_expression, schedule_timezone, assistant_id,
                group_id, group_name, message, yolo_mode, created_by_session_id,
                session_id, workspace_override, enabled, last_run_at, next_run_at,
                created_at, updated_at, task_category
            )
            SELECT
                id, name, cron_expression, schedule_timezone, assistant_id,
                group_id, group_name, message, yolo_mode, created_by_session_id,
                session_id, workspace_override, enabled, last_run_at, next_run_at,
                created_at, updated_at, 'GLOBAL'
            FROM scheduled_tasks;
            "#
            .to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "DROP TABLE scheduled_tasks;".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE scheduled_tasks_new RENAME TO scheduled_tasks;".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "PRAGMA foreign_keys = ON;".to_owned(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "scheduled_tasks", "task_category").await? {
            return Ok(());
        }

        let conn = manager.get_connection();
        let backend = manager.get_database_backend();

        conn.execute(Statement::from_string(
            backend,
            "PRAGMA foreign_keys = OFF;".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            r#"
            CREATE TABLE scheduled_tasks_legacy (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                cron_expression TEXT NOT NULL,
                schedule_timezone TEXT NOT NULL DEFAULT 'local',
                assistant_id TEXT NOT NULL,
                group_id TEXT,
                group_name TEXT,
                message TEXT NOT NULL,
                yolo_mode BOOLEAN NOT NULL DEFAULT 0,
                created_by_session_id TEXT,
                session_id TEXT,
                workspace_override TEXT,
                enabled BOOLEAN NOT NULL DEFAULT 1,
                last_run_at BIGINT,
                next_run_at BIGINT,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            );
            "#
            .to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            r#"
            INSERT INTO scheduled_tasks_legacy (
                id, name, cron_expression, schedule_timezone, assistant_id,
                group_id, group_name, message, yolo_mode, created_by_session_id,
                session_id, workspace_override, enabled, last_run_at, next_run_at,
                created_at, updated_at
            )
            SELECT
                id, name, COALESCE(cron_expression, ''), schedule_timezone, assistant_id,
                group_id, group_name, message, yolo_mode, created_by_session_id,
                session_id, workspace_override, enabled, last_run_at, next_run_at,
                created_at, updated_at
            FROM scheduled_tasks
            WHERE task_category = 'GLOBAL' OR task_category IS NULL;
            "#
            .to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "DROP TABLE scheduled_tasks;".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE scheduled_tasks_legacy RENAME TO scheduled_tasks;".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "PRAGMA foreign_keys = ON;".to_owned(),
        ))
        .await?;

        Ok(())
    }
}
