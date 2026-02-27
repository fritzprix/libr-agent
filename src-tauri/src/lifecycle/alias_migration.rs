//! One-time data migration: normalise legacy builtin service alias names in the DB.
//!
//! ## What this does
//!
//! Rows in the `assistants` table that were created before 0.6.0 may contain
//! `"content_store"` inside their `config` JSON blob:
//! ```json
//! { "allowedBuiltInServiceAliases": ["content_store", "browser"] }
//! ```
//!
//! At runtime we already handle this via [`crate::mcp::builtin::service_id::BuiltinServiceId::from_alias`],
//! but leaving stale values in the DB is technical debt.  This module performs a
//! safe, idempotent SQL `REPLACE` so old rows converge to the current canonical form.
//!
//! ## Safety
//!
//! - The migration is idempotent: running it twice has no effect (no rows match
//!   after the first run).
//! - Only rows whose `config` actually contains `"content_store"` are touched.
//! - The SQL `REPLACE()` operates on the literal substring `"content_store"``
//!   (with JSON quotes), preventing accidental partial matches.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use tracing::{info, warn};

/// Run all pending builtin-alias normalisation migrations.
///
/// Designed to be called once at application startup, after repositories
/// are initialised.  All steps are idempotent.
pub async fn run_alias_migrations(db: &DatabaseConnection) {
    migrate_content_store_to_attachments(db).await;
}

/// Replace the legacy `"content_store"` alias with `"attachments"` in every
/// assistant `config` JSON blob that still contains the old value.
async fn migrate_content_store_to_attachments(db: &DatabaseConnection) {
    let sql = r#"
        UPDATE assistants
        SET config = REPLACE(config, '"content_store"', '"attachments"')
        WHERE config LIKE '%"content_store"%'
    "#;

    match db
        .execute(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
        .await
    {
        Ok(result) => {
            let rows = result.rows_affected();
            if rows > 0 {
                info!(
                    "alias_migration: updated {} assistant row(s): \
                     \"content_store\" → \"attachments\"",
                    rows
                );
            }
            // rows == 0 is the normal steady-state after first run
        }
        Err(e) => {
            // Non-fatal: runtime mapping still works via BuiltinServiceId::from_alias().
            warn!(
                "alias_migration: failed to migrate content_store aliases (non-fatal): {}",
                e
            );
        }
    }
}
