mod archive;
mod assistant_skills;
mod plugin_install;
mod preview;
mod source;
mod user_install;

#[cfg(test)]
mod tests_hook_preview;
#[cfg(test)]
mod tests_plugin_install;
#[cfg(test)]
mod tests_source_flow;

pub use super::scanning::skill_storage_directory_name;
#[cfg(test)]
pub(super) use archive::prepare_local_skill_import_blocking;
pub use assistant_skills::{
    copy_global_to_assistant, delete_assistant_skill, delete_user_skill, import_assistant_skills,
    reset_assistant_skills, reset_user_skills,
};
pub(super) use plugin_install::plugin_source_id;
#[cfg(test)]
pub(super) use plugin_install::sync_plugin_mcp_rows_from_record;
pub(super) use preview::build_import_preview;
pub use preview::build_skill_import_conflicts;
pub(in crate::services::skill_service) use source::prepare_plugin_source_import;
pub use user_install::{
    import_user_skills, install_github_skills, preview_github_skill_install,
    preview_user_skill_import,
};
pub(super) use user_install::{
    install_prepared_import, prepare_plugin_user_config, remove_skill_names_from_user_skills,
    save_plugin_user_config, ImportTarget,
};
