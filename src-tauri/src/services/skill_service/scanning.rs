use super::contracts::DiscoveredSkillRoot;
use super::contracts::{SkillFrontmatter, SkillMetadata, PLUGIN_SKILL_OWNER_FILE, SKILL_FILE_NAME};
use super::directories::{collect_allowed_skill_roots, resolve_skill_directories};
use log::{info, warn};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{self};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Read the plugin-owner marker (#2927 D2) of a skill directory itself.
/// `None` when the skill is not plugin-owned (no marker, or an unreadable
/// one). Single implementation behind `plugin_owner_of_skill_dir` (#2954
/// S1.2) — both the scanner and the importer must agree on what a marker is.
pub fn plugin_owner_marker_of_dir(skill_dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(skill_dir.join(PLUGIN_SKILL_OWNER_FILE)).ok()?;
    let owner = raw.trim().to_string();
    (!owner.is_empty()).then_some(owner)
}

/// Read the plugin-owner marker (#2927 D2) of the skill directory that
/// contains `skill_md_path`. `None` when the skill is not plugin-owned or the
/// marker is unreadable — an owner file that cannot be read must not make a
/// working skill invisible.
pub fn plugin_owner_of_skill_dir(skill_md_path: &Path) -> Option<String> {
    plugin_owner_marker_of_dir(skill_md_path.parent()?)
}

pub async fn scan_skills_directory(directory: &Path) -> Result<Vec<SkillMetadata>, String> {
    scan_skills_internal(directory, None, None).await
}

pub(crate) async fn scan_skills_internal(
    root_path: &Path,
    source_tag: Option<String>,
    origin_tag: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    if !root_path.exists() {
        info!("Skills directory does not exist: {:?}", root_path);
        return Ok(Vec::new());
    }

    let root_path_owned = root_path.to_owned();
    let source_tag_owned = source_tag.clone();
    let origin_tag_owned = origin_tag.clone();

    tokio::task::spawn_blocking(move || {
        let mut skills = Vec::new();

        for entry in WalkDir::new(&root_path_owned)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if entry.file_name() == SKILL_FILE_NAME {
                let path = entry.path();
                match parse_skill_metadata(path) {
                    Ok(mut metadata) => {
                        metadata.source = source_tag_owned.clone();
                        metadata.origin = origin_tag_owned.clone();
                        metadata.plugin_owner = plugin_owner_of_skill_dir(path);
                        skills.push(metadata);
                    }
                    Err(error) => {
                        warn!("Failed to parse skill at {:?}: {}", path, error);
                    }
                }
            }
        }
        Ok(skills)
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
}

pub fn parse_skill_metadata(path: &Path) -> Result<SkillMetadata, String> {
    let content = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            "Content appears to be binary or contains invalid UTF-8 characters".to_string()
        } else {
            error.to_string()
        }
    })?;
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);

    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_idx) = stripped.find("---") {
            let frontmatter_str = &stripped[..end_idx];
            let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
                .map_err(|error| format!("YAML parse error: {}", error))?;

            if frontmatter.name.trim().is_empty() {
                return Err("Skill name cannot be empty".to_string());
            }
            if frontmatter.description.trim().is_empty() {
                return Err("Skill description cannot be empty".to_string());
            }

            return Ok(SkillMetadata {
                name: frontmatter.name,
                description: frontmatter.description,
                path: path.to_string_lossy().to_string(),
                alias_path: None,
                source: None,
                origin: None,
                plugin_owner: None,
            });
        }
    }

    Err("No valid YAML frontmatter found".to_string())
}

