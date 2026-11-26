//! `export_org_bundle`: collect the org's assistants, MCP servers, teamwork
//! artifacts and skill layers into one zip, optionally encrypted.

use sea_orm::EntityTrait;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tauri::command;

use crate::commands::migration_commands::crypto::encrypt_data;
use crate::commands::migration_commands::models::{
    OrgBundleExportInfo, OrgBundleManifest, CURRENT_FORMAT_VERSION, ORG_BUNDLE_KIND,
};
use crate::commands::migration_commands::sanitize::{clean_assistants, clean_mcp_servers};
use crate::commands::migration_commands::zip_fs::{
    validate_export_output_path, zip_directory_under_prefix,
};
use crate::entity::{assistant, mcp_server};
use crate::repositories::SessionRepository;
use crate::services::skill_service::{
    get_assistant_skills_directory, get_workspace_skills_directory_for_session,
};
use crate::session::{get_session_manager, teamwork_artifact_dir_for_session};
use crate::state::{get_database_connection, get_session_repository};

use super::{emit_progress, extract_mcp_server_ids_from_config};

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
