use crate::services::skill_service::SKILL_FILE_NAME;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Marker file written into every bundled skill directory in AppData.
/// Used to distinguish bundled skills from user-created ones so that skills
/// removed from the bundle can be cleaned up automatically on the next launch.
pub(crate) const BUNDLED_SKILL_MARKER: &str = ".bundled_skill";
pub(crate) const ASSISTANT_BUNDLED_SKILLS_MANIFEST_FILE_NAME: &str =
    ".bundled_skills_manifest.json";
const MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledSkillsManifest {
    pub schema_version: u32,
    pub skills: BTreeMap<String, String>,
}

pub(crate) fn collect_skill_directory_names(skills_dir: &Path) -> Result<BTreeSet<String>, String> {
    if !skills_dir.exists() {
        return Ok(BTreeSet::new());
    }

    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(skills_dir).map_err(|e| {
        log::warn!(
            "Failed to read skills directory {}: {}",
            skills_dir.display(),
            e
        );
        "Failed to read skills directory".to_string()
    })? {
        let entry = entry.map_err(|e| {
            log::warn!("Failed to read directory entry: {}", e);
            "Failed to read directory entry".to_string()
        })?;
        if !entry.path().is_dir() {
            continue;
        }

        names.insert(entry.file_name().to_string_lossy().to_string());
    }

    Ok(names)
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|error| {
        log::warn!("Failed to strip prefix for {}: {}", path.display(), error);
        "Failed to resolve relative path".to_string()
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

pub fn hash_skill_directory(skill_dir: &Path) -> Result<String, String> {
    let mut files = walkdir::WalkDir::new(skill_dir)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            log::warn!(
                "Failed to scan skill directory {}: {}",
                skill_dir.display(),
                error
            );
            "Failed to scan skill directory".to_string()
        })?
        .into_iter()
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.file_name().to_string_lossy().as_ref() != BUNDLED_SKILL_MARKER
        })
        .map(|entry| {
            let path = entry.into_path();
            let normalized = normalized_relative_path(skill_dir, &path)?;
            Ok((normalized, path))
        })
        .collect::<Result<Vec<_>, String>>()?;

    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative_path, full_path) in files {
        hasher.update(relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(std::fs::read(&full_path).map_err(|error| {
            log::warn!("Failed to read file {}: {}", full_path.display(), error);
            "Failed to read skill file content".to_string()
        })?);
        hasher.update([0]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn build_bundled_skills_manifest(
    skills_dir: &Path,
) -> Result<BundledSkillsManifest, String> {
    let mut manifest = BundledSkillsManifest {
        schema_version: MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION,
        skills: BTreeMap::new(),
    };

    if !skills_dir.exists() {
        return Ok(manifest);
    }

    let mut entries = std::fs::read_dir(skills_dir)
        .map_err(|error| {
            log::warn!(
                "Failed to read skills directory {}: {}",
                skills_dir.display(),
                error
            );
            "Failed to read skills directory".to_string()
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            log::warn!("Failed to read directory entry: {}", error);
            "Failed to read directory entry".to_string()
        })?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    for entry in entries {
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        if !skill_path.join(SKILL_FILE_NAME).is_file() {
            continue;
        }

        let skill_dir_name = entry.file_name().to_string_lossy().to_string();
        manifest
            .skills
            .insert(skill_dir_name, hash_skill_directory(&skill_path)?);
    }

    Ok(manifest)
}

pub(crate) fn build_marked_bundled_skills_manifest(
    skills_dir: &Path,
) -> Result<BundledSkillsManifest, String> {
    let mut manifest = BundledSkillsManifest {
        schema_version: MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION,
        skills: BTreeMap::new(),
    };

    if !skills_dir.exists() {
        return Ok(manifest);
    }

    let mut entries = std::fs::read_dir(skills_dir)
        .map_err(|error| {
            log::warn!(
                "Failed to read skills directory {}: {}",
                skills_dir.display(),
                error
            );
            "Failed to read skills directory".to_string()
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            log::warn!("Failed to read directory entry: {}", error);
            "Failed to read directory entry".to_string()
        })?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    for entry in entries {
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        if !skill_path.join(BUNDLED_SKILL_MARKER).is_file()
            || !skill_path.join(SKILL_FILE_NAME).is_file()
        {
            continue;
        }

        let skill_dir_name = entry.file_name().to_string_lossy().to_string();
        manifest
            .skills
            .insert(skill_dir_name, hash_skill_directory(&skill_path)?);
    }

    Ok(manifest)
}

pub fn load_persisted_bundled_skills_manifest(
    manifest_path: &Path,
) -> Result<Option<BundledSkillsManifest>, String> {
    if !manifest_path.exists() {
        return Ok(None);
    }

    let payload = std::fs::read(manifest_path).map_err(|error| {
        log::warn!(
            "Failed to read manifest {}: {}",
            manifest_path.display(),
            error
        );
        "Failed to read persisted skills manifest".to_string()
    })?;
    let manifest = match serde_json::from_slice::<BundledSkillsManifest>(&payload) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };

    if manifest.schema_version != MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION {
        return Ok(None);
    }

    Ok(Some(manifest))
}

fn remove_path_if_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_file() {
        std::fs::remove_file(path)
    } else {
        std::fs::remove_dir_all(path)
    }
}

fn best_effort_remove_path(path: &Path, context: &str) {
    if let Err(error) = remove_path_if_exists(path) {
        log::warn!(
            "Failed to remove {} at {}: {}",
            context,
            path.display(),
            error
        );
    }
}

fn best_effort_rename(from: &Path, to: &Path, context: &str) {
    if let Err(error) = std::fs::rename(from, to) {
        log::warn!(
            "Failed to {} from {} to {}: {}",
            context,
            from.display(),
            to.display(),
            error
        );
    }
}

/// Rolls back staged file replacement unless `commit()` is called after the target is updated.
struct StagedFileReplaceGuard {
    staging_path: PathBuf,
    target_path: PathBuf,
    backup_path: PathBuf,
    moved_target_to_backup: bool,
    committed: bool,
}

impl StagedFileReplaceGuard {
    fn new(staging_path: PathBuf, target_path: PathBuf, backup_path: PathBuf) -> Self {
        Self {
            staging_path,
            target_path,
            backup_path,
            moved_target_to_backup: false,
            committed: false,
        }
    }

    fn mark_target_moved_to_backup(&mut self) {
        self.moved_target_to_backup = true;
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for StagedFileReplaceGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        best_effort_remove_path(&self.staging_path, "staging manifest file");
        if self.moved_target_to_backup && !self.target_path.exists() {
            best_effort_rename(
                &self.backup_path,
                &self.target_path,
                "restore manifest from backup",
            );
        }
    }
}

/// Rolls back staged directory replacement unless `commit()` is called after the target is updated.
struct StagedDirReplaceGuard {
    staging_path: PathBuf,
    target_path: PathBuf,
    backup_path: PathBuf,
    moved_target_to_backup: bool,
    committed: bool,
}

impl StagedDirReplaceGuard {
    fn new(staging_path: PathBuf, target_path: PathBuf, backup_path: PathBuf) -> Self {
        Self {
            staging_path,
            target_path,
            backup_path,
            moved_target_to_backup: false,
            committed: false,
        }
    }

    fn mark_target_moved_to_backup(&mut self) {
        self.moved_target_to_backup = true;
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for StagedDirReplaceGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        best_effort_remove_path(&self.staging_path, "staging skill directory");
        if self.moved_target_to_backup && !self.target_path.exists() {
            best_effort_rename(
                &self.backup_path,
                &self.target_path,
                "restore skill directory from backup",
            );
        }
    }
}

pub fn write_manifest_atomically(
    manifest_path: &Path,
    manifest: &BundledSkillsManifest,
) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Failed to serialize manifest: {}", error))?;
    let temp_path = manifest_path.with_extension("json.tmp");
    let backup_path = manifest_path.with_extension("json.bak");

    remove_path_if_exists(&temp_path).map_err(|error| {
        log::warn!(
            "Failed to clear stale temp manifest {}: {}",
            temp_path.display(),
            error
        );
        "Failed to clear stale temporary manifest file".to_string()
    })?;

    std::fs::write(&temp_path, payload).map_err(|error| {
        log::warn!(
            "Failed to write temp manifest {}: {}",
            temp_path.display(),
            error
        );
        "Failed to write temporary manifest file".to_string()
    })?;

    let mut guard = StagedFileReplaceGuard::new(
        temp_path.clone(),
        manifest_path.to_path_buf(),
        backup_path.clone(),
    );
    let mut moved_existing_to_backup = false;

    if backup_path.exists() {
        std::fs::remove_file(&backup_path).map_err(|error| {
            log::warn!(
                "Failed to clear backup manifest {}: {}",
                backup_path.display(),
                error
            );
            "Failed to clear existing backup manifest".to_string()
        })?;
    }

    if manifest_path.exists() {
        std::fs::rename(manifest_path, &backup_path).map_err(|error| {
            log::warn!(
                "Failed to move existing manifest aside from {} to {}: {}",
                manifest_path.display(),
                backup_path.display(),
                error
            );
            "Failed to move existing manifest aside".to_string()
        })?;
        moved_existing_to_backup = true;
        guard.mark_target_moved_to_backup();
    }

    std::fs::rename(&temp_path, manifest_path).map_err(|error| {
        log::warn!(
            "Failed to finalize manifest from {} to {}: {}",
            temp_path.display(),
            manifest_path.display(),
            error
        );
        "Failed to finalize manifest file".to_string()
    })?;

    guard.commit();

    // Manifest is committed; leftover backup files are non-fatal.
    if moved_existing_to_backup {
        best_effort_remove_path(&backup_path, "backup manifest file");
    }

    Ok(())
}

