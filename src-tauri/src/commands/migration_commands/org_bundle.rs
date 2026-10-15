//! Portable Org bundle export/import (capability + teamwork SSOT + skill layers).
//!
//! Includes: referenced assistants/MCP, assistant skills, org-root workspace skills,
//! and `teamwork-artifacts/<root>/` (scaffold + deliverables).
//! Does **not** transfer session messages or full workspace trees (v1).

use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, Statement, TransactionTrait};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use tauri::{command, Emitter};

use crate::entity::{assistant, mcp_server};
use crate::lifecycle::database_backup::BackupManager;
use crate::repositories::SessionRepository;
use crate::services::skill_service::{
    get_assistant_skills_directory, get_workspace_skills_directory_for_session,
};
use crate::session::{get_session_manager, teamwork_artifact_dir_for_session};
use crate::state::{get_database_connection, get_session_repository};

use super::crypto::{decrypt_data, encrypt_data};
use super::import::{import_assistants_data, import_mcp_data};
use super::inspect::check_compatibility;
use super::models::{
    AssistantRecord, CompatibilityStatus, McpServerRecord, MigrationSectionReport,
    OrgBundleExportInfo, OrgBundleImportResult, OrgBundleManifest, OrgBundlePreview,
    SectionPreview, CURRENT_FORMAT_VERSION, ORG_BUNDLE_KIND,
};
use super::sanitize::{clean_assistants, clean_mcp_servers};
use super::zip_fs::{
    archive_has_file_prefix, extract_zip_prefix, validate_export_output_path,
    zip_directory_under_prefix,
};

