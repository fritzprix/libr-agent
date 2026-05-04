mod cache;
mod contracts;
mod directories;
mod github;
mod importing;
mod scanning;

pub use cache::{invalidate_skill_scan_cache, prewarm_managed_skill_scans};
pub use contracts::{
    GitHubRepoSpec, ManagedSkillsOverview, SkillImportCandidate, SkillImportConflict,
    SkillImportPreview, SkillImportResult, SkillMetadata, LEGACY_SYSTEM_SKILLS_DIR_NAME,
    MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME, SKILL_FILE_NAME, SYSTEM_SKILLS_DIR_NAME,
    USER_SKILLS_DIR_NAME,
};
pub use directories::{
    collect_allowed_skill_roots, get_assistant_skills_directory, get_configured_skills_directory,
    get_default_skills_directory, get_legacy_global_skills_directory, get_managed_skills_overview,
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
pub use scanning::{
    copy_dir_recursive, get_skill_content, get_skill_content_from_roots, parse_skill_metadata,
    scan_skills_directory,
};

pub(crate) use cache::scan_skills_internal_cached;
