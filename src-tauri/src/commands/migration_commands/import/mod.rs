mod db;
mod entities;
mod skills;
mod tables;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_fk;

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use tauri::{command, Emitter};

use crate::lifecycle::database_backup::BackupManager;
use crate::state::get_database_connection;

use super::crypto::decrypt_data;
use super::inspect::check_compatibility;
use super::models::{
    AssistantRecord, CompatibilityStatus, McpServerRecord, MigrationImportResult,
    MigrationSectionReport, PlaybookRecord, ScheduledTaskRecord, SettingsRecord,
};

pub(crate) use entities::{import_assistants_data, import_mcp_data};

fn read_and_decrypt_zip(file_path: &str, password: Option<&str>) -> Result<Vec<u8>, String> {
    let file_bytes = fs::read(file_path).map_err(|e| format!("파일을 읽을 수 없습니다: {}", e))?;
    match decrypt_data(&file_bytes, password)? {
        Some(decrypted) => Ok(decrypted),
        None => Ok(file_bytes),
    }
}

fn validate_archive_and_strategy(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    conflict_strategy: &str,
) -> Result<(), String> {
    let mut format_version = 1;
    if let Ok(mut manifest_entry) = archive.by_name("manifest.json") {
        let mut content = String::new();
        if manifest_entry.read_to_string(&mut content).is_ok() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                format_version = v
                    .get("format_version")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(1) as u32;
            }
        }
    }

    if let CompatibilityStatus::Incompatible { message } = check_compatibility(format_version) {
        return Err(format!("가져오기 중단: {}", message));
    }

    if conflict_strategy != "skip"
        && conflict_strategy != "overwrite"
        && conflict_strategy != "merge"
    {
        return Err("지원되지 않는 충돌 해결 전략입니다.".to_string());
    }

    Ok(())
}

fn read_json_entry<T: serde::de::DeserializeOwned>(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    entry_name: &str,
) -> Result<Vec<T>, String> {
    let mut data = Vec::new();
    if let Ok(mut entry) = archive.by_name(entry_name) {
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| e.to_string())?;
        data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    }
    Ok(data)
}

#[derive(Default)]
struct SectionTotals {
    imported: usize,
    skipped: usize,
    errors: usize,
}

impl SectionTotals {
    fn record(
        &mut self,
        reports: &mut HashMap<String, MigrationSectionReport>,
        key: &str,
        report: MigrationSectionReport,
    ) {
        self.imported += report.success;
        self.skipped += report.skipped;
        self.errors += report.errors.len();
        reports.insert(key.to_string(), report);
    }
}

#[command]
pub async fn import_migration(
    window: tauri::Window,
    file_path: String,
    conflict_strategy: String, // "skip" | "overwrite" | "merge"
    password: Option<String>,
) -> Result<MigrationImportResult, String> {
    // #3039: the import runs with foreign-key checks off, and the connection
    // that ran it goes back into the pool. Wrapping the whole body means every
    // early `?` below restores the pragma too, not only the commit and the
    // explicit rollback.
    let db = get_database_connection();
    super::fk::with_foreign_keys_disabled(
        db,
        run_import_migration(window, file_path, conflict_strategy, password),
    )
    .await
}

async fn run_import_migration(
    window: tauri::Window,
    file_path: String,
    conflict_strategy: String, // "skip" | "overwrite" | "merge"
    password: Option<String>,
) -> Result<MigrationImportResult, String> {
    window.emit("migration:progress", 5).ok();

    let zip_bytes = read_and_decrypt_zip(&file_path, password.as_deref())?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes[..]))
        .map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;
    validate_archive_and_strategy(&mut archive, &conflict_strategy)?;

    let db = get_database_connection();
    let base_dir = crate::session::get_session_manager()?.get_base_data_dir();
    let backup_manager = BackupManager::new(base_dir.join("libragent.db"));
    let backup_file = backup_manager
        .create_backup(db)
        .await
        .map_err(|e| e.to_string())?;

    window.emit("migration:progress", 20).ok();

    let txn = db::begin_import_transaction(db).await?;
    if conflict_strategy == "overwrite" {
        db::clear_existing_rows_for_overwrite(&txn).await?;
    }

    window.emit("migration:progress", 30).ok();

    let mut reports = HashMap::new();
    let mut totals = SectionTotals::default();

    let settings_data: Vec<SettingsRecord> = read_json_entry(&mut archive, "settings.json")?;
    let report = tables::import_settings_data(&txn, settings_data, &conflict_strategy).await?;
    totals.record(&mut reports, "settings", report);
    window.emit("migration:progress", 45).ok();

    let mcp_data: Vec<McpServerRecord> = read_json_entry(&mut archive, "mcp_servers.json")?;
    let report = entities::import_mcp_data(&txn, mcp_data, &conflict_strategy).await?;
    totals.record(&mut reports, "mcp_servers", report);
    window.emit("migration:progress", 60).ok();

    let assistants_data: Vec<AssistantRecord> = read_json_entry(&mut archive, "assistants.json")?;
    let report =
        entities::import_assistants_data(&txn, assistants_data, &conflict_strategy).await?;
    totals.record(&mut reports, "assistants", report);
    window.emit("migration:progress", 75).ok();

    let playbooks_data: Vec<PlaybookRecord> = read_json_entry(&mut archive, "playbooks.json")?;
    let report = tables::import_playbooks_data(&txn, playbooks_data, &conflict_strategy).await?;
    totals.record(&mut reports, "playbooks", report);
    window.emit("migration:progress", 85).ok();

    let tasks_data: Vec<ScheduledTaskRecord> =
        read_json_entry(&mut archive, "scheduled_tasks.json")?;
    let report = tables::import_tasks_data(&txn, tasks_data, &conflict_strategy).await?;
    totals.record(&mut reports, "scheduled_tasks", report);
    window.emit("migration:progress", 90).ok();

    let transaction_ok = db::validate_foreign_keys(&txn).await;

    if transaction_ok && totals.errors == 0 {
        db::commit_import_transaction(txn).await?;
        let report_skills = skills::restore_user_skills(&window, &zip_bytes, &conflict_strategy)?;
        totals.record(&mut reports, "user_skills", report_skills);
        window.emit("migration:progress", 100).ok();

        Ok(MigrationImportResult {
            sections_imported: reports,
            total_imported: totals.imported,
            total_skipped: totals.skipped,
            total_errors: totals.errors,
        })
    } else {
        db::rollback_and_restore_backup(txn, &backup_manager, &backup_file, totals.errors).await
    }
}
