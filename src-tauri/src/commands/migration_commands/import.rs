use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DbBackend, EntityTrait, Set, Statement, TransactionTrait,
};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use tauri::{command, Emitter};

use crate::entity::{assistant, mcp_server, playbook, scheduled_task, settings};
use crate::lifecycle::database_backup::BackupManager;
use crate::mcp::builtin::utils::SecurityValidator;
use crate::services::skill_service::get_user_skills_directory;
use crate::state::get_database_connection;

use super::crypto::decrypt_data;
use super::inspect::check_compatibility;
use super::models::{
    AssistantRecord, CompatibilityStatus, McpServerRecord, MigrationImportResult,
    MigrationSectionReport, PlaybookRecord, ScheduledTaskRecord, SettingsRecord,
};

#[command]
pub async fn import_migration(
    window: tauri::Window,
    file_path: String,
    conflict_strategy: String, // "skip" | "overwrite" | "merge"
    password: Option<String>,
) -> Result<MigrationImportResult, String> {
    window.emit("migration:progress", 5).ok();

    // Read file bytes
    let file_bytes = fs::read(&file_path).map_err(|e| format!("파일을 읽을 수 없습니다: {}", e))?;

    // Decrypt if necessary
    let zip_bytes = match decrypt_data(&file_bytes, password.as_deref())? {
        Some(decrypted) => decrypted,
        None => file_bytes,
    };

    // Open from decrypted memory cursor
    let cursor = std::io::Cursor::new(&zip_bytes[..]);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;

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

    // Validate Strategy
    if conflict_strategy != "skip"
        && conflict_strategy != "overwrite"
        && conflict_strategy != "merge"
    {
        return Err("지원되지 않는 충돌 해결 전략입니다.".to_string());
    }

    let db = get_database_connection();

    // 1. Create a Database Backup before changing state
    let base_dir = crate::session::get_session_manager()?.get_base_data_dir();
    let db_path = base_dir.join("libragent.db");
    let backup_manager = BackupManager::new(db_path.clone());
    let backup_file = backup_manager
        .create_backup(db)
        .await
        .map_err(|e| e.to_string())?;

    window.emit("migration:progress", 20).ok();

    // 2. Disable Foreign Keys on database connection *outside* transaction
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys = OFF;".to_string(),
    ))
    .await
    .map_err(|e| e.to_string())?;

    // Start single transaction for atomic DB updates
    let txn = db.begin().await.map_err(|e| e.to_string())?;

    // --- REVERSE DEPENDENCY DELETION (For overwrite strategy) ---
    if conflict_strategy == "overwrite" {
        scheduled_task::Entity::delete_many()
            .exec(&txn)
            .await
            .map_err(|e| e.to_string())?;
        playbook::Entity::delete_many()
            .exec(&txn)
            .await
            .map_err(|e| e.to_string())?;
        assistant::Entity::delete_many()
            .exec(&txn)
            .await
            .map_err(|e| e.to_string())?;
        mcp_server::Entity::delete_many()
            .exec(&txn)
            .await
            .map_err(|e| e.to_string())?;
        settings::Entity::delete_many()
            .exec(&txn)
            .await
            .map_err(|e| e.to_string())?;
    }

    window.emit("migration:progress", 30).ok();

    let mut reports = HashMap::new();
    let mut total_imported = 0;
    let mut total_skipped = 0;
    let mut total_errors = 0;

    // --- IMPORT SETTINGS ---
    let mut settings_data: Vec<SettingsRecord> = Vec::new();
    if let Ok(mut entry) = archive.by_name("settings.json") {
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| e.to_string())?;
        settings_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    }
    let report_settings = import_settings_data(&txn, settings_data, &conflict_strategy).await?;
    total_imported += report_settings.success;
    total_skipped += report_settings.skipped;
    total_errors += report_settings.errors.len();
    reports.insert("settings".to_string(), report_settings);

    window.emit("migration:progress", 45).ok();

    // --- IMPORT MCP SERVERS ---
    let mut mcp_data: Vec<McpServerRecord> = Vec::new();
    if let Ok(mut entry) = archive.by_name("mcp_servers.json") {
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| e.to_string())?;
        mcp_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    }
    let report_mcp = import_mcp_data(&txn, mcp_data, &conflict_strategy).await?;
    total_imported += report_mcp.success;
    total_skipped += report_mcp.skipped;
    total_errors += report_mcp.errors.len();
    reports.insert("mcp_servers".to_string(), report_mcp);

    window.emit("migration:progress", 60).ok();

    // --- IMPORT ASSISTANTS ---
    let mut assistants_data: Vec<AssistantRecord> = Vec::new();
    if let Ok(mut entry) = archive.by_name("assistants.json") {
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| e.to_string())?;
        assistants_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    }
    let report_assistants =
        import_assistants_data(&txn, assistants_data, &conflict_strategy).await?;
    total_imported += report_assistants.success;
    total_skipped += report_assistants.skipped;
    total_errors += report_assistants.errors.len();
    reports.insert("assistants".to_string(), report_assistants);

    window.emit("migration:progress", 75).ok();

    // --- IMPORT PLAYBOOKS ---
    let mut playbooks_data: Vec<PlaybookRecord> = Vec::new();
    if let Ok(mut entry) = archive.by_name("playbooks.json") {
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| e.to_string())?;
        playbooks_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    }
    let report_playbooks = import_playbooks_data(&txn, playbooks_data, &conflict_strategy).await?;
    total_imported += report_playbooks.success;
    total_skipped += report_playbooks.skipped;
    total_errors += report_playbooks.errors.len();
    reports.insert("playbooks".to_string(), report_playbooks);

    window.emit("migration:progress", 85).ok();

    // --- IMPORT SCHEDULED TASKS ---
    let mut tasks_data: Vec<ScheduledTaskRecord> = Vec::new();
    if let Ok(mut entry) = archive.by_name("scheduled_tasks.json") {
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| e.to_string())?;
        tasks_data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    }
    let report_tasks = import_tasks_data(&txn, tasks_data, &conflict_strategy).await?;
    total_imported += report_tasks.success;
    total_skipped += report_tasks.skipped;
    total_errors += report_tasks.errors.len();
    reports.insert("scheduled_tasks".to_string(), report_tasks);

    window.emit("migration:progress", 90).ok();

    // 3. Dry-run Foreign Key check to validate relational integrity
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
                "Foreign key constraint violations detected during import: {:?}",
                v
            );
            false
        }
        Err(e) => {
            log::error!("Failed to check foreign key integrity: {}", e);
            false
        }
    };

    if transaction_ok && total_errors == 0 {
        // Commit DB transaction
        txn.commit().await.map_err(|e| e.to_string())?;

        // Re-enable Foreign Keys
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_keys = ON;".to_string(),
        ))
        .await
        .ok();

        // --- IMPORT USER SKILLS ---
        let mut skills_success = 0;
        let mut skills_skipped = 0;
        let mut skills_errors = Vec::new();

        if let Ok(user_skills_dir) = get_user_skills_directory() {
            // If overwrite, clear target directory first
            if conflict_strategy == "overwrite" && user_skills_dir.exists() {
                fs::remove_dir_all(&user_skills_dir).ok();
            }
            fs::create_dir_all(&user_skills_dir).ok();

            // Re-open zip from memory cursor to copy skills
            let cursor = std::io::Cursor::new(&zip_bytes[..]);
            let mut archive = zip::ZipArchive::new(cursor)
                .map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;
            let file_names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
            let total_files = file_names.len();

            let security_validator =
                SecurityValidator::new_scoped_with_base_dir(user_skills_dir.clone());

            for (idx, name) in file_names.into_iter().enumerate() {
                if name.starts_with("user_skills/") {
                    let mut entry = archive.by_name(&name).map_err(|e| e.to_string())?;
                    if entry.is_file() {
                        let cleaned_name = name.strip_prefix("user_skills/").unwrap_or(&name);
                        // ZIP Slip Protection
                        let outpath = security_validator
                            .validate_path_for_write(cleaned_name)
                            .map_err(|e| format!("ZIP Slip 탐지: {}", e))?;

                        if outpath.exists()
                            && (conflict_strategy == "skip" || conflict_strategy == "merge")
                        {
                            skills_skipped += 1;
                            continue;
                        }

                        if let Some(parent) = outpath.parent() {
                            fs::create_dir_all(parent).ok();
                        }

                        let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
                        if let Err(e) = std::io::copy(&mut entry, &mut outfile) {
                            skills_errors.push(format!(
                                "Failed to copy skill file {:?}: {}",
                                cleaned_name, e
                            ));
                        } else {
                            skills_success += 1;
                        }
                    }
                }
                if let Some(div) = ((idx + 1) * 10).checked_div(total_files) {
                    let progress = 90 + div;
                    window.emit("migration:progress", progress as u32).ok();
                }
            }
        }

        total_imported += skills_success;
        total_skipped += skills_skipped;
        total_errors += skills_errors.len();

        reports.insert(
            "user_skills".to_string(),
            MigrationSectionReport {
                success: skills_success,
                skipped: skills_skipped,
                errors: skills_errors,
            },
        );

        window.emit("migration:progress", 100).ok();

        Ok(MigrationImportResult {
            sections_imported: reports,
            total_imported,
            total_skipped,
            total_errors,
        })
    } else {
        // Rollback Transaction
        txn.rollback().await.ok();

        // Restore SQLite FK checks
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_keys = ON;".to_string(),
        ))
        .await
        .ok();

        // Restore Database from WAL-safe backup in case of filesystem state inconsistency
        backup_manager.restore_from_backup(&backup_file).ok();

        let err_msg = if total_errors > 0 {
            "데이터를 파싱하거나 데이터베이스에 쓰는 도중 오류가 발생해 원복했습니다.".to_string()
        } else {
            "데이터베이스 참조 무결성(외래키 제약조건) 위반으로 인해 가져오기가 중단되었습니다."
                .to_string()
        };

        Err(err_msg)
    }
}

