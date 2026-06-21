use crate::entity::{assistant, mcp_server, playbook, scheduled_task, settings};
use crate::lifecycle::database_backup::BackupManager;
use crate::mcp::builtin::utils::SecurityValidator;
use crate::services::skill_service::get_user_skills_directory;
use crate::state::get_database_connection;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DbBackend, EntityTrait, Set, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{command, Emitter};
use walkdir::WalkDir;

use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

const ENC_MAGIC: &[u8] = b"LIBRAGENT_ENC_V1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const PBKDF2_ITERATIONS: u32 = 100_000;

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

fn encrypt_data(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let key_bytes = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("암호화 키 초기화 실패: {}", e))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("데이터 암호화 실패: {}", e))?;

    let mut result = Vec::with_capacity(ENC_MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    result.extend_from_slice(ENC_MAGIC);
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

fn decrypt_data(data: &[u8], password: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    if data.len() < ENC_MAGIC.len() {
        return Ok(None);
    }

    if &data[0..ENC_MAGIC.len()] != ENC_MAGIC {
        return Ok(None); // Plain ZIP
    }

    let pwd = password.ok_or_else(|| "PASSWORD_REQUIRED".to_string())?;

    if data.len() < ENC_MAGIC.len() + SALT_LEN + NONCE_LEN {
        return Err("암호화된 백업 파일의 헤더가 손상되었습니다.".to_string());
    }

    let salt = &data[ENC_MAGIC.len()..(ENC_MAGIC.len() + SALT_LEN)];
    let nonce_bytes = &data[(ENC_MAGIC.len() + SALT_LEN)..(ENC_MAGIC.len() + SALT_LEN + NONCE_LEN)];
    let ciphertext = &data[(ENC_MAGIC.len() + SALT_LEN + NONCE_LEN)..];

    let key_bytes = derive_key(pwd, salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("복호화 키 초기화 실패: {}", e))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "WRONG_PASSWORD".to_string())?;

    Ok(Some(decrypted))
}