/// Extract `mcpServerIds` from an assistant config JSON string.
pub fn extract_mcp_server_ids_from_config(config: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(config) else {
        return Vec::new();
    };
    value
        .get("mcpServerIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Sanitize org id for use as a single path segment under `imported-orgs/`.
pub fn sanitize_org_id_for_path(org_id: &str) -> String {
    let cleaned: String = org_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown-org".to_string()
    } else {
        cleaned
    }
}

fn emit_progress(window: &tauri::Window, progress: u32) {
    window.emit("org-bundle:progress", progress.min(100)).ok();
}

async fn resolve_org_sessions(
    org_id: &str,
) -> Result<(String, String, String, Vec<String>), String> {
    let sessions = get_session_repository()
        .get_all_sessions()
        .await
        .map_err(|e| e.to_string())?;

    let org_sessions: Vec<_> = sessions
        .into_iter()
        .filter(|s| s.org_id.as_deref() == Some(org_id))
        .collect();

    if org_sessions.is_empty() {
        return Err(format!("Org not found: {}", org_id));
    }

    let root = org_sessions
        .iter()
        .find(|s| {
            s.org_root_session_id
                .as_deref()
                .is_some_and(|root_id| root_id == s.id)
        })
        .ok_or_else(|| {
            format!(
                "Org '{}' has no root session with matching org_root_session_id",
                org_id
            )
        })?;

    let org_name = root
        .org_name
        .clone()
        .filter(|n| !n.is_empty())
        .ok_or_else(|| format!("Org '{}' is missing org_name", org_id))?;
    let root_session_id = root.id.clone();

    let mut assistant_ids: Vec<String> = org_sessions
        .iter()
        .filter_map(|s| s.assistant_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    assistant_ids.sort();

    Ok((org_id.to_string(), org_name, root_session_id, assistant_ids))
}

#[command]
pub async fn export_org_bundle(
    window: tauri::Window,
    org_id: String,
    output_path: String,
    include_sensitive_data: bool,
    password: Option<String>,
) -> Result<OrgBundleExportInfo, String> {
    if include_sensitive_data && password.is_none() {
        return Err("민감 데이터를 포함할 경우 백업 암호 설정이 필수입니다.".to_string());
    }

    emit_progress(&window, 5);
    validate_export_output_path(&output_path)?;

    let (org_id, org_name, root_session_id, assistant_ids) = resolve_org_sessions(&org_id).await?;
    emit_progress(&window, 15);

    let db = get_database_connection();
    let mut assistant_models = Vec::new();
    for id in &assistant_ids {
        if let Some(model) = assistant::Entity::find_by_id(id.clone())
            .one(db)
            .await
            .map_err(|e| e.to_string())?
        {
            assistant_models.push(model);
        }
    }

    let mut mcp_id_set = HashSet::new();
    for model in &assistant_models {
        for mcp_id in extract_mcp_server_ids_from_config(&model.config) {
            mcp_id_set.insert(mcp_id);
        }
    }
    let mut mcp_ids: Vec<String> = mcp_id_set.into_iter().collect();
    mcp_ids.sort();

    let mut mcp_models = Vec::new();
    for id in &mcp_ids {
        if let Some(model) = mcp_server::Entity::find_by_id(id.clone())
            .one(db)
            .await
            .map_err(|e| e.to_string())?
        {
            mcp_models.push(model);
        }
    }
    emit_progress(&window, 30);

    let cleaned_assistants = clean_assistants(&assistant_models, include_sensitive_data);
    let cleaned_mcp = clean_mcp_servers(&mcp_models, include_sensitive_data);

    let session_manager = get_session_manager()?;
    let teamwork_dir = teamwork_artifact_dir_for_session(session_manager, &root_session_id);

    let temp_zip_path = if include_sensitive_data {
        let temp_dir = std::env::temp_dir();
        temp_dir.join(format!("libragent-org-export-{}.zip", uuid::Uuid::new_v4()))
    } else {
        PathBuf::from(&output_path)
    };

    let file = File::create(&temp_zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let manifest = OrgBundleManifest {
        kind: ORG_BUNDLE_KIND.to_string(),
        format_version: CURRENT_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        org_id: org_id.clone(),
        org_name: org_name.clone(),
        source_root_session_id: root_session_id.clone(),
        assistant_ids: assistant_ids.clone(),
        mcp_ids: mcp_ids.clone(),
    };

    zip.start_file("manifest.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    zip.start_file("assistants.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&cleaned_assistants)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    zip.start_file("mcp_servers.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&cleaned_mcp)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    emit_progress(&window, 45);

    let mut sections = vec![
        "manifest".to_string(),
        "assistants".to_string(),
        "mcp_servers".to_string(),
    ];

    if teamwork_dir.exists() {
        let window_ref = window.clone();
        let mut progress_cb = move |pct: u32| {
            emit_progress(&window_ref, 45 + (pct * 25 / 100));
        };
        let written = zip_directory_under_prefix(
            &teamwork_dir,
            "teamwork",
            &mut zip,
            options,
            Some(&mut progress_cb),
        )?;
        if written > 0 || teamwork_dir.exists() {
            sections.push("teamwork".to_string());
        }
    }
    emit_progress(&window, 75);

    let mut any_assistant_skills = false;
    for assistant_id in &assistant_ids {
        let skills_dir = get_assistant_skills_directory(assistant_id)?;
        if !skills_dir.exists() {
            continue;
        }
        let zip_prefix = format!("assistant_skills/{}", assistant_id);
        let written =
            zip_directory_under_prefix(&skills_dir, &zip_prefix, &mut zip, options, None)?;
        if written > 0 {
            any_assistant_skills = true;
        }
    }
    if any_assistant_skills {
        sections.push("assistant_skills".to_string());
    }
    emit_progress(&window, 85);

    // Org-root workspace skills only (not the full workspaces/<root>/ tree).
    if let Ok(workspace_skills_dir) = get_workspace_skills_directory_for_session(&root_session_id) {
        if workspace_skills_dir.exists() {
            let written = zip_directory_under_prefix(
                &workspace_skills_dir,
                "workspace_skills",
                &mut zip,
                options,
                None,
            )?;
            if written > 0 {
                sections.push("workspace_skills".to_string());
            }
        }
    }
    emit_progress(&window, 92);

    zip.finish().map_err(|e| e.to_string())?;

    let final_file_size = if include_sensitive_data {
        let zip_bytes = fs::read(&temp_zip_path).map_err(|e| e.to_string())?;
        let pwd = password.as_deref().unwrap_or("");
        let encrypted_bytes = encrypt_data(&zip_bytes, pwd)?;
        fs::write(&output_path, &encrypted_bytes).map_err(|e| e.to_string())?;
        fs::remove_file(&temp_zip_path).ok();
        encrypted_bytes.len() as u64
    } else {
        fs::metadata(&output_path).map_err(|e| e.to_string())?.len()
    };

    emit_progress(&window, 100);

    Ok(OrgBundleExportInfo {
        file_path: output_path,
        file_size_bytes: final_file_size,
        org_id,
        org_name,
        sections,
        assistant_ids,
        mcp_ids,
    })
}

#[command]
pub async fn inspect_org_bundle(
    file_path: String,
    password: Option<String>,
) -> Result<OrgBundlePreview, String> {
    let file_bytes = fs::read(&file_path).map_err(|e| format!("파일을 읽을 수 없습니다: {}", e))?;
    let zip_bytes = match decrypt_data(&file_bytes, password.as_deref())? {
        Some(decrypted) => decrypted,
        None => file_bytes,
    };

    let cursor = std::io::Cursor::new(&zip_bytes[..]);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;

    let mut format_version = 1u32;
    let mut app_version = None;
    let mut exported_at = None;
    let mut org_id = None;
    let mut org_name = None;
    let mut source_root_session_id = None;
    let mut assistant_ids = Vec::new();
    let mut mcp_ids = Vec::new();
    let mut kind_ok = false;

    if let Ok(mut manifest_entry) = archive.by_name("manifest.json") {
        let mut content = String::new();
        if manifest_entry.read_to_string(&mut content).is_ok() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if v.get("kind").and_then(|k| k.as_str()) == Some(ORG_BUNDLE_KIND) {
                    kind_ok = true;
                }
                format_version = v
                    .get("format_version")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(1) as u32;
                app_version = v
                    .get("app_version")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                exported_at = v
                    .get("exported_at")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                org_id = v.get("org_id").and_then(|x| x.as_str()).map(str::to_string);
                org_name = v
                    .get("org_name")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                source_root_session_id = v
                    .get("source_root_session_id")
                    .and_then(|x| x.as_str())
                    .map(str::to_string);
                if let Some(arr) = v.get("assistant_ids").and_then(|x| x.as_array()) {
                    assistant_ids = arr
                        .iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect();
                }
                if let Some(arr) = v.get("mcp_ids").and_then(|x| x.as_array()) {
                    mcp_ids = arr
                        .iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect();
                }
            }
        }
    }

    if !kind_ok {
        return Err(
            "이 파일은 Org 번들(org_bundle)이 아닙니다. 환경 Migration 파일과 구분됩니다."
                .to_string(),
        );
    }

    let mut sections = Vec::new();
    let mut total_size_bytes = 0u64;
    let file_names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    let mut teamwork_count = 0usize;
    let mut teamwork_size = 0u64;
    let mut assistant_skills_count = 0usize;
    let mut assistant_skills_size = 0u64;
    let mut workspace_skills_count = 0usize;
    let mut workspace_skills_size = 0u64;

    for name in file_names {
        let entry = archive.by_name(&name).map_err(|e| e.to_string())?;
        let size = entry.size();
        total_size_bytes = total_size_bytes.saturating_add(size);

        if name == "assistants.json" {
            sections.push(SectionPreview {
                name: "assistants".to_string(),
                item_count: assistant_ids.len(),
                size_bytes: size,
            });
        } else if name == "mcp_servers.json" {
            sections.push(SectionPreview {
                name: "mcp_servers".to_string(),
                item_count: mcp_ids.len(),
                size_bytes: size,
            });
        } else if name.starts_with("teamwork/") && entry.is_file() {
            teamwork_count += 1;
            teamwork_size = teamwork_size.saturating_add(size);
        } else if name.starts_with("assistant_skills/") && entry.is_file() {
            assistant_skills_count += 1;
            assistant_skills_size = assistant_skills_size.saturating_add(size);
        } else if name.starts_with("workspace_skills/") && entry.is_file() {
            workspace_skills_count += 1;
            workspace_skills_size = workspace_skills_size.saturating_add(size);
        }
    }

    if teamwork_count > 0 {
        sections.push(SectionPreview {
            name: "teamwork".to_string(),
            item_count: teamwork_count,
            size_bytes: teamwork_size,
        });
    }
    if assistant_skills_count > 0 {
        sections.push(SectionPreview {
            name: "assistant_skills".to_string(),
            item_count: assistant_skills_count,
            size_bytes: assistant_skills_size,
        });
    }
    if workspace_skills_count > 0 {
        sections.push(SectionPreview {
            name: "workspace_skills".to_string(),
            item_count: workspace_skills_count,
            size_bytes: workspace_skills_size,
        });
    }

    Ok(OrgBundlePreview {
        format_version,
        app_version,
        exported_at,
        compatibility: check_compatibility(format_version),
        org_id,
        org_name,
        source_root_session_id,
        assistant_ids,
        mcp_ids,
        sections,
        total_size_bytes,
        file_path,
    })
}

#[command]
pub async fn import_org_bundle(
    window: tauri::Window,
    file_path: String,
    conflict_strategy: String,
    password: Option<String>,
) -> Result<OrgBundleImportResult, String> {
    emit_progress(&window, 5);

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
    emit_progress(&window, 15);

    let db = get_database_connection();
    let base_dir = get_session_manager()?.get_base_data_dir();
    let db_path = base_dir.join("libragent.db");
    let backup_manager = BackupManager::new(db_path);
    let backup_file = backup_manager
        .create_backup(db)
        .await
        .map_err(|e| e.to_string())?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys = OFF;".to_string(),
    ))
    .await
    .map_err(|e| e.to_string())?;

    let txn = db.begin().await.map_err(|e| e.to_string())?;
    emit_progress(&window, 25);

    let mut reports = HashMap::new();
    let mut total_imported = 0usize;
    let mut total_skipped = 0usize;
    let mut total_errors = 0usize;

    let report_mcp = import_mcp_data(&txn, mcp_data, &conflict_strategy).await?;
    total_imported += report_mcp.success;
    total_skipped += report_mcp.skipped;
    total_errors += report_mcp.errors.len();
    reports.insert("mcp_servers".to_string(), report_mcp);
    emit_progress(&window, 45);

    let report_assistants =
        import_assistants_data(&txn, assistants_data, &conflict_strategy).await?;
    total_imported += report_assistants.success;
    total_skipped += report_assistants.skipped;
    total_errors += report_assistants.errors.len();
    reports.insert("assistants".to_string(), report_assistants);
    emit_progress(&window, 60);

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
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_keys = ON;".to_string(),
        ))
        .await
        .ok();
        backup_manager.restore_from_backup(&backup_file).ok();
        return Err("Org 번들 가져오기 중 데이터베이스 오류가 발생해 원복했습니다.".to_string());
    }

    txn.commit().await.map_err(|e| e.to_string())?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys = ON;".to_string(),
    ))
    .await
    .ok();
    emit_progress(&window, 70);

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
    emit_progress(&window, 85);

    let mut skills_success = 0usize;
    let mut skills_skipped = 0usize;
    let mut skills_errors = Vec::new();

    for assistant_id in &manifest.assistant_ids {
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
    emit_progress(&window, 92);

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

    emit_progress(&window, 100);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_mcp_server_ids() {
        let ids = extract_mcp_server_ids_from_config(
            r#"{"mcpServerIds":["a","b"],"disabledSkills":["x"]}"#,
        );
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn sanitizes_org_id_path_segment() {
        assert_eq!(sanitize_org_id_for_path("org-abc_123"), "org-abc_123");
        assert_eq!(sanitize_org_id_for_path("org/../evil"), "org___evil");
        assert_eq!(sanitize_org_id_for_path(""), "unknown-org");
    }
}
