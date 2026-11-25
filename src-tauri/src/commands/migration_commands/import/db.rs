use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait, Statement,
    TransactionTrait,
};
use std::path::Path;

use crate::entity::{mcp_server, playbook, scheduled_task, settings};
use crate::lifecycle::database_backup::BackupManager;

use super::entities::delete_non_plugin_assistants;

pub(super) async fn begin_import_transaction(
    db: &DatabaseConnection,
) -> Result<DatabaseTransaction, String> {
    // Disable Foreign Keys on database connection *outside* transaction
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys = OFF;".to_string(),
    ))
    .await
    .map_err(|e| e.to_string())?;

    db.begin().await.map_err(|e| e.to_string())
}

pub(super) async fn clear_existing_rows_for_overwrite(
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    scheduled_task::Entity::delete_many()
        .exec(txn)
        .await
        .map_err(|e| e.to_string())?;
    playbook::Entity::delete_many()
        .exec(txn)
        .await
        .map_err(|e| e.to_string())?;
    // #2923 (D2, gatekeeper G1.1): a replace import must not wipe
    // plugin-owned assistants; those rows live and die with their
    // plugin (uninstall removes them).
    delete_non_plugin_assistants(txn)
        .await
        .map_err(|e| e.to_string())?;
    mcp_server::Entity::delete_many()
        .exec(txn)
        .await
        .map_err(|e| e.to_string())?;
    settings::Entity::delete_many()
        .exec(txn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn validate_foreign_keys(txn: &DatabaseTransaction) -> bool {
    let fk_violations = txn
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check;".to_string(),
        ))
        .await;

    match fk_violations {
        Ok(v) if v.is_empty() => true,
        Ok(v) => {
            log::error!(
                "Foreign key constraint violations detected during import: {:?}",
                v
            );
            false
        }
        Err(e) => {
            log::error!("Failed to check foreign key integrity: {}", e);
            false
        }
    }
}

pub(super) async fn commit_and_restore_fk(
    db: &DatabaseConnection,
    txn: DatabaseTransaction,
) -> Result<(), String> {
    txn.commit().await.map_err(|e| e.to_string())?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys = ON;".to_string(),
    ))
    .await
    .ok();
    Ok(())
}

pub(super) async fn rollback_and_restore_backup<T>(
    db: &DatabaseConnection,
    txn: DatabaseTransaction,
    backup_manager: &BackupManager,
    backup_file: &Path,
    total_errors: usize,
) -> Result<T, String> {
    txn.rollback().await.ok();
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys = ON;".to_string(),
    ))
    .await
    .ok();
    backup_manager.restore_from_backup(backup_file).ok();

    let err_msg = if total_errors > 0 {
        "데이터를 파싱하거나 데이터베이스에 쓰는 도중 오류가 발생해 원복했습니다.".to_string()
    } else {
        "데이터베이스 참조 무결성(외래키 제약조건) 위반으로 인해 가져오기가 중단되었습니다."
            .to_string()
    };

    Err(err_msg)
}
