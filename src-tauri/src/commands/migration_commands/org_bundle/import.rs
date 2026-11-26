//! `import_org_bundle`: restore an org bundle's DB sections and filesystem
//! sections, with a DB backup/restore safety net.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use tauri::command;

use crate::commands::migration_commands::crypto::decrypt_data;
use crate::commands::migration_commands::import::{import_assistants_data, import_mcp_data};
use crate::commands::migration_commands::inspect::check_compatibility;
use crate::commands::migration_commands::models::{
    AssistantRecord, CompatibilityStatus, McpServerRecord, MigrationSectionReport,
    OrgBundleImportResult, OrgBundleManifest, ORG_BUNDLE_KIND,
};
use crate::commands::migration_commands::zip_fs::{archive_has_file_prefix, extract_zip_prefix};
use crate::lifecycle::database_backup::BackupManager;
use crate::services::skill_service::get_assistant_skills_directory;
use crate::session::get_session_manager;
use crate::state::get_database_connection;

use super::{emit_progress, sanitize_org_id_for_path};

#[command]
pub async fn import_org_bundle(
    window: tauri::Window,
    file_path: String,
    conflict_strategy: String,
    password: Option<String>,
) -> Result<OrgBundleImportResult, String> {
    // #3039: as in `import_migration`, the import runs on its own connection
    // with foreign-key checks off (see `fk`), so the shared pool keeps
    // enforcing them on every exit.
    let db = get_database_connection();
    super::super::fk::with_foreign_keys_disabled(db, move |import_db| async move {
        run_import_org_bundle(&window, &import_db, file_path, conflict_strategy, password).await
    })
    .await
}

