use super::super::contracts::{
    HookPreviewEntry, HookPreviewExecutionStatus, McpServerPreviewEntry, PreparedSkillImport,
    SKILL_FILE_NAME,
};
use super::super::plugin_manifest::{self, PluginManifest};
use super::super::plugin_mcp::collect_plugin_mcp_servers;
use super::super::scanning::copy_dir_recursive;
use super::archive::commit_from_archive_root;
use crate::services::hooks::registry::{
    get_source, save_source_in, PluginMetadata, PluginSourceManifest, PluginSourceRecord,
};
use crate::services::hooks::schema::{HookDispatchMode, HooksConfig};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) async fn sync_installed_plugin_mcp_servers(
    source_key: &str,
) -> Result<Vec<String>, String> {
    let source_id = plugin_source_id(source_key);
    let record = get_source(&source_id)?;
    sync_plugin_mcp_rows_from_record(&record).await
}

/// Re-sync one plugin source's plugin-agent rows in the assistant store
/// after a global install/update (#2923 D2). Reads back the persisted
/// source record, so it must run after `install_prepared_import`.
pub(super) async fn sync_installed_plugin_agents(source_key: &str) -> Result<(), String> {
    let source_id = plugin_source_id(source_key);
    let record = get_source(&source_id)?;
    super::super::plugin_agents::sync_global_plugin_agents(&record).await
}

/// The sync body once the persisted source record is read (see
/// [`sync_installed_plugin_mcp_servers`]). Extracted so the remove-before-
/// every-return rule (gatekeeper G1.1) is testable through the record
/// itself, without a global plugin directory (gatekeeper G2.1).
pub(in crate::services::skill_service) async fn sync_plugin_mcp_rows_from_record(
    record: &PluginSourceRecord,
) -> Result<Vec<String>, String> {
    let source_id = &record.manifest.source_id;
    super::super::plugin_mcp::remove_global_plugin_mcp_servers(source_id).await?;
    if record.manifest.mcp_servers.is_empty() {
        return Ok(Vec::new());
    }
    if !record.manifest.mcp_approved {
        log::info!(
            "Plugin {} declares MCP servers but they were not approved at install; not starting them",
            source_id
        );
        return Ok(Vec::new());
    }
    // #2926: a plugin-declared server may reference its data directory and its
    // `userConfig` values, exactly like its hooks may.
    let data_dir = crate::services::hooks::registry::get_user_plugin_data_dir(source_id).ok();
    let (options, warning) = match crate::services::hooks::registry::get_user_plugins_directory() {
        Ok(store_dir) => {
            super::super::plugin_options::load_option_values(&store_dir, source_id, None).await
        }
        Err(_) => (BTreeMap::new(), None),
    };
    if let Some(warning) = warning {
        log::warn!("{warning}");
    }
    super::super::plugin_mcp::create_plugin_mcp_server_rows_in(
        source_id,
        &super::super::plugin_mcp::PluginExpansion {
            root: &record.root_dir,
            data_dir: data_dir.as_deref(),
            options: &options,
        },
        &record.manifest.mcp_servers,
    )
    .await
}

pub(super) fn collect_plugin_hook_files(
    scan_root: &Path,
    manifest: Option<&PluginManifest>,
) -> Result<Vec<PathBuf>, String> {
    let hooks_dir = scan_root.join("hooks");
    let default_main = hooks_dir.join("hooks.json");
    let mut files: Vec<PathBuf> = Vec::new();
    for candidate in [
        default_main.as_path(),
        hooks_dir.join("hooks-cursor.json").as_path(),
    ] {
        if candidate.is_file() {
            files.push(candidate.to_path_buf());
        }
    }

    if let Some(paths) = plugin_manifest::hook_config_paths(manifest) {
        for raw in paths {
            let resolved = scan_root.join(&raw);
            if resolved == default_main {
                return Err(format!(
                    "plugin manifest hooks path {raw:?} duplicates the default hooks/hooks.json"
                ));
            }
            if !resolved.is_file() {
                return Err(format!(
                    "plugin manifest hooks path {raw:?} not found in the plugin"
                ));
            }
            files.push(resolved);
        }
    }

    Ok(files)
}

pub(super) fn try_load_plugin_hooks(
    scan_root: &Path,
    manifest: Option<&PluginManifest>,
) -> Result<Option<HooksConfig>, String> {
    let files = collect_plugin_hook_files(scan_root, manifest)?;
    let inline = plugin_manifest::hook_inline_values(manifest);
    if files.is_empty() && inline.is_empty() {
        return Ok(None);
    }
    let mut config = if files.is_empty() {
        HooksConfig::empty()
    } else {
        crate::services::hooks::schema::load_hooks_from_config_files(&files)?
    };
    // Inline manifest hooks (#3005) are add-semantics like the file entries,
    // and are validated here so a bad object fails the install preview.
    crate::services::hooks::schema::merge_hooks_inline(
        &mut config,
        &inline,
        "plugin manifest hooks",
    )?;
    Ok(Some(config))
}