pub fn replace_skill_directory_atomically(
    source_dir: &Path,
    target_dir: &Path,
) -> Result<(), String> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| "Invalid managed skill path".to_string())?;
    let skill_name = target_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid managed skill directory name".to_string())?;

    let temp_dir = parent.join(format!(".sync-tmp-{}", skill_name));
    let backup_dir = parent.join(format!(".sync-backup-{}", skill_name));

    remove_path_if_exists(&temp_dir).map_err(|error| {
        log::warn!("Failed to clear temp dir {}: {}", temp_dir.display(), error);
        "Failed to clear temporary sync directory".to_string()
    })?;
    remove_path_if_exists(&backup_dir).map_err(|error| {
        log::warn!(
            "Failed to clear backup dir {}: {}",
            backup_dir.display(),
            error
        );
        "Failed to clear existing backup directory".to_string()
    })?;

    copy_dir_recursive_path(source_dir, &temp_dir).map_err(|e| {
        log::warn!(
            "Failed to copy dir from {} to {}: {}",
            source_dir.display(),
            temp_dir.display(),
            e
        );
        "Failed to copy skill directory".to_string()
    })?;

    let mut guard = StagedDirReplaceGuard::new(
        temp_dir.clone(),
        target_dir.to_path_buf(),
        backup_dir.clone(),
    );
    let mut moved_existing_to_backup = false;

    if target_dir.exists() {
        std::fs::rename(target_dir, &backup_dir).map_err(|error| {
            log::warn!(
                "Failed to move existing managed skill aside from {} to {}: {}",
                target_dir.display(),
                backup_dir.display(),
                error
            );
            "Failed to move existing managed skill aside".to_string()
        })?;
        moved_existing_to_backup = true;
        guard.mark_target_moved_to_backup();
    }

    std::fs::rename(&temp_dir, target_dir).map_err(|error| {
        log::warn!(
            "Failed to activate managed skill from {} to {}: {}",
            temp_dir.display(),
            target_dir.display(),
            error
        );
        "Failed to activate managed skill".to_string()
    })?;

    guard.commit();

    let marker_path = target_dir.join(BUNDLED_SKILL_MARKER);
    std::fs::write(&marker_path, b"bundled\n").map_err(|error| {
        log::warn!(
            "Failed to write bundled marker at {}: {}",
            marker_path.display(),
            error
        );
        "Failed to write bundled skill marker".to_string()
    })?;

    // Skill directory is committed; leftover backup directories are non-fatal.
    if moved_existing_to_backup {
        best_effort_remove_path(&backup_dir, "backup skill directory");
    }

    Ok(())
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    copy_dir_recursive_path(src, dst)
}

fn copy_dir_recursive_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
