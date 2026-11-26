//! Foreign-key enforcement around a migration import (#3039).
//!
//! `PRAGMA foreign_keys` is a property of the SQLite *connection*, not of a
//! transaction, and those connections come from a pool. The importers turned it
//! off for the bulk insert and restored it only on the two normal exits, so an
//! early `?` in between — a broken JSON section, a failed begin, a failed
//! commit — left a pooled connection that enforced nothing for the rest of the
//! session. One owner for the pragma, and a body that cannot skip the restore.
use std::future::Future;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

const ON: &str = "PRAGMA foreign_keys = ON;";
const OFF: &str = "PRAGMA foreign_keys = OFF;";

async fn set_foreign_keys(db: &DatabaseConnection, statement: &str) -> Result<(), String> {
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        statement.to_string(),
    ))
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Run `body` with foreign-key checks off, and turn them back on whatever it does.
///
/// `body` is a future, so the `OFF` above runs before anything inside it —
/// including the `begin` of the import transaction it is expected to open.
pub(super) async fn with_foreign_keys_disabled<T>(
    db: &DatabaseConnection,
    body: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    set_foreign_keys(db, OFF).await?;
    let result = body.await;
    // The body's own error is the one to report: a failing restore leaves the
    // connection exactly as broken as it was before this fix, so it must not
    // mask the real cause.
    if let Err(error) = set_foreign_keys(db, ON).await {
        log::error!("could not re-enable foreign keys after a migration import: {error}");
    }
    result
}

/// Read the connection's current `foreign_keys` setting: `true` when enforced.
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