/// #2924 (D3, gatekeeper G1.3): preview everything the plugin can run —
/// command with its full argument line, env variable names, and HTTP header
/// names (values never) — so the one-time approval is informed.
pub(super) fn build_mcp_preview_entries(
    servers: &[crate::services::hooks::registry::PluginMcpServerSnapshot],
) -> Vec<McpServerPreviewEntry> {
    servers
        .iter()
        .map(|server| {
            let entry = &server.entry;
            let declared_names = |field: &str| -> Option<Vec<String>> {
                let mut names: Vec<String> = entry
                    .get(field)
                    .and_then(|v| v.as_object())
                    .map(|map| map.keys().cloned().collect())
                    .unwrap_or_default();
                (!names.is_empty()).then(|| {
                    names.sort();
                    names
                })
            };
            let args = entry
                .get("args")
                .and_then(|v| v.as_array())
                .map(|items| {
                    // Quote arguments containing whitespace so the joined
                    // line stays unambiguous (gatekeeper S2.2).
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .map(|arg| {
                            if arg.contains(char::is_whitespace) {
                                format!("\"{arg}\"")
                            } else {
                                arg.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|joined| !joined.is_empty());
            McpServerPreviewEntry {
                name: server.name.clone(),
                transport: if entry.get("url").and_then(|v| v.as_str()).is_some() {
                    "http".to_string()
                } else {
                    "stdio".to_string()
                },
                command: entry
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                args,
                env_names: declared_names("env"),
                url: entry
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                header_names: declared_names("headers"),
            }
        })
        .collect()
}

pub(super) fn build_hook_preview_entries(config: &HooksConfig) -> Vec<HookPreviewEntry> {
    let mut entries = Vec::new();
    for (event, groups) in &config.events {
        let execution_status = match event.dispatch_mode() {
            HookDispatchMode::Execute => HookPreviewExecutionStatus::Executed,
            HookDispatchMode::Stub => HookPreviewExecutionStatus::Stubbed,
        };
        for group in groups {
            for handler in &group.handlers {
                // HTTP handlers run too; show their POST target where a
                // command hook shows its command (#3009). The header names
                // with their raw `$VAR` templates and the allowed env-var
                // names must also be visible — the allowlist is written by
                // the plugin, so the user approves exactly which variables
                // may leave the machine (#3041 G1.1).
                let (command, http_headers, http_allowed_env_vars) = match &handler.http {
                    Some(http) => (
                        http.url.clone(),
                        (!http.headers.is_empty()).then(|| http.headers.clone()),
                        (!http.allowed_env_vars.is_empty()).then(|| http.allowed_env_vars.clone()),
                    ),
                    None => (handler.command.clone(), None, None),
                };
                entries.push(HookPreviewEntry {
                    event: event.as_str().to_string(),
                    matcher: group.matcher.clone(),
                    command,
                    args: handler.args.clone(),
                    powershell: handler.powershell.clone(),
                    script_body: None,
                    execution_status,
                    http_headers,
                    http_allowed_env_vars,
                });
            }
        }
    }
    for (raw_event, unsupported) in &config.unsupported_events {
        for item in unsupported {
            entries.push(HookPreviewEntry {
                event: raw_event.clone(),
                matcher: item.matcher.clone(),
                command: item.command.clone(),
                args: None,
                powershell: None,
                script_body: None,
                execution_status: HookPreviewExecutionStatus::Stubbed,
                http_headers: None,
                http_allowed_env_vars: None,
            });
        }
    }
    // Handler types LibrAgent does not run yet are marked unsupported in the
    // preview instead of failing the install (#3009).
    for (event_name, unsupported) in &config.unsupported_handlers {
        for item in unsupported {
            entries.push(HookPreviewEntry {
                event: event_name.clone(),
                matcher: item.matcher.clone(),
                command: item.command.clone(),
                args: None,
                powershell: None,
                script_body: None,
                execution_status: HookPreviewExecutionStatus::Stubbed,
                http_headers: None,
                http_allowed_env_vars: None,
            });
        }
    }
    entries.sort_by(|a, b| {
        a.event
            .cmp(&b.event)
            .then_with(|| a.matcher.cmp(&b.matcher))
            .then_with(|| a.command.cmp(&b.command))
    });
    entries
}

pub(in crate::services::skill_service) fn plugin_source_id(source_key: &str) -> String {
    let digest = Sha256::digest(source_key.as_bytes());
    format!("plugin-{}", &format!("{:x}", digest)[..16])
}

/// Rename plugin skills whose names collide with existing skills to
/// `<plugin-name>:<skill-name>` (Claude Code's namespace convention, #2921),
/// rewriting the staged `SKILL.md` so the installed skill carries the
/// namespaced name. Neither side hides the other afterwards.
pub(super) fn namescope_plugin_skills(
    prepared: &mut PreparedSkillImport,
    plugin_name: &str,
    existing_lower: &HashSet<String>,
) -> Result<(), String> {
    for skill in &mut prepared.discovered_skills {
        if !existing_lower.contains(&skill.metadata.name.to_lowercase()) {
            continue;
        }
        let renamed = format!("{}:{}", plugin_name, skill.metadata.name);
        rewrite_skill_frontmatter_name(&skill.root_dir.join(SKILL_FILE_NAME), &renamed)?;
        skill.metadata.name = renamed;
    }
    Ok(())
}

/// Replace the top-level `name:` line inside a skill's YAML frontmatter with
/// `new_name`, keeping every other line — and the original line endings —
/// byte-identical where possible. Only a non-indented `name:` matches: an
/// indented one belongs to a nested map, not the skill name (gatekeeper S1.2).
pub(super) fn rewrite_skill_frontmatter_name(
    skill_file: &Path,
    new_name: &str,
) -> Result<(), String> {
    let raw = fs::read_to_string(skill_file)
        .map_err(|error| format!("read {}: {error}", skill_file.display()))?;
    let content = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| format!("{}: missing YAML frontmatter", skill_file.display()))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| format!("{}: missing YAML frontmatter end", skill_file.display()))?;
    let (frontmatter, tail) = (&rest[..end], &rest[end..]);
    // A JSON string is a valid YAML double-quoted scalar for any name.
    let quoted = serde_json::to_string(new_name).map_err(|error| error.to_string())?;
    let mut replaced = false;
    let rewritten = frontmatter
        .split('\n')
        .map(|line| {
            let (body, eol) = match line.strip_suffix('\r') {
                Some(body) => (body, "\r"),
                None => (line, ""),
            };
            if !replaced && body.starts_with("name:") {
                replaced = true;
                format!("name: {quoted}{eol}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !replaced {
        return Err(format!(
            "{}: no name field in frontmatter",
            skill_file.display()
        ));
    }
    fs::write(skill_file, format!("---{rewritten}{tail}")).map_err(|error| error.to_string())
}

fn plugin_metadata_from_manifest(manifest: Option<&PluginManifest>) -> Option<PluginMetadata> {
    let manifest = manifest?;
    Some(PluginMetadata {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        author: manifest
            .author
            .as_ref()
            .map(|author| author.display_name().to_string()),
        homepage: manifest.homepage.clone(),
        repository: manifest.repository.clone(),
        license: manifest.license.clone(),
        keywords: manifest.keywords.clone().unwrap_or_default(),
    })
}

pub(super) fn install_plugin_source(
    scan_root: &Path,
    source_key: &str,
    skill_names: &[String],
    approve_hooks: bool,
    plugins_dir: &Path,
) -> Result<(), String> {
    let manifest = plugin_manifest::load_plugin_manifest(scan_root)?;
    let hook_files = collect_plugin_hook_files(scan_root, manifest.as_ref())?;
    let config = if hook_files.is_empty() {
        None
    } else {
        Some(crate::services::hooks::schema::load_hooks_from_config_files(&hook_files)?)
    };
    // Relative, slash-separated for the source store: dispatch loads exactly
    // these files from the installed root (gatekeeper G2.2).
    let hook_file_paths: Vec<String> = hook_files
        .iter()
        .filter_map(|file| file.strip_prefix(scan_root).ok())
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .collect();

    let source_id = plugin_source_id(source_key);
    // #2927: an update keeps the state the user chose; a fresh install starts
    // in the manifest's `defaultEnabled` state (enabled when it says nothing).
    let previous_enabled = crate::services::hooks::registry::get_source_in(plugins_dir, &source_id)
        .ok()
        .map(|record| record.manifest.enabled);
    let enabled = previous_enabled.unwrap_or_else(|| {
        manifest
            .as_ref()
            .map(PluginManifest::starts_enabled)
            .unwrap_or(true)
    });
    let installed_commit = commit_from_archive_root(scan_root);
    let target = plugins_dir.join(&source_id);
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    }
    copy_dir_recursive(scan_root, &target)?;

    let hooks_approved = approve_hooks && config.is_some();
    let approval_hashes = match &config {
        Some(config) if hooks_approved => {
            crate::services::hooks::registry::compute_handler_approval_hashes(&target, config)
        }
        _ => BTreeMap::new(),
    };

    // #2924 (D3): one install-time approval covers hooks and MCP servers
    // alike — the dialog shows both before the user agrees.
    let mcp_servers = collect_plugin_mcp_servers(scan_root, manifest.as_ref())?;
    let mcp_approved = approve_hooks && !mcp_servers.is_empty();

    save_source_in(
        plugins_dir,
        PluginSourceManifest {
            source_url: source_key.to_string(),
            source_id,
            skill_names: skill_names.to_vec(),
            hooks: config.as_ref().map(|config| {
                crate::services::hooks::registry::HooksConfigSnapshot {
                    version: config.version,
                    validated: true,
                }
            }),
            hooks_approved,
            approval_hashes,
            created_at: chrono::Utc::now().timestamp(),
            plugin: plugin_metadata_from_manifest(manifest.as_ref()),
            hook_files: hook_file_paths,
            inline_hooks: plugin_manifest::hook_inline_values(manifest.as_ref()),
            mcp_servers,
            mcp_approved,
            enabled,
            installed_commit,
        },
    )?;

    Ok(())
}
