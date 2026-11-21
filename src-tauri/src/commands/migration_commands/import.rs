use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Set,
    Statement, TransactionTrait,
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
        // #2923 (D2, gatekeeper G1.1): a replace import must not wipe
        // plugin-owned assistants; those rows live and die with their
        // plugin (uninstall removes them).
        delete_non_plugin_assistants(&txn)
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

/// Delete every assistant row that is NOT plugin-owned (#2923 D2, gatekeeper
/// G1.1). Used by the replace ("overwrite") import instead of a blanket
/// `delete_many`, so a replace import can never wipe the rows a plugin owns;
/// those are removed only by the plugin's uninstall flow.
pub(crate) async fn delete_non_plugin_assistants(
    txn: &sea_orm::DatabaseTransaction,
) -> Result<u64, sea_orm::DbErr> {
    use crate::services::skill_service::{is_plugin_agent_id, plugin_source_of_config};

    let owned: Vec<String> = assistant::Entity::find()
        .all(txn)
        .await?
        .into_iter()
        .filter(|row| is_plugin_agent_id(&row.id) || plugin_source_of_config(&row.config).is_some())
        .map(|row| row.id)
        .collect();

    let mut delete = assistant::Entity::delete_many();
    if !owned.is_empty() {
        delete = delete.filter(assistant::Column::Id.is_not_in(owned));
    }
    Ok(delete.exec(txn).await?.rows_affected)
}

pub(crate) async fn import_assistants_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<AssistantRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    use crate::services::skill_service::{is_plugin_agent_id, plugin_source_of_config};

    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();

        // #2923 (D2, gatekeeper G1.1): an import may not create or overwrite
        // plugin-owned assistants, and may not spoof the ownership marker.
        // Same rule #2924 applied to MCP servers: skip with an error entry.
        let incoming_config: Option<serde_json::Value> = serde_json::from_str(&r.config).ok();
        if let Err(reason) = crate::services::skill_service::reject_plugin_agent_mutation(
            &id,
            incoming_config.as_ref(),
        ) {
            errors.push(format!("Skipped assistant '{}': {}", r.name, reason));
            continue;
        }

        let existing = assistant::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // An existing plugin-owned row is read-only no matter which
        // conflict strategy is asked for.
        if let Some(existing_model) = existing.as_ref() {
            if is_plugin_agent_id(&existing_model.id)
                || plugin_source_of_config(&existing_model.config).is_some()
            {
                errors.push(format!(
                    "Skipped assistant '{}': owned by a plugin (read-only)",
                    r.name
                ));
                continue;
            }
        }

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

