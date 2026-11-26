//! `inspect_org_bundle`: read a bundle's manifest and section previews
//! without importing anything.

use std::fs;
use std::io::Read;
use tauri::command;

use crate::commands::migration_commands::crypto::decrypt_data;
use crate::commands::migration_commands::inspect::check_compatibility;
use crate::commands::migration_commands::models::{
    OrgBundlePreview, SectionPreview, ORG_BUNDLE_KIND,
};

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
