use std::fs::{self, File};
use tauri::Emitter;

use crate::commands::migration_commands::models::MigrationSectionReport;
use crate::mcp::builtin::utils::SecurityValidator;
use crate::services::skill_service::get_user_skills_directory;

pub(super) fn restore_user_skills(
    window: &tauri::Window,
    zip_bytes: &[u8],
    conflict_strategy: &str,
) -> Result<MigrationSectionReport, String> {
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
        let cursor = std::io::Cursor::new(zip_bytes);
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

    Ok(MigrationSectionReport {
        success: skills_success,
        skipped: skills_skipped,
        errors: skills_errors,
    })
}
