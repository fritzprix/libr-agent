use super::skills_manifest::{
    build_bundled_skills_manifest, build_marked_bundled_skills_manifest, hash_skill_directory,
    load_persisted_bundled_skills_manifest, replace_skill_directory_atomically,
    write_manifest_atomically, ASSISTANT_BUNDLED_SKILLS_MANIFEST_FILE_NAME, BUNDLED_SKILL_MARKER,
};
use crate::repositories::AssistantRepository;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

pub async fn sync_assistant_bundled_skills(
    resource_dir: &Path,
    base_data_dir: &Path,
) -> Result<(), String> {
    let assistants = crate::services::assistant_init::load_bundled_assistants(resource_dir)?;
    let bundled_assistants_dir = resource_dir.join("bundled_assistants");
    let assistant_names: Vec<String> = if assistants.is_empty() {
        crate::services::assistant_init::default_assistant_names()
            .iter()
            .map(|name| (*name).to_string())
            .collect()
    } else {
        assistants
            .into_iter()
            .map(|assistant| assistant.name)
            .collect()
    };

    let repo = crate::get_assistant_repository();
    let assistants_db = repo.list_assistants().await.map_err(|e| e.to_string())?;

    let assistant_map: HashMap<String, String> =
        assistants_db.into_iter().map(|a| (a.name, a.id)).collect();

    for assistant_name in assistant_names {
        let assistant_skills_dir = bundled_assistants_dir
            .join(&assistant_name)
            .join("bundled_skills");

        let Some(assistant_id) = assistant_map.get(&assistant_name) else {
            log::warn!(
                "⚠️ Skipping skill sync for '{}': assistant not found in database.",
                assistant_name
            );
            continue;
        };

        let target_skills_dir = base_data_dir
            .join("assistants")
            .join(assistant_id)
            .join("skills");
        sync_assistant_bundled_skills_snapshot(
            assistant_skills_dir.as_path(),
            &target_skills_dir,
            &assistant_name,
        )?;
    }

    Ok(())
}

fn sync_assistant_bundled_skills_snapshot(
    source_skills_dir: &Path,
    target_skills_dir: &Path,
    assistant_name: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(target_skills_dir).map_err(|error| {
        format!(
            "Failed to create assistant skills directory {}: {}",
            target_skills_dir.display(),
            error
        )
    })?;

    let manifest_path = target_skills_dir.join(ASSISTANT_BUNDLED_SKILLS_MANIFEST_FILE_NAME);
    let source_manifest = build_bundled_skills_manifest(source_skills_dir)?;
    let source_skill_names = source_manifest
        .skills
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let installed_skill_names = build_marked_bundled_skills_manifest(target_skills_dir)?
        .skills
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let persisted_manifest = load_persisted_bundled_skills_manifest(&manifest_path)?;
    let installed_manifest = match persisted_manifest.as_ref() {
        Some(manifest) => manifest.clone(),
        None => build_marked_bundled_skills_manifest(target_skills_dir)?,
    };

    if installed_manifest == source_manifest && installed_skill_names == source_skill_names {
        if persisted_manifest.is_none() {
            write_manifest_atomically(&manifest_path, &source_manifest)?;
        }
        return Ok(());
    }

    for obsolete_skill in installed_skill_names.difference(&source_skill_names) {
        let obsolete_dir = target_skills_dir.join(obsolete_skill);
        if obsolete_dir.exists() {
            std::fs::remove_dir_all(&obsolete_dir).map_err(|error| {
                format!(
                    "Failed to delete obsolete bundled assistant skill {} for '{}': {}",
                    obsolete_skill, assistant_name, error
                )
            })?;
        }
    }

    for (skill_name, source_hash) in &source_manifest.skills {
        let target_skill_dir = target_skills_dir.join(skill_name);
        let needs_update = if target_skill_dir.exists() {
            let target_hash = hash_skill_directory(&target_skill_dir)?;
            installed_manifest.skills.get(skill_name) != Some(source_hash)
                || source_hash != &target_hash
                || !target_skill_dir.join(BUNDLED_SKILL_MARKER).is_file()
        } else {
            true
        };

        if needs_update {
            replace_skill_directory_atomically(
                &source_skills_dir.join(skill_name),
                &target_skill_dir,
            )?;
            log::info!(
                "Synced skill '{}' for assistant '{}'",
                skill_name,
                assistant_name
            );
        }
    }

    write_manifest_atomically(&manifest_path, &source_manifest)?;
    Ok(())
}
