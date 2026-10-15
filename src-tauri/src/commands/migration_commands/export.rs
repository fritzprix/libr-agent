use sea_orm::EntityTrait;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tauri::{command, Emitter};

use crate::entity::{assistant, mcp_server, playbook, scheduled_task, settings};
use crate::services::skill_service::get_user_skills_directory;
use crate::state::get_database_connection;

use super::crypto::encrypt_data;
use super::models::{
    MigrationExportInfo, PlaybookRecord, ScheduledTaskRecord, SettingsRecord, CURRENT_FORMAT_VERSION,
};
use super::sanitize::{clean_assistants, clean_mcp_servers, mask_sensitive_json};
use super::zip_fs::{validate_export_output_path, zip_directory_under_prefix};

fn clean_settings(records: &[settings::Model], include_sensitive: bool) -> Vec<SettingsRecord> {
    let mut cleaned = Vec::new();
    for r in records {
        let mut r = r.clone();
        if !include_sensitive {
            let key_lower = r.key.to_lowercase();
            if key_lower.contains("key")
                || key_lower.contains("token")
                || key_lower.contains("password")
                || key_lower.contains("secret")
                || key_lower.contains("credential")
            {
                r.value = "null".to_string();
            } else if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&r.value) {
                mask_sensitive_json(&mut val);
                r.value = serde_json::to_string(&val).unwrap_or(r.value);
            }
        }
        cleaned.push(SettingsRecord {
            key: r.key,
            value: r.value,
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }
    cleaned
}

#[command]
pub async fn export_migration(
    window: tauri::Window,
    output_path: String,
    include_sensitive_data: bool,
    password: Option<String>,
) -> Result<MigrationExportInfo, String> {
    if include_sensitive_data && password.is_none() {
        return Err("민감 데이터를 포함할 경우 백업 암호 설정이 필수입니다.".to_string());
    }

    window.emit("migration:progress", 5).ok();
    let db = get_database_connection();

    validate_export_output_path(&output_path)?;

    // Fetch data
    let db_settings = settings::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 12).ok();

    let db_assistants = assistant::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 19).ok();

    let db_mcp = mcp_server::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 26).ok();

    let db_playbooks = playbook::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 33).ok();

    let db_tasks = scheduled_task::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 40).ok();

    // Mask / clean
    let cleaned_settings = clean_settings(&db_settings, include_sensitive_data);
    let cleaned_mcp = clean_mcp_servers(&db_mcp, include_sensitive_data);
    let cleaned_assistants = clean_assistants(&db_assistants, include_sensitive_data);

    let playbooks_records: Vec<PlaybookRecord> = db_playbooks
        .into_iter()
        .map(|p| PlaybookRecord {
            id: p.id,
            assistant_id: p.assistant_id,
            goal: p.goal,
            initial_command: p.initial_command,
            workflow: p.workflow,
            success_criteria: p.success_criteria,
            default_target_session: p.default_target_session,
            created_at: p.created_at,
            updated_at: p.updated_at,
            is_bookmarked: p.is_bookmarked,
        })
        .collect();

    let task_records: Vec<ScheduledTaskRecord> = db_tasks
        .into_iter()
        .map(|t| ScheduledTaskRecord {
            id: t.id,
            name: t.name,
            task_category: t.task_category,
            cron_expression: t.cron_expression,
            schedule_timezone: t.schedule_timezone,
            assistant_id: t.assistant_id,
            message: t.message,
            execution_mode: t.execution_mode,
            yolo_mode: None,
            unsafe_mode: None,
            created_by_session_id: t.created_by_session_id,
            session_id: t.session_id,
            workspace_override: t.workspace_override,
            reset_planning_state: t.reset_planning_state,
            enabled: t.enabled,
            last_run_at: t.last_run_at,
            next_run_at: t.next_run_at,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();

    window.emit("migration:progress", 45).ok();

    // Determine target zip file path (temporary if encrypting)
    let temp_zip_path = if include_sensitive_data {
        let temp_dir = std::env::temp_dir();
        let uuid = uuid::Uuid::new_v4().to_string();
        temp_dir.join(format!("libragent-export-{}.zip", uuid))
    } else {
        PathBuf::from(&output_path)
    };

    // Create zip
    let file = File::create(&temp_zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Write manifest
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": chrono::Utc::now().to_rfc3339(),
    });
    zip.start_file("manifest.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;

    // Write tables
    zip.start_file("settings.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&cleaned_settings)
            .unwrap()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 50).ok();

    zip.start_file("assistants.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&cleaned_assistants)
            .unwrap()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 55).ok();

    zip.start_file("mcp_servers.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&cleaned_mcp)
            .unwrap()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 60).ok();

    zip.start_file("playbooks.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&playbooks_records)
            .unwrap()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 65).ok();

    zip.start_file("scheduled_tasks.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&task_records)
            .unwrap()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    window.emit("migration:progress", 70).ok();

    // Zip user skills
    let mut zipped_skills = false;
    if let Ok(skills_dir) = get_user_skills_directory() {
        if skills_dir.exists() {
            let window_ref = window.clone();
            let mut progress_cb = move |pct: u32| {
                let progress = 70 + (pct * 25 / 100);
                window_ref.emit("migration:progress", progress).ok();
            };
            zip_directory_under_prefix(
                &skills_dir,
                "user_skills",
                &mut zip,
                options,
                Some(&mut progress_cb),
            )?;
            zipped_skills = true;
        }
    }

    if !zipped_skills {
        window.emit("migration:progress", 95).ok();
    }

    zip.finish().map_err(|e| e.to_string())?;

    let final_file_size = if include_sensitive_data {
        // Read unencrypted temp zip bytes
        let zip_bytes = fs::read(&temp_zip_path).map_err(|e| e.to_string())?;

        // Encrypt ZIP bytes using password
        let pwd = password.as_deref().unwrap_or("");
        let encrypted_bytes = encrypt_data(&zip_bytes, pwd)?;

        // Write final encrypted file
        fs::write(&output_path, &encrypted_bytes).map_err(|e| e.to_string())?;

        // Clean up temp ZIP file
        fs::remove_file(&temp_zip_path).ok();

        encrypted_bytes.len() as u64
    } else {
        let file_metadata = fs::metadata(&output_path).map_err(|e| e.to_string())?;
        file_metadata.len()
    };

    window.emit("migration:progress", 100).ok();

    Ok(MigrationExportInfo {
        file_path: output_path,
        file_size_bytes: final_file_size,
        sections: vec![
            "settings".to_string(),
            "assistants".to_string(),
            "mcp_servers".to_string(),
            "playbooks".to_string(),
            "scheduled_tasks".to_string(),
            "user_skills".to_string(),
        ],
    })
}
