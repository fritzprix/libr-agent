use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::mcp::builtin::utils::SecurityValidator;

/// Zip every non-symlink entry under `source_dir` into `writer` using
/// `zip_prefix/<relative-path>` entry names.
pub(crate) fn zip_directory_under_prefix<W: Write + std::io::Seek>(
    source_dir: &Path,
    zip_prefix: &str,
    writer: &mut zip::ZipWriter<W>,
    options: zip::write::FileOptions,
    mut on_progress: Option<&mut dyn FnMut(u32)>,
) -> Result<usize, String> {
    if !source_dir.exists() {
        return Ok(0);
    }

    let walk: Vec<_> = WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();
    let total_files = walk.len().max(1);
    let mut buffer = Vec::new();
    let mut written = 0usize;
    let prefix = zip_prefix.trim_matches('/');

    for (i, entry) in walk.into_iter().enumerate() {
        let path = entry.path();
        let metadata = path.symlink_metadata().map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }

        let name = path.strip_prefix(source_dir).map_err(|e| e.to_string())?;
        let name_str = name.to_string_lossy().replace('\\', "/");
        if name_str.is_empty() {
            continue;
        }

        let zip_path = format!("{prefix}/{name_str}");

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
            written += 1;
        }

        if let Some(cb) = on_progress.as_mut() {
            let pct = ((i + 1) * 100 / total_files) as u32;
            cb(pct.min(100));
        }
    }

    Ok(written)
}

/// Extract zip entries whose names start with `zip_prefix/` into `target_dir`,
/// with ZIP-slip protection scoped to `target_dir`.
pub(crate) fn extract_zip_prefix(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    zip_prefix: &str,
    target_dir: &Path,
    conflict_strategy: &str,
) -> Result<(usize, usize, Vec<String>), String> {
    let mut success = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    let security_validator = SecurityValidator::new_scoped_with_base_dir(target_dir.to_path_buf());
    let prefix = format!("{}/", zip_prefix.trim_matches('/'));
    let file_names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();

    for name in file_names {
        if !name.starts_with(&prefix) {
            continue;
        }
        let mut entry = archive.by_name(&name).map_err(|e| e.to_string())?;
        if !entry.is_file() {
            continue;
        }

        let cleaned_name = name.strip_prefix(&prefix).unwrap_or(&name);
        let outpath = match security_validator.validate_path_for_write(cleaned_name) {
            Ok(path) => path,
            Err(e) => {
                errors.push(format!("ZIP Slip blocked for '{}': {}", name, e));
                continue;
            }
        };

        if outpath.exists() && (conflict_strategy == "skip" || conflict_strategy == "merge") {
            skipped += 1;
            continue;
        }

        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent).ok();
        }

        match File::create(&outpath) {
            Ok(mut outfile) => {
                if let Err(e) = std::io::copy(&mut entry, &mut outfile) {
                    errors.push(format!("Failed to extract '{}': {}", name, e));
                } else {
                    success += 1;
                }
            }
            Err(e) => errors.push(format!("Failed to create '{}': {}", outpath.display(), e)),
        }
    }

    Ok((success, skipped, errors))
}

/// True when the archive contains at least one file entry under `zip_prefix/`.
pub(crate) fn archive_has_file_prefix(
    archive: &zip::ZipArchive<std::io::Cursor<&[u8]>>,
    zip_prefix: &str,
) -> bool {
    let prefix = format!("{}/", zip_prefix.trim_matches('/'));
    archive.file_names().any(|name| {
        name.starts_with(&prefix)
            && !name.ends_with('/')
            && name.len() > prefix.len()
    })
}

/// Resolve a writable export path under Downloads or an existing parent directory.
pub(crate) fn validate_export_output_path(output_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(output_path);
    let download_dir = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut allowed = false;

    if path.starts_with(&download_dir) {
        allowed = true;
    } else if let Some(parent) = path.parent() {
        if parent.exists() {
            allowed = true;
        }
    }

    if !allowed {
        return Err("선택한 경로에 쓸 권한이 없거나 유효하지 않은 경로입니다.".to_string());
    }

    Ok(path.to_path_buf())
}
