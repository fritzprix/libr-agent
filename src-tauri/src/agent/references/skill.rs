use super::ReferenceResolver;
use crate::services::skill_service::{self, SkillMetadata};
use async_trait::async_trait;
use std::path::PathBuf;

/// Resolves `@skill:name` references with skill metadata and read guidance.
/// Full SKILL.md content is not inlined; the agent is directed to read the file.
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

pub(crate) fn format_skill_reference_block(skill: &SkillMetadata) -> String {
    let path = PathBuf::from(&skill.path);
    let base_dir = path.parent().unwrap_or(&path);
    let instructions_path = path.display().to_string();

    format!(
        "**Name:** {}\n\
         **Description:** {}\n\
         **Instructions file:** `{}`\n\
         **Base directory:** `{}`\n\n\
         **Required — read before any other action:** The user message above is the primary \
         task. This block only identifies which skill applies. You MUST NOT call other tools, \
         delegate, or answer from the description alone.\n\n\
         1. First, call `workspace__readFile(path: \"{}\")`.\n\
         2. Follow the workflow in that file exactly.\n\
         3. Apply it to the task stated in the user message above.\n\n\
         Do not infer the skill workflow from its name or description. Skipping the read step \
         is not allowed. Interpret any relative paths mentioned in the skill relative to the \
         base directory.",
        skill.name,
        skill.description,
        instructions_path,
        base_dir.display(),
        instructions_path
    )
}

#[async_trait]
impl ReferenceResolver for SkillReferenceResolver {
    fn type_name(&self) -> &'static str {
        "skill"
    }

    fn append_after_user_text(&self) -> bool {
        true
    }

    /// Looks up the skill named `arg` and returns metadata plus read guidance.
    async fn resolve(&self, arg: &str) -> Option<String> {
        let system_dir = skill_service::get_system_skills_directory().ok()?;
        let user_dir = skill_service::get_user_skills_directory().ok()?;
        let assistant_dir = self.assistant_id.as_deref().and_then(|assistant_id| {
            skill_service::get_assistant_skills_directory(assistant_id).ok()
        });
        let workspace_dir =
            skill_service::get_workspace_skills_directory_for_session(&self.session_id).ok();

        let skills =
            skill_service::resolve_skills(system_dir, user_dir, assistant_dir, workspace_dir)
                .await
                .ok()?;

        let skill = skills
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(arg))?;

        if !PathBuf::from(&skill.path).is_file() {
            return None;
        }

        Some(format_skill_reference_block(&skill))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_skill_reference_block_includes_metadata_and_read_guidance() {
        let skill = SkillMetadata {
            name: "delegate".to_string(),
            description: "Delegate work between sessions.".to_string(),
            path: "/tmp/skills/delegate/SKILL.md".to_string(),
            source: None,
            origin: None,
        };

        let output = format_skill_reference_block(&skill);

        assert!(output.contains("**Name:** delegate"));
        assert!(output.contains("**Description:** Delegate work between sessions."));
        assert!(output.contains("**Instructions file:** `/tmp/skills/delegate/SKILL.md`"));
        assert!(output.contains("**Base directory:** `/tmp/skills/delegate`"));
        assert!(output.contains("workspace__readFile(path: \"/tmp/skills/delegate/SKILL.md\")"));
        assert!(output.contains("Required — read before any other action"));
        assert!(output.contains("Skipping the read step is not allowed"));
        assert!(!output.contains("--- Content Start ---"));
    }
}