// --- DTO structs for serialization/deserialization to bypass non-deserializable Entity models ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssistantRecord {
    pub id: String,
    pub name: String,
    pub config: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpServerRecord {
    pub id: String,
    pub name: String,
    pub config: String,
    pub tool_count: Option<i32>,
    pub cached_tools: Option<String>,
    pub verification_status: Option<String>,
    pub last_verification_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaybookRecord {
    pub id: String,
    pub assistant_id: String,
    pub goal: String,
    pub initial_command: Option<String>,
    pub workflow: String,
    pub success_criteria: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScheduledTaskRecord {
    pub id: String,
    pub name: String,
    pub task_category: String,
    pub cron_expression: Option<String>,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    pub message: String,
    pub yolo_mode: bool,
    pub unsafe_mode: bool,
    pub created_by_session_id: Option<String>,
    pub session_id: Option<String>,
    pub workspace_override: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SettingsRecord {
    pub key: String,
    pub value: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// --- Tauri Command Return Structs ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationExportInfo {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub sections: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationSectionReport {
    pub success: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationImportResult {
    pub sections_imported: HashMap<String, MigrationSectionReport>,
    pub total_imported: usize,
    pub total_skipped: usize,
    pub total_errors: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SectionPreview {
    pub name: String,
    pub item_count: usize,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CompatibilityStatus {
    Compatible,
    NewerVersion { message: String },
    Incompatible { message: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationPreview {
    pub format_version: u32,
    pub app_version: Option<String>,
    pub exported_at: Option<String>,
    pub compatibility: CompatibilityStatus,
    pub sections: Vec<SectionPreview>,
    pub total_size_bytes: u64,
    pub file_path: String,
}

// --- Constant Definitions ---

const CURRENT_FORMAT_VERSION: u32 = 1;
const MIN_COMPATIBLE_VERSION: u32 = 1;
const MAX_SINGLE_JSON_BYTES: u64 = 20 * 1024 * 1024; // 20 MB
const MAX_TOTAL_DECOMPRESSED_BYTES: u64 = 250 * 1024 * 1024; // 250 MB

// --- Helper functions for sensitive data masking ---

fn mask_sensitive_json(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let k_lower = k.to_lowercase();
                if k_lower.contains("key")
                    || k_lower.contains("token")
                    || k_lower.contains("password")
                    || k_lower.contains("secret")
                    || k_lower.contains("auth")
                    || k_lower.contains("credential")
                {
                    if let serde_json::Value::String(s) = v {
                        *s = "REDACTED".to_string();
                    } else {
                        *v = serde_json::Value::Null;
                    }
                } else {
                    mask_sensitive_json(v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                mask_sensitive_json(item);
            }
        }
        _ => {}
    }
}

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

fn clean_mcp_servers(
    records: &[mcp_server::Model],
    include_sensitive: bool,
) -> Vec<McpServerRecord> {
    let mut cleaned = Vec::new();
    for r in records {
        let mut config_str = r.config.clone();
        if !include_sensitive {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&r.config) {
                mask_sensitive_json(&mut val);
                config_str = serde_json::to_string(&val).unwrap_or(r.config.clone());
            }
        }
        cleaned.push(McpServerRecord {
            id: r.id.clone(),
            name: r.name.clone(),
            config: config_str,
            tool_count: r.tool_count,
            cached_tools: r.cached_tools.clone(),
            verification_status: r.verification_status.clone(),
            last_verification_error: r.last_verification_error.clone(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }
    cleaned
}

fn clean_assistants(records: &[assistant::Model], include_sensitive: bool) -> Vec<AssistantRecord> {
    let mut cleaned = Vec::new();
    for r in records {
        let mut config_str = r.config.clone();
        if !include_sensitive {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&r.config) {
                mask_sensitive_json(&mut val);
                config_str = serde_json::to_string(&val).unwrap_or(r.config.clone());
            }
        }
        cleaned.push(AssistantRecord {
            id: r.id.clone(),
            name: r.name.clone(),
            config: config_str,
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }
    cleaned
}

// --- ZIP Directory Helper ---

fn zip_dir<W: Write + std::io::Seek>(
    prefix: &Path,
    writer: &mut zip::ZipWriter<W>,
    options: zip::write::FileOptions,
    window: &tauri::Window,
    base_progress: u32,
    progress_span: u32,
) -> Result<(), String> {
    let walk: Vec<_> = WalkDir::new(prefix)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();
    let total_files = walk.len();
    let mut buffer = Vec::new();

    for (i, entry) in walk.into_iter().enumerate() {
        let path = entry.path();
        let metadata = path.symlink_metadata().map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            continue; // Skip symlinks for security
        }

        let name = path.strip_prefix(prefix).map_err(|e| e.to_string())?;
        let name_str = name.to_string_lossy().to_string();

        if name_str.is_empty() {
            continue;
        }

        let zip_path = format!("user_skills/{}", name_str.replace('\\', "/"));

        if path.is_dir() {
            writer
                .add_directory(&zip_path, options)
                .map_err(|e| e.to_string())?;
        } else {
            writer
                .start_file(&zip_path, options)
                .map_err(|e| e.to_string())?;
            let mut f = File::open(path).map_err(|e| e.to_string())?;
            f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            writer.write_all(&buffer).map_err(|e| e.to_string())?;
            buffer.clear();
        }

        if total_files > 0 {
            let progress = base_progress + ((i + 1) as u32 * progress_span / total_files as u32);
            window.emit("migration:progress", progress).ok();
        }
    }
    Ok(())
}

fn check_compatibility(manifest_version: u32) -> CompatibilityStatus {
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

// --- Tauri Commands ---

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

    // Verify output directory eligibility
    let path = Path::new(&output_path);
    let download_dir = dirs::download_dir().unwrap_or(PathBuf::from("."));
    let mut allowed = false;

    if path.starts_with(&download_dir) {
        allowed = true;
    } else if let Some(parent) = path.parent() {
        if parent.exists() {
            allowed = true; // Best effort checks
        }
    }

    if !allowed {
        return Err("선택한 경로에 쓸 권한이 없거나 유효하지 않은 경로입니다.".to_string());
    }

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
            group_id: t.group_id,
            group_name: t.group_name,
            message: t.message,
            yolo_mode: t.yolo_mode,
            unsafe_mode: t.unsafe_mode,
            created_by_session_id: t.created_by_session_id,
            session_id: t.session_id,
            workspace_override: t.workspace_override,
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
            zip_dir(&skills_dir, &mut zip, options, &window, 70, 25)?;
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
    // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
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
    // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
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
    // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
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
    // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
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
                if total_files > 0 {
                    let progress = 90 + ((idx + 1) * 10 / total_files);
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
        if let Some(existing_model) = existing {
            let mut active: scheduled_task::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.task_category = Set(r.task_category);
            active.cron_expression = Set(r.cron_expression);
            active.schedule_timezone = Set(r.schedule_timezone);
            active.assistant_id = Set(r.assistant_id);
            active.group_id = Set(r.group_id);
            active.group_name = Set(r.group_name);
            active.message = Set(r.message);
            active.yolo_mode = Set(r.yolo_mode);
            active.unsafe_mode = Set(r.unsafe_mode);
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
                group_id: Set(r.group_id),
                group_name: Set(r.group_name),
                message: Set(r.message),
                yolo_mode: Set(r.yolo_mode),
                unsafe_mode: Set(r.unsafe_mode),
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

#[command]
pub async fn reverify_mcp_servers() -> Result<HashMap<String, String>, String> {
    let db = get_database_connection();
    let repo = crate::state::get_mcp_server_repository();

    // Fetch all MCP servers from the database
    let db_mcp = mcp_server::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;

    let mut results = HashMap::new();

    // Iterate and probe each server
    for server in db_mcp {
        let res =
            crate::services::mcp_server_service::McpServerService::probe_server(repo, &server.id)
                .await;
        match res {
            Ok(_) => {
                results.insert(server.id, "success".to_string());
            }
            Err(e) => {
                log::error!("Failed to reverify MCP server {}: {}", server.id, e);
                results.insert(server.id, "error".to_string());
            }
        }
    }

    Ok(results)
}