pub async fn get_skill_content(
    skill_path: String,
    assistant_id: Option<String>,
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<String, String> {
    let (system_dir, user_dir, assistant_dir, workspace_dir) = resolve_skill_directories(
        assistant_id.as_deref(),
        session_id.as_deref(),
        workspace_path.as_deref().map(Path::new),
    )
    .await?;
    let allowed_roots =
        collect_allowed_skill_roots(system_dir, user_dir, assistant_dir, workspace_dir);
    get_skill_content_from_roots(skill_path, &allowed_roots).await
}

pub async fn get_skill_content_from_roots(
    skill_path: String,
    allowed_roots: &[PathBuf],
) -> Result<String, String> {
    let path = PathBuf::from(&skill_path);

    if path.file_name() != Some(OsStr::new(SKILL_FILE_NAME)) {
        return Err("Skill path must point to a SKILL.md file".to_string());
    }

    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("Invalid skill path: {}", error))?;

    let mut is_allowed = false;
    for root in allowed_roots {
        if !root.exists() {
            continue;
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("Invalid skills directory: {}", error))?;
        if canonical_path.starts_with(&canonical_root) {
            is_allowed = true;
            break;
        }
    }

    if !is_allowed {
        return Err("Skill path is outside the allowed skills directories".to_string());
    }

    tokio::task::spawn_blocking(move || {
        fs::read_to_string(&canonical_path)
            .map_err(|error| format!("Failed to read skill content: {}", error))
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|error| error.to_string())?;
    }

    for entry in fs::read_dir(src).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub(super) fn discover_existing_skill_roots(
    root_path: &Path,
) -> Result<HashMap<String, PathBuf>, String> {
    if !root_path.exists() {
        return Ok(HashMap::new());
    }

    let mut roots = HashMap::new();
    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_name() != SKILL_FILE_NAME {
            continue;
        }

        let path = entry.path();
        match parse_skill_metadata(path) {
            Ok(metadata) => {
                if let Some(parent) = path.parent() {
                    roots.insert(metadata.name.to_lowercase(), parent.to_path_buf());
                }
            }
            Err(error) => {
                warn!(
                    "Failed to parse existing skill while scanning {}: {}",
                    path.display(),
                    error
                );
            }
        }
    }

    Ok(roots)
}

pub(super) fn find_skill_root_by_name(
    root_path: &Path,
    skill_name: &str,
) -> Result<Option<PathBuf>, String> {
    let existing_roots = discover_existing_skill_roots(root_path)?;
    Ok(existing_roots.get(&skill_name.to_lowercase()).cloned())
}

pub(super) fn remove_skill_name_from_directory(
    root_path: &Path,
    skill_name: &str,
) -> Result<(), String> {
    let Some(target_root) = find_skill_root_by_name(root_path, skill_name)? else {
        return Err(format!("Skill '{}' not found", skill_name));
    };

    fs::remove_dir_all(target_root).map_err(|error| error.to_string())
}

pub(super) fn discover_skill_roots(root_path: &Path) -> Result<Vec<DiscoveredSkillRoot>, String> {
    let mut discovered = Vec::new();
    let mut seen_names = HashSet::new();

    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_name() != SKILL_FILE_NAME {
            continue;
        }

        let skill_path = entry.path();
        let metadata = parse_skill_metadata(skill_path)?;
        let normalized_name = metadata.name.to_lowercase();
        if !seen_names.insert(normalized_name) {
            return Err(format!(
                "Duplicate skill name '{}' found in the import source",
                metadata.name
            ));
        }

        let root_dir = skill_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            format!(
                "Failed to determine skill root for {}",
                skill_path.display()
            )
        })?;

        discovered.push(DiscoveredSkillRoot { metadata, root_dir });
    }

    if discovered.is_empty() {
        return Err("No skills (SKILL.md) found in the imported files".to_string());
    }

    discovered.sort_by_cached_key(|skill| skill.metadata.name.to_lowercase());
    Ok(discovered)
}

pub fn skill_storage_directory_name(skill_name: &str) -> Result<String, String> {
    let trimmed = skill_name.trim();
    if trimmed.is_empty() {
        return Err("Skill name cannot be empty".to_string());
    }
    if trimmed == "." || trimmed == ".." {
        return Err(format!(
            "Skill name '{}' cannot be used as a storage directory",
            skill_name
        ));
    }

    let mut slug = String::new();
    let mut previous_was_separator = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if !previous_was_separator && !slug.is_empty() {
            slug.push('-');
            previous_was_separator = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    let stem = if slug.is_empty() {
        "skill"
    } else {
        slug.as_str()
    };
    let digest = Sha256::digest(trimmed.as_bytes());
    let digest_hex = format!("{:x}", digest);
    Ok(format!("{}-{}", stem, &digest_hex[..12]))
}
