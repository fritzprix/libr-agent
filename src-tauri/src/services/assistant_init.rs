use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::repositories::AssistantRepository;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

// Embedded prompts and configs (SSOT - Single Source of Truth)
const MASTER_MIND_PROMPT: &str = include_str!("../../bundled_assistants/Master Mind/prompt.md");
const MASTER_MIND_CONFIG: &str =
    include_str!("../../bundled_assistants/Master Mind/mcp-config.json");

const LIBR_ASSISTANT_PROMPT: &str =
    include_str!("../../bundled_assistants/Libr Assistant/prompt.md");
const LIBR_ASSISTANT_CONFIG: &str =
    include_str!("../../bundled_assistants/Libr Assistant/mcp-config.json");

const CODING_EXPERT_PROMPT: &str = include_str!("../../bundled_assistants/Coding Expert/prompt.md");
const CODING_EXPERT_CONFIG: &str =
    include_str!("../../bundled_assistants/Coding Expert/mcp-config.json");

const APP_WIZARD_PROMPT: &str = include_str!("../../bundled_assistants/App Wizard/prompt.md");
const APP_WIZARD_CONFIG: &str = include_str!("../../bundled_assistants/App Wizard/mcp-config.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BundledAssistantConfig {
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) mcp_server_ids: Vec<String>,
    #[serde(default = "default_false")]
    pub(crate) deletion_protected: bool,
    #[serde(default)]
    pub(crate) local_services: Vec<String>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    pub(crate) allowed_builtin_service_aliases: Vec<String>,
}

fn default_false() -> bool {
    false
}

fn json_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn sorted_string_slices_equal(a: &[String], b: &[String]) -> bool {
    let mut a_sorted = a.to_vec();
    let mut b_sorted = b.to_vec();
    a_sorted.sort();
    b_sorted.sort();
    a_sorted == b_sorted
}

fn bundled_config_needs_update(config_val: &Value, assistant: &BundledAssistant) -> bool {
    config_val.get("description").and_then(|v| v.as_str())
        != Some(assistant.config.description.as_str())
        || config_val.get("systemPrompt").and_then(|v| v.as_str())
            != Some(assistant.prompt.as_str())
        || config_val
            .get("deletionProtected")
            .and_then(|v| v.as_bool())
            != Some(assistant.config.deletion_protected)
        || !sorted_string_slices_equal(
            &json_string_array(config_val, "mcpServerIds"),
            &assistant.config.mcp_server_ids,
        )
        || !sorted_string_slices_equal(
            &json_string_array(config_val, "localServices"),
            &assistant.config.local_services,
        )
        || !sorted_string_slices_equal(
            &json_string_array(config_val, "allowedBuiltInServiceAliases"),
            &assistant.config.allowed_builtin_service_aliases,
        )
}

#[derive(Debug, Clone)]
pub struct BundledAssistant {
    pub name: String,
    pub(crate) prompt: String,
    pub(crate) config: BundledAssistantConfig,
}

fn try_load_bundled_assistant(assistant_dir: &Path, name: &str) -> Option<BundledAssistant> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        log::warn!(
            "Skipping assistant directory '{}' due to invalid characters",
            name
        );
        return None;
    }

    let prompt_path = assistant_dir.join("prompt.md");
    let prompt_metadata = match std::fs::metadata(&prompt_path) {
        Ok(metadata) => metadata,
        Err(_) => {
            log::warn!(
                "Skipping assistant '{}': failed to read prompt.md metadata",
                name
            );
            return None;
        }
    };
    if prompt_metadata.len() > 1024 * 1024 {
        log::warn!("Skipping assistant '{}': prompt.md exceeds 1MB limit", name);
        return None;
    }

    let prompt = match std::fs::read_to_string(&prompt_path) {
        Ok(content) => content,
        Err(_) => {
            log::warn!("Skipping assistant '{}': failed to read prompt.md", name);
            return None;
        }
    };

    let config_path = assistant_dir.join("mcp-config.json");
    let config_bytes = match std::fs::read(&config_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            log::warn!(
                "Skipping assistant '{}': failed to read mcp-config.json",
                name
            );
            return None;
        }
    };
    if config_bytes.len() > 64 * 1024 {
        log::warn!(
            "Skipping assistant '{}': mcp-config.json exceeds 64KB limit",
            name
        );
        return None;
    }

    let config: BundledAssistantConfig = match serde_json::from_slice(&config_bytes) {
        Ok(config) => config,
        Err(_) => {
            log::warn!(
                "Skipping assistant '{}': invalid JSON in mcp-config.json",
                name
            );
            return None;
        }
    };

    if config.allowed_builtin_service_aliases.len() > 20 {
        log::warn!(
            "Skipping assistant '{}': more than 20 allowed builtin service aliases",
            name
        );
        return None;
    }

    for alias in &config.allowed_builtin_service_aliases {
        if BuiltinServiceId::from_alias(alias).is_none() {
            log::warn!(
                "Skipping assistant '{}': unauthorized or unknown builtin service alias '{}'",
                name,
                alias
            );
            return None;
        }
    }

    Some(BundledAssistant {
        name: name.to_string(),
        prompt,
        config,
    })
}

