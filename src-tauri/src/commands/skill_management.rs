use crate::services::skill_service::{
    self, PluginCommandInfo, PluginSourceOverview, SessionPluginsOverview, SkillImportPreview,
    SkillImportResult,
};

#[tauri::command]
pub async fn copy_global_to_assistant(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    skill_service::copy_global_to_assistant(assistant_id, skill_name).await
}

#[tauri::command]
pub async fn delete_assistant_skill(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    skill_service::delete_assistant_skill(assistant_id, skill_name).await
}

#[tauri::command]
pub async fn reset_assistant_skills(assistant_id: String) -> Result<String, String> {
    skill_service::reset_assistant_skills(assistant_id).await
}

#[tauri::command]
pub async fn import_assistant_skills(
    assistant_id: String,
    file_path: String,
) -> Result<String, String> {
    skill_service::import_assistant_skills(assistant_id, file_path).await
}

#[tauri::command]
pub async fn preview_user_skill_import(file_path: String) -> Result<SkillImportPreview, String> {
    skill_service::preview_user_skill_import(file_path).await
}

#[tauri::command]
pub async fn import_user_skills(
    file_path: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
) -> Result<SkillImportResult, String> {
    skill_service::import_user_skills(
        file_path,
        overwrite_existing,
        excluded_skill_names,
        approve_hooks,
    )
    .await
}

#[tauri::command]
pub async fn preview_github_skill_install(repo_url: String) -> Result<SkillImportPreview, String> {
    skill_service::preview_github_skill_install(repo_url).await
}

#[tauri::command]
pub async fn install_github_skills(
    repo_url: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
) -> Result<SkillImportResult, String> {
    skill_service::install_github_skills(
        repo_url,
        overwrite_existing,
        excluded_skill_names,
        approve_hooks,
    )
    .await
}

#[tauri::command]
pub async fn list_plugin_sources() -> Result<Vec<PluginSourceOverview>, String> {
    skill_service::list_plugin_source_overviews()
}

#[tauri::command]
pub async fn delete_plugin_source(source_id: String) -> Result<(), String> {
    skill_service::delete_plugin_source(source_id).await
}

#[tauri::command]
pub async fn delete_user_skill(skill_name: String) -> Result<String, String> {
    skill_service::delete_user_skill(skill_name).await
}

#[tauri::command]
pub async fn reset_user_skills() -> Result<String, String> {
    skill_service::reset_user_skills().await
}

/// Preview a git plugin for a session-scoped install (#2825).
#[tauri::command]
pub async fn preview_session_plugin_install(
    session_id: String,
    repo_url: String,
) -> Result<SkillImportPreview, String> {
    skill_service::preview_session_plugin_install(session_id, repo_url).await
}

/// Install a git plugin for one session only, then fire SessionStart
/// (`source=resume`) so its hooks apply immediately (#2825).
#[tauri::command]
pub async fn install_session_plugin(
    session_id: String,
    repo_url: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
) -> Result<SkillImportResult, String> {
    skill_service::install_session_plugin(
        session_id,
        repo_url,
        overwrite_existing,
        excluded_skill_names,
        approve_hooks,
    )
    .await
}

/// Plugins active in a session (global + session) and the session's install history.
#[tauri::command]
pub async fn list_session_plugins(session_id: String) -> Result<SessionPluginsOverview, String> {
    skill_service::list_session_plugins(&session_id)
}

/// Slash commands contributed by plugins active in a session (#2922).
#[tauri::command]
pub async fn list_plugin_commands(session_id: String) -> Result<Vec<PluginCommandInfo>, String> {
    skill_service::list_plugin_commands(&session_id)
}

/// Expand a plugin slash command into its prompt body (#2922). `None` means
/// the text is not a plugin command (built-ins are never shadowed).
#[tauri::command]
pub async fn expand_plugin_command(
    session_id: String,
    command_text: String,
) -> Result<Option<String>, String> {
    skill_service::expand_plugin_command(&session_id, &command_text)
}
