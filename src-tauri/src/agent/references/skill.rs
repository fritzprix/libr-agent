use super::ReferenceResolver;
use crate::services::skill_service;
use async_trait::async_trait;
use std::path::PathBuf;

/// Resolves `@skill:name` references by reading the corresponding SKILL.md file.
pub struct SkillReferenceResolver;

#[async_trait]
impl ReferenceResolver for SkillReferenceResolver {
    fn type_name(&self) -> &'static str {
        "skill"
    }

    /// Looks up the skill named `arg` in the configured skills directory and returns its content.
    async fn resolve(&self, arg: &str) -> Option<String> {
        let skills_dir_str = skill_service::get_configured_skills_directory()
            .await
            .ok()?;
        let skills_dir = PathBuf::from(skills_dir_str);

        let skills = skill_service::scan_skills_directory(&skills_dir)
            .await
            .ok()?;

        // Find skill by case-insensitive name match
        let skill = skills
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(arg))?;

        skill_service::get_skill_content(skill.path).await.ok()
    }
}
