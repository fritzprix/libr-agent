// #3039: `PRAGMA foreign_keys` belongs to the SQLite connection, not to the
// transaction, and the connection returns to the pool. Turning it off for the
// import and restoring it only on the two normal exits left a pooled connection
// enforcing nothing after any early `?`. These drive the real wrapper around a
// body that fails part-way, which is the case the old code got wrong.
#[cfg(test)]
mod foreign_key_restore_tests {
    use crate::commands::migration_commands::fk;
    use crate::commands::migration_commands::import::db;
    use crate::entity::mcp_server;
    use sea_orm::{DatabaseConnection, EntityTrait};
    use sea_orm_migration::MigratorTrait;

    async fn memory_db() -> DatabaseConnection {
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db
    }

    #[tokio::test]
    async fn foreign_keys_are_off_while_the_import_body_runs() {
        let db = memory_db().await;
        fk::with_foreign_keys_disabled(&db, async {
            assert!(
                !fk::foreign_keys_enabled(&db).await,
                "the body must run with foreign-key checks off"
            );
            Ok::<(), String>(())
        })
        .await
        .expect("body succeeded");
    }

    #[tokio::test]
    async fn an_early_error_restores_foreign_keys() {
        let db = memory_db().await;
        assert!(fk::foreign_keys_enabled(&db).await);

        let result = fk::with_foreign_keys_disabled(&db, async {
            // The importer opens the transaction, then reads a section: a
            // malformed archive fails here, before commit or rollback.
            let txn = db::begin_import_transaction(&db).await?;
            drop(txn);
            Err::<(), String>("invalid JSON in mcp_servers.json".to_string())
        })
        .await;

        assert_eq!(
            result,
            Err("invalid JSON in mcp_servers.json".to_string()),
            "the body's own error must survive the restore"
        );
        assert!(
            fk::foreign_keys_enabled(&db).await,
            "an early error left the connection with foreign-key checks off"
        );
    }

    #[tokio::test]
    async fn a_failed_commit_restores_foreign_keys() {
        let db = memory_db().await;
        let result = fk::with_foreign_keys_disabled(&db, async {
            let txn = db::begin_import_transaction(&db).await?;
            // A commit that fails consumes the transaction and returns early,
            // which used to skip the restore.
            mcp_server::Entity::delete_many().exec(&txn).await.ok();
            Err::<(), String>("commit failed".to_string())
        })
        .await;

        assert_eq!(result, Err("commit failed".to_string()));
        assert!(fk::foreign_keys_enabled(&db).await);
    }
}
