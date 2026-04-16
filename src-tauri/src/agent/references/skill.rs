use super::ReferenceResolver;
use crate::services::skill_service;
use async_trait::async_trait;
use std::path::PathBuf;

/// Max size for inlining skill content into context (100 KB)
const MAX_SKILL_INLINE_BYTES: u64 = 100 * 1024;

/// Resolves `@skill:name` references by reading the corresponding SKILL.md file.
pub struct SkillReferenceResolver {
    session_id: String,
    assistant_id: Option<String>,
}

impl SkillReferenceResolver {
    pub fn new(session_id: &str, assistant_id: Option<&str>) -> Self {
        Self {
            session_id: session_id.to_string(),
            assistant_id: assistant_id.map(str::to_string),
        }
    }
}

#[async_trait]
impl ReferenceResolver for SkillReferenceResolver {
    fn type_name(&self) -> &'static str {
        "skill"
    }

    /// Looks up the skill named `arg` in the configured skills directory and returns its content.
    async fn resolve(&self, arg: &str) -> Option<String> {
        let system_dir = skill_service::get_system_skills_directory().ok()?;
        let user_dir = skill_service::get_user_skills_directory().ok()?;
        let assistant_dir = self.assistant_id.as_deref().and_then(|assistant_id| {
            skill_service::get_assistant_skills_directory(assistant_id).ok()
        });
        let workspace_dir =
            skill_service::get_workspace_skills_directory_for_session(&self.session_id).ok();

        let skills = skill_service::resolve_skills(
            system_dir.clone(),
            user_dir.clone(),
            assistant_dir.clone(),
            workspace_dir.clone(),
        )
        .await
        .ok()?;

        // Find skill by case-insensitive name match
        let skill = skills
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(arg))?;

        let path = PathBuf::from(&skill.path);

        // Guard: check file size before reading
        let file_size = tokio::fs::metadata(&path).await.ok()?.len();
        if file_size > MAX_SKILL_INLINE_BYTES {
            return Some(format!(
                "# Follow Instruction `{}`\n\n⚠️ Skill file is too large to inline ({} KB).",
                skill.path,
                file_size / 1024
            ));
        }

        let allowed_roots = skill_service::collect_allowed_skill_roots(
            system_dir,
            user_dir,
            assistant_dir,
            workspace_dir,
        );
        let content =
            skill_service::get_skill_content_from_roots(skill.path.clone(), &allowed_roots)
                .await
                .ok()?;
        let base_dir = path.parent().unwrap_or(&path);

        // Pre-inject content with explicit Base Directory metadata.
        // This gives the AI immediate access to instructions while clarifying the
        // absolute path context for any relative resources or templates mentioned in the skill.
        Some(format!(
            "# Follow Instruction `{}`\n\
            **Base Directory for this skill is**: `{}`\n\n\
            --- Content Start ---\n\n\
            {}\n\n\
            --- Content End ---\n\n\
            **Note**: All relative paths mentioned in the instructions above must be interpreted relative to the **Base Directory** provided.",
            path.display(),
            base_dir.display(),
            content
        ))
    }
}
