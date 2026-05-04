use super::contracts::SkillMetadata;
use super::directories::{get_system_skills_directory, get_user_skills_directory};
use super::scanning::scan_skills_internal;
use crate::state::{get_skills_catalog_revision, invalidate_skills_catalog};
use log::warn;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SkillScanCacheKey {
    root: String,
    source: Option<String>,
    origin: Option<String>,
    revision: u64,
}

#[derive(Default)]
struct SkillScanCache {
    entries: RwLock<HashMap<SkillScanCacheKey, Vec<SkillMetadata>>>,
}

fn skill_scan_cache() -> &'static SkillScanCache {
    static SKILL_SCAN_CACHE: OnceLock<SkillScanCache> = OnceLock::new();
    SKILL_SCAN_CACHE.get_or_init(SkillScanCache::default)
}

fn build_cache_key(
    root_path: &Path,
    source_tag: Option<&str>,
    origin_tag: Option<&str>,
) -> SkillScanCacheKey {
    SkillScanCacheKey {
        root: root_path.to_string_lossy().to_string(),
        source: source_tag.map(str::to_string),
        origin: origin_tag.map(str::to_string),
        revision: get_skills_catalog_revision(),
    }
}

pub fn invalidate_skill_scan_cache() -> u64 {
    match skill_scan_cache().entries.write() {
        Ok(mut entries) => entries.clear(),
        Err(error) => warn!("Failed to clear skill scan cache after mutation: {}", error),
    }

    invalidate_skills_catalog()
}

pub async fn scan_skills_internal_cached(
    root_path: &Path,
    source_tag: Option<String>,
    origin_tag: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    let key = build_cache_key(root_path, source_tag.as_deref(), origin_tag.as_deref());

    match skill_scan_cache().entries.read() {
        Ok(entries) => {
            if let Some(cached) = entries.get(&key) {
                return Ok(cached.clone());
            }
        }
        Err(error) => warn!(
            "Failed to read skill scan cache; falling back to rescan: {}",
            error
        ),
    }

    let scanned = scan_skills_internal(root_path, source_tag, origin_tag).await?;
    let current_revision = get_skills_catalog_revision();

    if key.revision != current_revision {
        return Ok(scanned);
    }

    match skill_scan_cache().entries.write() {
        Ok(mut entries) => {
            entries.insert(key, scanned.clone());
        }
        Err(error) => warn!(
            "Failed to update skill scan cache; returning uncached scan result: {}",
            error
        ),
    }

    Ok(scanned)
}

pub async fn prewarm_managed_skill_scans() -> Result<(), String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;

    let _ = scan_skills_internal_cached(
        &system_dir,
        Some("global".to_string()),
        Some("system".to_string()),
    )
    .await?;
    let _ = scan_skills_internal_cached(
        &user_dir,
        Some("global".to_string()),
        Some("user".to_string()),
    )
    .await?;

    Ok(())
}
