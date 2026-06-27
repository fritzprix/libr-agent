use std::fs;
use std::io::Read;
use tauri::command;

use super::crypto::decrypt_data;
use super::models::{
    CompatibilityStatus, MigrationPreview, SectionPreview, CURRENT_FORMAT_VERSION,
    MAX_SINGLE_JSON_BYTES, MAX_TOTAL_DECOMPRESSED_BYTES, MIN_COMPATIBLE_VERSION,
};

pub fn check_compatibility(manifest_version: u32) -> CompatibilityStatus {
    if manifest_version < MIN_COMPATIBLE_VERSION {
        CompatibilityStatus::Incompatible {
            message: format!(
                "지원하지 않는 포맷 v{}. 최소 필요 버전: v{}",
                manifest_version, MIN_COMPATIBLE_VERSION
            ),
        }
    } else if manifest_version > CURRENT_FORMAT_VERSION {
        CompatibilityStatus::NewerVersion {
            message: format!(
                "이 파일은 최신 버전의 LibrAgent 포맷 v{}으로 백업되었습니다. 일부 가져오기 기능이 제한될 수 있습니다.",
                manifest_version
            ),
        }
    } else {
        CompatibilityStatus::Compatible
    }
}

#[command]
pub async fn inspect_migration(
    file_path: String,
    password: Option<String>,
) -> Result<MigrationPreview, String> {
    let file_bytes = fs::read(&file_path).map_err(|e| format!("파일을 읽을 수 없습니다: {}", e))?;

    let zip_bytes = match decrypt_data(&file_bytes, password.as_deref())? {
        Some(decrypted) => decrypted,
        None => file_bytes,
    };

    let cursor = std::io::Cursor::new(&zip_bytes[..]);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("올바른 ZIP 파일이 아닙니다: {}", e))?;

    let mut format_version = 1;
    let mut app_version = None;
    let mut exported_at = None;
    let mut sections = Vec::new();
    let mut total_size_bytes = 0;

    // 1. Read manifest.json in memory
    if let Ok(mut manifest_entry) = archive.by_name("manifest.json") {
        let mut content = String::new();
        if manifest_entry.read_to_string(&mut content).is_ok() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                format_version = v
                    .get("format_version")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(1) as u32;
                app_version = v
                    .get("app_version")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                exported_at = v
                    .get("exported_at")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    // 2. Scan entries in memory to count sizes and items without writing to disk
    let file_names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    let mut skills_count = 0;
    let mut skills_size = 0;

    for name in file_names {
        if name == "manifest.json" {
            continue;
        }

        let mut entry = archive.by_name(&name).map_err(|e| e.to_string())?;
        let entry_size = entry.size();
        total_size_bytes += entry_size;

        if total_size_bytes > MAX_TOTAL_DECOMPRESSED_BYTES {
            return Err(
                "마이그레이션 파일의 압축 해제 용량이 250MB 제한을 초과했습니다.".to_string(),
            );
        }

        if name.starts_with("user_skills/") {
            if entry.is_file() {
                skills_count += 1;
                skills_size += entry_size;
            }
            continue;
        }

        if entry.is_file() {
            if entry_size > MAX_SINGLE_JSON_BYTES {
                return Err(format!(
                    "마이그레이션 파일 내 특정 설정 파일의 용량이 너무 큽니다. (최대 20MB): {}",
                    name
                ));
            }

            let mut content = String::new();
            if entry.read_to_string(&mut content).is_ok() {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let item_count = json_val.as_array().map(|a| a.len()).unwrap_or(0);
                    let sec_name = name.trim_end_matches(".json").to_string();
                    sections.push(SectionPreview {
                        name: sec_name,
                        item_count,
                        size_bytes: entry_size,
                    });
                }
            }
        }
    }
    if skills_count > 0 {
        sections.push(SectionPreview {
            name: "user_skills".to_string(),
            item_count: skills_count,
            size_bytes: skills_size,
        });
    }

    let compatibility = check_compatibility(format_version);

    Ok(MigrationPreview {
        format_version,
        app_version,
        exported_at,
        compatibility,
        sections,
        total_size_bytes,
        file_path,
    })
}