pub fn load_bundled_assistants(resource_dir: &Path) -> Result<Vec<BundledAssistant>, String> {
    let base = resource_dir.join("bundled_assistants");

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut assistants = Vec::new();
    for entry in std::fs::read_dir(&base)
        .map_err(|_| "Failed to read bundled_assistants directory".to_string())?
    {
        let entry = entry.map_err(|_| "Failed to read directory entry".to_string())?;
        let assistant_dir = entry.path();

        if !assistant_dir.is_dir() {
            continue;
        }

        let name = match assistant_dir.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => {
                log::warn!("Skipping assistant directory with invalid name encoding");
                continue;
            }
        };

        if let Some(assistant) = try_load_bundled_assistant(&assistant_dir, name) {
            assistants.push(assistant);
        }
    }

    assistants.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(assistants)
}

pub async fn ensure_default_assistants(resource_dir: Option<&Path>) -> Result<(), String> {
    let bundled = if let Some(dir) = resource_dir {
        match load_bundled_assistants(dir) {
            Ok(list) => list,
            Err(e) => {
                log::warn!(
                    "Failed to load bundled assistants: {}. Falling back to hardcoded defaults.",
                    e
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    if bundled.is_empty() {
        log::warn!("No bundled assistants found, falling back to hardcoded defaults");
        return ensure_default_assistants_hardcoded().await;
    }

    let repo = crate::get_assistant_repository();

    // Fetch all existing assistants in a single query
    let existing_assistants = repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list assistants: {}", e))?;

    let mut existing_map: HashMap<String, crate::entity::assistant::Model> = existing_assistants
        .into_iter()
        .map(|a| (a.name.clone(), a))
        .collect();

    for assistant in &bundled {
        log::info!("Ensuring assistant: {}", assistant.name);

        if let Some(existing) = existing_map.remove(&assistant.name) {
            let mut config_val =
                serde_json::from_str::<Value>(&existing.config).unwrap_or_else(|_| json!({}));

            if !bundled_config_needs_update(&config_val, assistant) {
                continue;
            }

            config_val["description"] = Value::String(assistant.config.description.clone());
            config_val["systemPrompt"] = Value::String(assistant.prompt.clone());
            config_val["mcpServerIds"] = Value::Array(
                assistant
                    .config
                    .mcp_server_ids
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );
            config_val["deletionProtected"] = Value::Bool(assistant.config.deletion_protected);
            config_val["localServices"] = Value::Array(
                assistant
                    .config
                    .local_services
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );
            config_val["allowedBuiltInServiceAliases"] = Value::Array(
                assistant
                    .config
                    .allowed_builtin_service_aliases
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );

            repo.update_assistant(&existing.id, None, Some(config_val.to_string()))
                .await
                .map_err(|e| format!("Failed to update assistant '{}': {}", assistant.name, e))?;
        } else {
            let mut config_val = json!({});
            config_val["description"] = Value::String(assistant.config.description.clone());
            config_val["systemPrompt"] = Value::String(assistant.prompt.clone());
            config_val["mcpServerIds"] = Value::Array(
                assistant
                    .config
                    .mcp_server_ids
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );
            config_val["deletionProtected"] = Value::Bool(assistant.config.deletion_protected);
            config_val["localServices"] = Value::Array(
                assistant
                    .config
                    .local_services
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );
            config_val["allowedBuiltInServiceAliases"] = Value::Array(
                assistant
                    .config
                    .allowed_builtin_service_aliases
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );

            let id = uuid::Uuid::new_v4().to_string();
            repo.create_assistant(id, assistant.name.clone(), config_val.to_string())
                .await
                .map_err(|e| format!("Failed to create assistant '{}': {}", assistant.name, e))?;
        }
    }

    // Clean up any legacy or removed default assistants (zombie prevention)
    // If an assistant exists in the DB but is NOT in the current bundle, and it is marked
    // as deletionProtected in its config, we delete it from the database.
    for (name, existing) in existing_map {
        let config_val =
            serde_json::from_str::<Value>(&existing.config).unwrap_or_else(|_| json!({}));
        let is_deletion_protected = config_val
            .get("deletionProtected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_deletion_protected {
            log::info!("Removing legacy/removed default assistant: {}", name);
            if let Err(e) = repo.delete_assistant(&existing.id).await {
                log::warn!("Failed to delete legacy assistant '{}': {}", name, e);
            }
        }
    }

    Ok(())
}

pub async fn ensure_default_assistants_hardcoded() -> Result<(), String> {
    let repo = crate::get_assistant_repository();

    let defaults = vec![
        ("Master Mind", MASTER_MIND_PROMPT, MASTER_MIND_CONFIG),
        (
            "Libr Assistant",
            LIBR_ASSISTANT_PROMPT,
            LIBR_ASSISTANT_CONFIG,
        ),
        ("Coding Expert", CODING_EXPERT_PROMPT, CODING_EXPERT_CONFIG),
        ("App Wizard", APP_WIZARD_PROMPT, APP_WIZARD_CONFIG),
    ];
    let default_names: Vec<&str> = defaults.iter().map(|(n, _, _)| *n).collect();

    for (name, prompt, config_str) in defaults {
        let exists = repo
            .check_assistant_exists(name)
            .await
            .map_err(|e| format!("Failed to check for {}: {}", name, e))?;

        let mut config_val: Value = serde_json::from_str(config_str)
            .map_err(|e| format!("Failed to parse embedded config for {}: {}", name, e))?;

        // Inject the system prompt into the config
        config_val["systemPrompt"] = Value::String(prompt.to_string());

        if !exists {
            log::info!("Creating default '{}'...", name);
            let id = uuid::Uuid::new_v4().to_string();
            repo.create_assistant(id, name.to_string(), config_val.to_string())
                .await
                .map_err(|e| format!("Failed to create {}: {}", name, e))?;
        } else {
            // Reconcile if exists
            let assistants = repo
                .list_assistants()
                .await
                .map_err(|e| format!("Failed to list assistants: {}", e))?;
            if let Some(existing) = assistants.into_iter().find(|a| a.name == name) {
                let existing_config =
                    serde_json::from_str::<Value>(&existing.config).unwrap_or_else(|_| json!({}));

                let temp_config: BundledAssistantConfig =
                    serde_json::from_value(config_val.clone())
                        .map_err(|e| format!("Failed to parse config for comparison: {}", e))?;

                let temp_bundled = BundledAssistant {
                    name: name.to_string(),
                    prompt: prompt.to_string(),
                    config: temp_config,
                };

                if bundled_config_needs_update(&existing_config, &temp_bundled) {
                    log::info!("Updating default '{}'...", name);
                    repo.update_assistant(&existing.id, None, Some(config_val.to_string()))
                        .await
                        .map_err(|e| format!("Failed to update {}: {}", name, e))?;
                }
            }
        }
    }

    // Clean up any legacy or removed default assistants in the fallback path as well
    let mut existing_map: HashMap<String, crate::entity::assistant::Model> = repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list assistants: {}", e))?
        .into_iter()
        .map(|a| (a.name.clone(), a))
        .collect();

    // Remove the current defaults from the map
    for name in default_names {
        existing_map.remove(name);
    }

    // Delete any remaining assistants that are deletion protected (e.g., legacy master-mind)
    for (name, existing) in existing_map {
        let config_val =
            serde_json::from_str::<Value>(&existing.config).unwrap_or_else(|_| json!({}));
        let is_deletion_protected = config_val
            .get("deletionProtected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_deletion_protected {
            log::info!(
                "Removing legacy/removed default assistant (fallback path): {}",
                name
            );
            if let Err(e) = repo.delete_assistant(&existing.id).await {
                log::warn!("Failed to delete legacy assistant '{}': {}", name, e);
            }
        }
    }

    Ok(())
}
