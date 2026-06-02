use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tauri_mcp_agent_lib::lifecycle::database::init_database;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tempfile::TempDir;

async fn list_table_names(db: &sea_orm::DatabaseConnection) -> Result<Vec<String>, sea_orm::DbErr> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name".to_string(),
        ))
        .await?;

    rows.into_iter()
        .map(|row| row.try_get("", "name"))
        .collect()
}

async fn table_sql(
    db: &sea_orm::DatabaseConnection,
    table_name: &str,
) -> Result<Option<String>, sea_orm::DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
        [table_name.into()],
    ))
    .await?
    .map(|row| row.try_get("", "sql"))
    .transpose()
}

#[tokio::test]
async fn fresh_file_database_init_creates_sessions_without_temp_table_leftovers() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("fresh-startup.db");
    let db_url = format_sqlite_url(&db_path.to_string_lossy());

    let db = init_database(&db_url)
        .await
        .expect("fresh file database init should succeed");

    let table_names = list_table_names(&db)
        .await
        .expect("table names should be queryable");

    assert!(
        table_names.iter().any(|name| name == "sessions"),
        "sessions table should exist after migrations: {:?}",
        table_names
    );
    assert!(
        !table_names.iter().any(|name| name == "sessions_new"),
        "temporary sessions_new table must not remain after migrations: {:?}",
        table_names
    );

    let sessions_sql = table_sql(&db, "sessions")
        .await
        .expect("sessions DDL should be queryable")
        .expect("sessions table should exist");
    assert!(
        sessions_sql.contains(
            "FOREIGN KEY (\"parent_session_id\") REFERENCES \"sessions\" (\"id\") ON DELETE CASCADE ON UPDATE CASCADE"
        ),
        "sessions table should preserve the parent_session_id cascade FK on fresh init: {}",
        sessions_sql
    );
}