async fn run_import_org_bundle(
    window: &tauri::Window,
    db: &DatabaseConnection,
    file_path: String,
    conflict_strategy: String,
    password: Option<String>,
) -> Result<OrgBundleImportResult, String> {
    emit_progress(window, 5);

    if conflict_strategy != "skip"
        && conflict_strategy != "overwrite"
        && conflict_strategy != "merge"
    {
        return Err("지원되지 않는 충돌 해결 전략입니다.".to_string());
    }

    let file_bytes = fs::read(&file_path).map_err(|e| format!("파일을 읽을 수 없습니다: {}", e))?;
    let zip_bytes = match decrypt_data(&file_bytes, password.as_deref())? {
        Some(decrypted) => decrypted,
        None => file_bytes,
    };

    let (manifest, mcp_data, assistants_data) = {
        let cursor = std::io::Cursor::new(&zip_bytes[..]);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;

        let mut manifest: Option<OrgBundleManifest> = None;
        if let Ok(mut manifest_entry) = archive.by_name("manifest.json") {
            let mut content = String::new();
            manifest_entry
                .read_to_string(&mut content)
                .map_err(|e| e.to_string())?;
            let parsed: OrgBundleManifest =
                serde_json::from_str(&content).map_err(|e| e.to_string())?;
            if parsed.kind != ORG_BUNDLE_KIND {
                return Err("이 파일은 Org 번들(org_bundle)이 아닙니다.".to_string());
            }
            if let CompatibilityStatus::Incompatible { message } =
                check_compatibility(parsed.format_version)
            {
                return Err(format!("가져오기 중단: {}", message));
            }
            manifest = Some(parsed);
        }

        let manifest = manifest.ok_or_else(|| "manifest.json을 읽을 수 없습니다.".to_string())?;

        let mut mcp_data: Vec<McpServerRecord> = Vec::new();
        if let Ok(mut entry) = archive.by_name("mcp_servers.json") {
            let mut content = String::new();
            entry
                .read_to_string(&mut content)
                .map_err(|e| e.to_string())?;
            mcp_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        }

        let mut assistants_data: Vec<AssistantRecord> = Vec::new();
        if let Ok(mut entry) = archive.by_name("assistants.json") {
            let mut content = String::new();
            entry
                .read_to_string(&mut content)
                .map_err(|e| e.to_string())?;
            assistants_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        }

        (manifest, mcp_data, assistants_data)
    };
    emit_progress(window, 15);

    let base_dir = get_session_manager()?.get_base_data_dir();
    let db_path = base_dir.join("libragent.db");
    let backup_manager = BackupManager::new(db_path);
    let backup_file = backup_manager
        .create_backup(db)
        .await
        .map_err(|e| e.to_string())?;

    let txn = db.begin().await.map_err(|e| e.to_string())?;
    emit_progress(window, 25);

    let mut reports = HashMap::new();
    let mut total_imported = 0usize;
    let mut total_skipped = 0usize;
    let mut total_errors = 0usize;

    let report_mcp = import_mcp_data(&txn, mcp_data, &conflict_strategy).await?;
    total_imported += report_mcp.success;
    total_skipped += report_mcp.skipped;
    total_errors += report_mcp.errors.len();
    reports.insert("mcp_servers".to_string(), report_mcp);
    emit_progress(window, 45);

    let report_assistants =
        import_assistants_data(&txn, assistants_data, &conflict_strategy).await?;
    total_imported += report_assistants.success;
    total_skipped += report_assistants.skipped;
    total_errors += report_assistants.errors.len();
    reports.insert("assistants".to_string(), report_assistants);
    emit_progress(window, 60);

    let fk_violations = txn
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check;".to_string(),
        ))
        .await;

    let transaction_ok = match fk_violations {
        Ok(v) if v.is_empty() => true,
        Ok(v) => {
            log::error!(
                "Foreign key constraint violations during org bundle import: {:?}",
                v
            );
            false
        }
        Err(e) => {
            log::error!("Failed to check foreign key integrity: {}", e);
            false
        }
    };

    if !(transaction_ok && total_errors == 0) {
        txn.rollback().await.ok();
        backup_manager.restore_from_backup(&backup_file).ok();
        return Err("Org 번들 가져오기 중 데이터베이스 오류가 발생해 원복했습니다.".to_string());
    }

    txn.commit().await.map_err(|e| e.to_string())?;
    emit_progress(window, 70);

    // Re-open archive for filesystem sections
    let cursor = std::io::Cursor::new(&zip_bytes[..]);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;

    let sanitized = sanitize_org_id_for_path(&manifest.org_id);
    let teamwork_import_path = base_dir
        .join("imported-orgs")
        .join(&sanitized)
        .join("teamwork");

    if conflict_strategy == "overwrite" && teamwork_import_path.exists() {
        fs::remove_dir_all(&teamwork_import_path).ok();
    }

    let (tw_success, tw_skipped, tw_errors) = extract_zip_prefix(
        &mut archive,
        "teamwork",
        &teamwork_import_path,
        &conflict_strategy,
    )?;
    total_imported += tw_success;
    total_skipped += tw_skipped;
    total_errors += tw_errors.len();
    reports.insert(
        "teamwork".to_string(),
        MigrationSectionReport {
            success: tw_success,
            skipped: tw_skipped,
            errors: tw_errors,
        },
    );
    emit_progress(window, 85);

    let mut skills_success = 0usize;
    let mut skills_skipped = 0usize;
    let mut skills_errors = Vec::new();

    for assistant_id in &manifest.assistant_ids {
        // The id comes from the bundle and is joined into a filesystem path.
        if sanitize_org_id_for_path(assistant_id) != *assistant_id {
            skills_errors.push(format!(
                "Invalid assistant id in bundle: '{}'",
                assistant_id
            ));
            continue;
        }
        let zip_prefix = format!("assistant_skills/{}", assistant_id);
        if !archive_has_file_prefix(&archive, &zip_prefix) {
            continue;
        }
        let target = match get_assistant_skills_directory(assistant_id) {
            Ok(path) => path,
            Err(e) => {
                skills_errors.push(format!(
                    "Failed to resolve assistant skills dir for '{}': {}",
                    assistant_id, e
                ));
                continue;
            }
        };

        // Only wipe local skills when the bundle actually carries replacements.
        if conflict_strategy == "overwrite" && target.exists() {
            fs::remove_dir_all(&target).ok();
        }

        match extract_zip_prefix(&mut archive, &zip_prefix, &target, &conflict_strategy) {
            Ok((s, k, errs)) => {
                skills_success += s;
                skills_skipped += k;
                skills_errors.extend(errs);
            }
            Err(e) => skills_errors.push(e),
        }
    }

    total_imported += skills_success;
    total_skipped += skills_skipped;
    total_errors += skills_errors.len();
    reports.insert(
        "assistant_skills".to_string(),
        MigrationSectionReport {
            success: skills_success,
            skipped: skills_skipped,
            errors: skills_errors,
        },
    );
    emit_progress(window, 92);

    let mut workspace_skills_import_path = None;
    if archive_has_file_prefix(&archive, "workspace_skills") {
        let ws_skills_path = base_dir
            .join("imported-orgs")
            .join(&sanitized)
            .join("workspace_skills");
        if conflict_strategy == "overwrite" && ws_skills_path.exists() {
            fs::remove_dir_all(&ws_skills_path).ok();
        }
        let (ws_success, ws_skipped, ws_errors) = extract_zip_prefix(
            &mut archive,
            "workspace_skills",
            &ws_skills_path,
            &conflict_strategy,
        )?;
        total_imported += ws_success;
        total_skipped += ws_skipped;
        total_errors += ws_errors.len();
        reports.insert(
            "workspace_skills".to_string(),
            MigrationSectionReport {
                success: ws_success,
                skipped: ws_skipped,
                errors: ws_errors,
            },
        );
        workspace_skills_import_path = Some(ws_skills_path.to_string_lossy().into_owned());
    }

    emit_progress(window, 100);

    Ok(OrgBundleImportResult {
        org_id: manifest.org_id,
        org_name: manifest.org_name,
        teamwork_import_path: teamwork_import_path.to_string_lossy().into_owned(),
        workspace_skills_import_path,
        sections_imported: reports,
        total_imported,
        total_skipped,
        total_errors,
    })
}
