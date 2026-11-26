//! Foreign-key enforcement around a migration import (#3039).
//!
//! `PRAGMA foreign_keys` is a property of the SQLite *connection*, not of a
//! transaction, and the app's connections come from a shared pool of eight. So
//! turning the pragma off on "a" connection and back on later can hit two
//! different connections: the importer's connection stays off in the pool while
//! an unrelated one gets the restore. Restoring after the fact cannot fix that.
//!
//! Instead, an import owns a private single connection to the same database
//! file, opened with checks off and closed when the import ends. The shared
//! pool is never touched, so every exit — an early `?`, a failed commit, a
//! dropped future — is safe by construction rather than by cleanup.
use std::future::Future;
use std::str::FromStr;
use std::time::Duration;

use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};

#[cfg(test)]
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::utils::sqlite::format_sqlite_url;

/// How long the import connection waits for the database file lock.
const IMPORT_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// Path of the file behind the shared connection.
///
/// Read from the pool's own connect options instead of rebuilding it from
/// settings: the private connection must land on the database the app is
/// actually using, not on a path we guessed.
fn database_file(shared: &DatabaseConnection) -> Result<String, String> {
    let path = shared
        .get_sqlite_connection_pool()
        .connect_options()
        .get_filename()
        .to_string_lossy()
        .to_string();

    // `src/lifecycle/database.rs` always opens a file; an in-memory URL has no
    // file to share, so an import could not see the app's data anyway.
    if path.is_empty() || path == ":memory:" {
        return Err("가져오기는 파일 기반 데이터베이스에서만 동작합니다.".to_string());
    }

    Ok(path)
}

/// Open one private connection to `file` with foreign-key checks off.
async fn open_import_connection(file: &str) -> Result<DatabaseConnection, String> {
    let options = SqliteConnectOptions::from_str(&format_sqlite_url(file))
        .map_err(|e| format!("가져오기용 데이터베이스 경로가 올바르지 않습니다: {e}"))?
        .foreign_keys(false)
        // The app pool keeps its own connections; this one queues behind them.
        .busy_timeout(IMPORT_BUSY_TIMEOUT)
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .acquire_timeout(IMPORT_BUSY_TIMEOUT)
        .connect_with(options)
        .await
        .map_err(|e| format!("가져오기용 데이터베이스에 연결하지 못했습니다: {e}"))?;

    Ok(SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

/// Run `body` on a private connection that enforces no foreign keys, then close it.
///
/// `body` receives the connection and must use it for the whole import: reading
/// the shared pool instead would put the transaction back on a pooled
/// connection, which is the defect this wrapper exists to remove.
pub(super) async fn with_foreign_keys_disabled<T, F, Fut>(
    shared: &DatabaseConnection,
    body: F,
) -> Result<T, String>
where
    F: FnOnce(DatabaseConnection) -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let file = database_file(shared)?;
    let import_db = open_import_connection(&file).await?;

    let result = body(import_db.clone()).await;

    // Closing is the restore: the pragma never left this connection, so the
    // shared pool is already clean. A close error is logged and not returned —
    // the body's own error is the one the user needs to see.
    if let Err(error) = import_db.close().await {
        log::error!("could not close the migration import connection: {error}");
    }

    result
}

/// Read a connection's `foreign_keys` setting: `true` when enforced.
#[cfg(test)]
pub(super) async fn foreign_keys_enabled(db: &DatabaseConnection) -> bool {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_keys;".to_string(),
        ))
        .await
        .expect("PRAGMA foreign_keys")
        .expect("one row");
    row.try_get_by_index::<i64>(0).expect("integer") != 0
}