pub(crate) async fn import_mcp_data(
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

        // #2924 (D2): an import must never overwrite a plugin-owned server;
        // those rows change only with their plugin.
        if let Some(existing_model) = existing.as_ref() {
            if let Some(owner) = crate::mcp::types::plugin_source_of(&existing_model.config) {
                errors.push(format!(
                    "Skipped MCP server '{}': owned by plugin '{}' (read-only)",
                    r.name, owner
                ));
                continue;
            }
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
                secret_names: Set(None),
                encrypted_secrets: Set(None),
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
            active.default_target_session = Set(r.default_target_session);
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
                default_target_session: Set(r.default_target_session),
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
            active.reset_planning_state = Set(r.reset_planning_state);
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
                reset_planning_state: Set(r.reset_planning_state),
                enabled: Set(r.enabled),
                last_run_at: Set(r.last_run_at),
                next_run_at: Set(r.next_run_at),
                run_claim_run: Set(None),
                run_claim_expires_at: Set(None),
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

// #2924 (D2, gatekeeper G1.4): a migration import must never overwrite a
// plugin-owned MCP server row — those change only with their plugin.
#[cfg(test)]
mod mcp_import_guard_tests {
    use super::*;

    async fn txn() -> sea_orm::DatabaseTransaction {
        use sea_orm_migration::MigratorTrait;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db.begin().await.expect("transaction")
    }

    fn record(id: &str, config: &str) -> McpServerRecord {
        McpServerRecord {
            id: id.to_string(),
            name: format!("imported-{id}"),
            config: config.to_string(),
            tool_count: None,
            cached_tools: None,
            verification_status: None,
            last_verification_error: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[tokio::test]
    async fn overwrite_import_skips_plugin_owned_rows() {
        let t = txn().await;
        let owned_config = serde_json::json!({
            "name": "from-plugin",
            "transport": { "type": "http", "url": "https://plugin.example/mcp" },
            "pluginSource": "plugin:plugin-ff00000000000001"
        })
        .to_string();
        let active = mcp_server::ActiveModel {
            id: Set("srv-plugin-owned".to_string()),
            name: Set("from-plugin".to_string()),
            config: Set(owned_config.clone()),
            tool_count: Set(None),
            cached_tools: Set(None),
            verification_status: Set(None),
            last_verification_error: Set(None),
            secret_names: Set(None),
            encrypted_secrets: Set(None),
            created_at: Set(0),
            updated_at: Set(0),
        };
        active.insert(&t).await.expect("seed owned row");

        let report = import_mcp_data(
            &t,
            vec![record(
                "srv-plugin-owned",
                r#"{"name":"evil","transport":{"type":"http","url":"https://evil.example"}}"#,
            )],
            "overwrite",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 0);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(
            report.errors[0].contains("owned by plugin"),
            "{:?}",
            report.errors
        );

        let row = mcp_server::Entity::find_by_id("srv-plugin-owned")
            .one(&t)
            .await
            .unwrap()
            .expect("row still exists");
        assert_eq!(row.config, owned_config, "owned config must survive import");
    }

    #[tokio::test]
    async fn overwrite_import_still_updates_normal_rows() {
        let t = txn().await;
        let active = mcp_server::ActiveModel {
            id: Set("srv-normal".to_string()),
            name: Set("old-name".to_string()),
            config: Set(
                r#"{"name":"old","transport":{"type":"http","url":"https://old.example"}}"#
                    .to_string(),
            ),
            tool_count: Set(None),
            cached_tools: Set(None),
            verification_status: Set(None),
            last_verification_error: Set(None),
            secret_names: Set(None),
            encrypted_secrets: Set(None),
            created_at: Set(0),
            updated_at: Set(0),
        };
        active.insert(&t).await.expect("seed normal row");

        let report = import_mcp_data(
            &t,
            vec![record(
                "srv-normal",
                r#"{"name":"new","transport":{"type":"http","url":"https://new.example"}}"#,
            )],
            "overwrite",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 1, "{:?}", report.errors);
        let row = mcp_server::Entity::find_by_id("srv-normal")
            .one(&t)
            .await
            .unwrap()
            .expect("row still exists");
        assert_eq!(row.name, "imported-srv-normal");
    }
}

// #2923 (D2, gatekeeper G1.1): a migration import must never create,
// overwrite, or wipe plugin-owned assistant rows — those change only with
// their plugin (install/update sync and uninstall).
#[cfg(test)]
mod assistant_import_guard_tests {
    use super::*;

    async fn txn() -> sea_orm::DatabaseTransaction {
        use sea_orm_migration::MigratorTrait;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db.begin().await.expect("transaction")
    }

    fn record(id: &str, name: &str, config: &str) -> AssistantRecord {
        AssistantRecord {
            id: id.to_string(),
            name: name.to_string(),
            config: config.to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    async fn seed(t: &sea_orm::DatabaseTransaction, id: &str, name: &str, config: &str) {
        let active = assistant::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(111),
            updated_at: Set(222),
        };
        active.insert(t).await.expect("seed row");
    }

    fn owned_config() -> String {
        serde_json::json!({
            "systemPrompt": "from-plugin",
            "pluginSource": "plugin:plugin-ff00000000000001"
        })
        .to_string()
    }

    #[tokio::test]
    async fn overwrite_import_leaves_owned_row_byte_identical() {
        let t = txn().await;
        let config = owned_config();
        seed(
            &t,
            "plugin:plugin-ff00000000000001:reviewer",
            "reviewer",
            &config,
        )
        .await;

        let report = import_assistants_data(
            &t,
            vec![record(
                "plugin:plugin-ff00000000000001:reviewer",
                "evil",
                r#"{"systemPrompt":"evil"}"#,
            )],
            "overwrite",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 0);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(
            report.errors[0].contains("read-only"),
            "{:?}",
            report.errors
        );

        let row = assistant::Entity::find_by_id("plugin:plugin-ff00000000000001:reviewer")
            .one(&t)
            .await
            .unwrap()
            .expect("owned row must survive the import");
        assert_eq!(row.config, config, "owned config must be byte-identical");
        assert_eq!(row.name, "reviewer");
        assert_eq!(row.created_at, 111);
        assert_eq!(row.updated_at, 222);
    }

    #[tokio::test]
    async fn import_rejects_records_with_spoofed_marker() {
        let t = txn().await;
        let report = import_assistants_data(
            &t,
            vec![record(
                "user-row",
                "spoofed",
                r#"{"systemPrompt":"x","pluginSource":"plugin:victim"}"#,
            )],
            "skip",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 0);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(
            report.errors[0].contains("pluginSource"),
            "{:?}",
            report.errors
        );
        assert!(assistant::Entity::find_by_id("user-row")
            .one(&t)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn replace_delete_keeps_owned_rows_and_deletes_user_rows() {
        let t = txn().await;
        let config = owned_config();
        seed(
            &t,
            "plugin:plugin-ff00000000000001:reviewer",
            "reviewer",
            &config,
        )
        .await;
        seed(&t, "user-1", "user one", r#"{"systemPrompt":"mine"}"#).await;

        let deleted = delete_non_plugin_assistants(&t)
            .await
            .expect("replace delete");
        assert_eq!(deleted, 1);
        assert!(
            assistant::Entity::find_by_id("plugin:plugin-ff00000000000001:reviewer")
                .one(&t)
                .await
                .unwrap()
                .is_some()
        );
        assert!(assistant::Entity::find_by_id("user-1")
            .one(&t)
            .await
            .unwrap()
            .is_none());
    }
}
