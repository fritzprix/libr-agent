mod cache;
mod contracts;
mod directories;
mod github;
mod importing;
mod outbox;
mod plugin_agents;
mod plugin_commands;
mod plugin_manifest;
mod plugin_mcp;
mod plugin_options;
mod plugin_sources;
mod scanning;
mod session_plugins;

pub use crate::services::hooks::registry::SessionPluginStoreEntry;
pub use cache::{
    clear_skill_scan_cache_entries, invalidate_skill_scan_cache, prewarm_managed_skill_scans,
};
pub use contracts::{
    GitHubRepoSpec, HookPreviewEntry, HookPreviewExecutionStatus, ManagedSkillsOverview,
    PluginSourceOverview, PluginUpdateStatus, SkillAliasRoot, SkillImportCandidate,
    SkillImportConflict, SkillImportKind, SkillImportPreview, SkillImportResult, SkillMetadata,
    ASSISTANT_SKILLS_ALIAS_PREFIX, LEGACY_SYSTEM_SKILLS_DIR_NAME,
    MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME, PLUGIN_SKILL_OWNER_FILE, SKILL_FILE_NAME,
    SYSTEM_SKILLS_ALIAS_PREFIX, SYSTEM_SKILLS_DIR_NAME, USER_SKILLS_ALIAS_PREFIX,
    USER_SKILLS_DIR_NAME, WORKSPACE_SKILLS_ALIAS_PREFIX,
};
pub use directories::{
    build_skill_alias_path, collect_allowed_skill_roots, collect_skill_alias_roots,
    extract_skill_alias_relative_path, get_assistant_skills_directory,
    get_default_skills_directory, get_legacy_global_skills_directory, get_managed_skills_overview,
    get_managed_skills_overview_for_directories, get_session_plugin_skills_directory_for_session,
    get_system_skills_directory, get_user_skills_directory,
    get_workspace_skills_directory_for_session, get_workspace_skills_directory_from_path,
    resolve_skill_directories, resolve_skills,
};
pub use github::parse_github_repo_url;
pub use importing::{
    build_skill_import_conflicts, copy_global_to_assistant, delete_assistant_skill,
    delete_user_skill, import_assistant_skills, import_user_skills, install_github_skills,
    preview_github_skill_install, preview_user_skill_import, reset_assistant_skills,
    reset_user_skills, skill_storage_directory_name,
};
pub use outbox::{install_outbox, SKILL_OUTBOX_RELATIVE_DIR};
pub use plugin_agents::{
    declared_plugin_agent_model, is_plugin_agent_id, parse_plugin_agent_id, plugin_agent_id,
    plugin_model_to_complexity, plugin_source_of_config, reject_plugin_agent_mutation,
    resolve_plugin_source_namespace, resolve_plugin_source_namespace_in,
    resolve_session_plugin_assistant, session_plugin_agent_infos, session_store_source_ids,
    PluginAgentListInfo,
};
pub use plugin_commands::{
    expand_plugin_command, list_plugin_commands, plugin_command_names, PluginCommandInfo,
};
pub use plugin_manifest::{option_env_name, resolve_user_config_values, UserConfigSpec};
pub use plugin_options::{
    delete_sensitive_options, load_option_values as load_plugin_option_values, persist_options,
    read_saved_options, remove_saved_options, write_saved_options, SavedPluginOptions,
};
pub use plugin_sources::{
    check_plugin_source_updates, delete_plugin_source, disabled_global_plugin_source_ids,
    disabled_plugin_skill_names, list_plugin_source_overviews, set_plugin_enabled,
};
pub use scanning::{
    copy_dir_recursive, get_skill_content, get_skill_content_from_roots, parse_skill_metadata,
    scan_skills_directory,
};
pub use session_plugins::{
    install_session_plugin, list_session_plugins, load_session_plugins,
    preview_session_plugin_install, purge_session_plugin_state, resolve_plugin_by_target,
    set_plugin_enabled_by_target, uninstall_session_plugin, PluginScope, SessionPluginAction,
    SessionPluginEntry, SessionPluginHistoryEntry, SessionPluginsOverview,
};

pub(crate) use cache::scan_skills_internal_cached;
#[cfg(test)]
pub(crate) use plugin_mcp::entry_to_mcp_config_json;
pub(crate) use plugin_mcp::{build_session_plugin_server_configs, SessionPluginServerConfig};
#[cfg(test)]
pub(crate) use plugin_sources::disabled_source_ids_in;