// --- Internal DB import logic helper functions ---

async fn import_settings_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<SettingsRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let key = r.key.clone();
        let existing = settings::Entity::find_by_id(&key)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        match (existing, strategy) {
            (Some(_), "skip") => {
                skipped += 1;
            }
            (Some(existing_model), "merge") => {
                let merged_value = if let (Ok(mut old_obj), Ok(new_obj)) = (
                    serde_json::from_str::<serde_json::Value>(&existing_model.value),
                    serde_json::from_str::<serde_json::Value>(&r.value),
                ) {
                    if let (Some(old_map), Some(new_map)) =
                        (old_obj.as_object_mut(), new_obj.as_object())
                    {
                        for (k, v) in new_map {
                            old_map.insert(k.clone(), v.clone());
                        }
                        serde_json::to_string(&old_obj).unwrap_or(r.value)
                    } else {
                        r.value
                    }
                } else {
                    r.value
                };

                let mut active: settings::ActiveModel = existing_model.into();
                active.value = Set(merged_value);
                active.updated_at = Set(chrono::Utc::now().timestamp_millis());
                if let Err(e) = active.update(txn).await {
                    errors.push(format!("Failed to merge setting '{}': {}", key, e));
                } else {
                    success += 1;
                }
            }
            (existing_opt, _) => {
                let now = chrono::Utc::now().timestamp_millis();
                if let Some(existing_model) = existing_opt {
                    let mut active: settings::ActiveModel = existing_model.into();
                    active.value = Set(r.value);
                    active.updated_at = Set(now);
                    if let Err(e) = active.update(txn).await {
                        errors.push(format!("Failed to overwrite setting '{}': {}", key, e));
                    } else {
                        success += 1;
                    }
                } else {
                    let active = settings::ActiveModel {
                        key: Set(r.key),
                        value: Set(r.value),
                        created_at: Set(r.created_at),
                        updated_at: Set(now),
                    };
                    if let Err(e) = settings::Entity::insert(active).exec(txn).await {
                        errors.push(format!("Failed to insert setting '{}': {}", key, e));
                    } else {
                        success += 1;
                    }
                }
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

async fn import_assistants_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<AssistantRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();
        let existing = assistant::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        if let Some(existing_model) = existing {
            let mut active: assistant::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.config = Set(r.config);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update assistant '{}': {}", id, e));
            } else {
                success += 1;
            }
        } else {
            let active = assistant::ActiveModel {
                id: Set(r.id),
                name: Set(r.name),
                config: Set(r.config),
                created_at: Set(r.created_at),
                updated_at: Set(now),
            };
            if let Err(e) = assistant::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert assistant '{}': {}", id, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

async fn import_mcp_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<McpServerRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();
        let existing = mcp_server::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        if let Some(existing_model) = existing {
            let mut active: mcp_server::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.config = Set(r.config);
            active.tool_count = Set(r.tool_count);
            active.cached_tools = Set(r.cached_tools);
            active.verification_status = Set(r.verification_status);
            active.last_verification_error = Set(r.last_verification_error);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update MCP server '{}': {}", id, e));
            } else {
                success += 1;
            }
        } else {
            let active = mcp_server::ActiveModel {
                id: Set(r.id),
                name: Set(r.name),
                config: Set(r.config),
                tool_count: Set(r.tool_count),
                cached_tools: Set(r.cached_tools),
                verification_status: Set(r.verification_status),
                last_verification_error: Set(r.last_verification_error),
                created_at: Set(r.created_at),
                updated_at: Set(now),
            };
            if let Err(e) = mcp_server::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert MCP server '{}': {}", id, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

async fn import_playbooks_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<PlaybookRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let pk = (r.id.clone(), r.assistant_id.clone());
        let existing = playbook::Entity::find_by_id(pk.clone())
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        if let Some(existing_model) = existing {
            let mut active: playbook::ActiveModel = existing_model.into();
            active.goal = Set(r.goal);
            active.initial_command = Set(r.initial_command);
            active.workflow = Set(r.workflow);
            active.success_criteria = Set(r.success_criteria);
            active.is_bookmarked = Set(r.is_bookmarked);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update playbook '{:?}': {}", pk, e));
            } else {
                success += 1;
            }
        } else {
            let active = playbook::ActiveModel {
                id: Set(r.id),
                assistant_id: Set(r.assistant_id),
                goal: Set(r.goal),
                initial_command: Set(r.initial_command),
                workflow: Set(r.workflow),
                success_criteria: Set(r.success_criteria),
                created_at: Set(r.created_at),
                updated_at: Set(now),
                is_bookmarked: Set(r.is_bookmarked),
            };
            if let Err(e) = playbook::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert playbook '{:?}': {}", pk, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

async fn import_tasks_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<ScheduledTaskRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();
        let existing = scheduled_task::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        let execution_mode = r.resolved_execution_mode();
        if let Some(existing_model) = existing {
            let mut active: scheduled_task::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.task_category = Set(r.task_category);
            active.cron_expression = Set(r.cron_expression);
            active.schedule_timezone = Set(r.schedule_timezone);
            active.assistant_id = Set(r.assistant_id);
            active.message = Set(r.message);
            active.execution_mode = Set(execution_mode.clone());
            active.created_by_session_id = Set(r.created_by_session_id);
            active.session_id = Set(r.session_id);
            active.workspace_override = Set(r.workspace_override);
            active.enabled = Set(r.enabled);
            active.last_run_at = Set(r.last_run_at);
            active.next_run_at = Set(r.next_run_at);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update scheduled task '{}': {}", id, e));
            } else {
                success += 1;
            }
        } else {
            let active = scheduled_task::ActiveModel {
                id: Set(r.id),
                name: Set(r.name),
                task_category: Set(r.task_category),
                cron_expression: Set(r.cron_expression),
                schedule_timezone: Set(r.schedule_timezone),
                assistant_id: Set(r.assistant_id),
                message: Set(r.message),
                execution_mode: Set(execution_mode),
                created_by_session_id: Set(r.created_by_session_id),
                session_id: Set(r.session_id),
                workspace_override: Set(r.workspace_override),
                enabled: Set(r.enabled),
                last_run_at: Set(r.last_run_at),
                next_run_at: Set(r.next_run_at),
                created_at: Set(r.created_at),
                updated_at: Set(now),
            };
            if let Err(e) = scheduled_task::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert scheduled task '{}': {}", id, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}
