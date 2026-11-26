// #3039: `PRAGMA foreign_keys` belongs to the SQLite connection, not to the
// transaction, and the app's connections come from a shared pool. These drive
// the real wrapper while the whole pool is checked out, so a pragma taken off
// "a" pooled connection cannot hide: an import that borrowed one would either
// have nowhere to run or leave that connection enforcing nothing.
#[cfg(test)]
mod import_connection_tests {
    use crate::commands::migration_commands::fk;
    use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sea_orm::sqlx::SqliteConnection;
    use sea_orm::{
        ConnectionTrait, DatabaseConnection, DbBackend, SqlxSqliteConnector, Statement,
        TransactionTrait,
    };
    use sea_orm_migration::MigratorTrait;
    use std::path::Path;
    use std::str::FromStr;

    /// A migrated file database, opened through the app's own connection helper
    /// so the pool under test matches production (eight connections).
    async fn file_db(dir: &Path) -> DatabaseConnection {
        crate::lifecycle::database::register_sqlite_vec();
        let path = dir.join("libragent.db").to_string_lossy().to_string();

        // Migrations run on a single connection, as in startup: table-rebuild
        // migrations hop across pooled connections otherwise (see
        // `lifecycle::database`).
        let options =
            SqliteConnectOptions::from_str(&crate::utils::sqlite::format_sqlite_url(&path))
                .expect("sqlite url")
                .foreign_keys(true)
                .create_if_missing(true);
        let migrate_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("migration connection");
        let migrate_db = SqlxSqliteConnector::from_sqlx_sqlite_pool(migrate_pool);
        crate::migration::Migrator::up(&migrate_db, None)
            .await
            .expect("migrations");
        migrate_db
            .close()
            .await
            .expect("close migration connection");

        crate::lifecycle::database::connect_sqlite_with_app_pragmas(&path, false)
            .await
            .expect("file db")
    }

    /// `PRAGMA foreign_keys` on a raw pooled connection: 1 when enforced.
    async fn enforced(conn: &mut SqliteConnection) -> i64 {
        let value: i64 = sea_orm::sqlx::query_scalar("PRAGMA foreign_keys;")
            .fetch_one(conn)
            .await
            .expect("PRAGMA foreign_keys");
        value
    }

    #[tokio::test]
    async fn a_successful_import_leaves_every_pooled_connection_enforcing_foreign_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shared = file_db(dir.path()).await;
        let pool = shared.get_sqlite_connection_pool().clone();

        // Hold the whole pool (max 8, see `lifecycle::database`), so the import
        // cannot borrow one of these idle connections.
        let mut held = Vec::new();
        for _ in 0..8 {
            held.push(pool.acquire().await.expect("pooled connection"));
        }

        let checks_were_off = fk::with_foreign_keys_disabled(&shared, |import_db| async move {
            Ok::<bool, String>(!fk::foreign_keys_enabled(&import_db).await)
        })
        .await
        .expect("import body succeeded");

        assert!(
            checks_were_off,
            "the import must run with foreign-key checks off"
        );
        for conn in held.iter_mut() {
            assert_eq!(
                enforced(&mut *conn).await,
                1,
                "the import turned foreign-key checks off on a pooled connection"
            );
        }
    }

    #[tokio::test]
    async fn an_early_error_leaves_every_pooled_connection_enforcing_foreign_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shared = file_db(dir.path()).await;
        let pool = shared.get_sqlite_connection_pool().clone();

        let mut held = Vec::new();
        for _ in 0..8 {
            held.push(pool.acquire().await.expect("pooled connection"));
        }

        let result = fk::with_foreign_keys_disabled(&shared, |import_db| async move {
            // The importer opens its transaction, then reads a section: a
            // malformed archive fails here, before commit or rollback.
            let txn = import_db.begin().await.map_err(|e| e.to_string())?;
            drop(txn);
            Err::<(), String>("invalid JSON in mcp_servers.json".to_string())
        })
        .await;

        assert_eq!(
            result,
            Err("invalid JSON in mcp_servers.json".to_string()),
            "the body's own error must survive"
        );
        for conn in held.iter_mut() {
            assert_eq!(
                enforced(&mut *conn).await,
                1,
                "an early error left a pooled connection with checks off"
            );
        }
    }

    #[tokio::test]
    async fn the_import_connection_points_at_the_same_database_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shared = file_db(dir.path()).await;

        // `user_version` lives in the database file header, so a value written
        // through the import connection is visible to the pooled one only when
        // both opened the same file.
        fk::with_foreign_keys_disabled(&shared, |import_db| async move {
            import_db
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    "PRAGMA user_version = 4242;".to_string(),
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .expect("import body succeeded");

        let row = shared
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA user_version;".to_string(),
            ))
            .await
            .expect("PRAGMA user_version")
            .expect("one row");
        assert_eq!(
            row.try_get_by_index::<i64>(0).expect("integer"),
            4242,
            "the import ran against a different database file"
        );
    }
}
