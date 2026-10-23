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

/// Retired bundled skill names and their replacements, so old `@skill:` references keep working.
const RENAMED_SKILLS: &[(&str, &str)] = &[("tool-installer", "mcp-installer")];

fn renamed_skill_target(name: &str) -> Option<&'static str> {
    RENAMED_SKILLS
        .iter()
        .find(|(old, _)| old.eq_ignore_ascii_case(name))
        .map(|(_, new)| *new)
}

fn renamed_skill_notice(old: &str, new: &str) -> String {
    format!("**Note:** `@skill:{old}` was renamed to `{new}`. Use `@skill:{new}` from now on.\n\n")
}

fn display_base_directory(path: &str) -> String {
    let path = PathBuf::from(path);
    path.parent().unwrap_or(&path).display().to_string()
}

/// Location lines for a Docker session, where host skill paths do not exist in the
/// container. `None` when the skill has no container copy (assistant/workspace skills).
fn docker_location_metadata(alias_path: &str) -> Option<String> {
    let container_base_dir = crate::session_isolation::container_path_for_skill_alias(
        &display_base_directory(alias_path),
    )?;
    Some(format!(
        "**Instructions file alias:** `{alias_path}`\n\
         **Base directory (container):** `{container_base_dir}`\n\n\
         Read skill files through the alias. Run skill scripts in workspace__runShell from the \
         container base directory; host skill paths do not exist in this Docker session.\n\n"
    ))
}

pub fn format_skill_reference_block(skill: &SkillMetadata, docker_session: bool) -> String {
    let path = PathBuf::from(&skill.path);
    let instructions_path = path.display().to_string();
    let base_dir = display_base_directory(&instructions_path);
    let preferred_path = skill.alias_path.as_deref().unwrap_or(&instructions_path);
    let preferred_base_dir = skill
        .alias_path
        .as_deref()
        .map(display_base_directory)
        .unwrap_or_else(|| base_dir.clone());
    let docker_metadata = if docker_session {
        skill
            .alias_path
            .as_deref()
            .and_then(docker_location_metadata)
    } else {
        None
    };
    let alias_metadata = docker_metadata.or_else(|| {
        skill.alias_path.as_ref().map(|alias_path| {
            format!(
                "**Instructions file alias:** `{}`\n\
             **Instructions file:** `{}`\n\
             **Base directory alias:** `{}`\n\
             **Base directory:** `{}`\n\n\
             The alias path is preferred. Use the absolute path only as a fallback if alias \
             access is unavailable in the current context.\n\n",
                alias_path, instructions_path, preferred_base_dir, base_dir
            )
        })
    });
    let location_metadata = alias_metadata.unwrap_or_else(|| {
        format!(
            "**Instructions file:** `{}`\n\
             **Base directory:** `{}`\n\n",
            instructions_path, base_dir
        )
    });

    format!(
        "**Name:** {}\n\
         **Description:** {}\n\
         {}\
         **Required — read before any other action:** The user message above is the primary \
         task. This block only identifies which skill applies. You MUST NOT call other tools, \
         delegate, or answer from the description alone.\n\n\
         1. First, call `workspace__readFile(path: \"{}\")`.\n\
         2. Follow the workflow in that file exactly.\n\
         3. Apply it to the task stated in the user message above.\n\n\
         Do not infer the skill workflow from its name or description. Skipping the read step \
         is not allowed. Interpret any relative paths mentioned in the skill relative to the \
         base directory.",
        skill.name, skill.description, location_metadata, preferred_path
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

        let find = |name: &str| {
            skills
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .filter(|s| PathBuf::from(&s.path).is_file())
        };
        let docker_session =
            crate::mcp::builtin::workspace::utils::is_session_docker_isolated(&self.session_id)
                .await;

        if let Some(skill) = find(arg) {
            return Some(format_skill_reference_block(skill, docker_session));
        }

        let new_name = renamed_skill_target(arg)?;
        let skill = find(new_name)?;
        Some(format!(
            "{}{}",
            renamed_skill_notice(arg, new_name),
            format_skill_reference_block(skill, docker_session)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_tool_installer_maps_to_mcp_installer() {
        assert_eq!(
            renamed_skill_target("tool-installer"),
            Some("mcp-installer")
        );
        assert_eq!(
            renamed_skill_target("Tool-Installer"),
            Some("mcp-installer")
        );
        assert_eq!(renamed_skill_target("mcp-installer"), None);
        assert_eq!(renamed_skill_target("delegate"), None);
    }

    #[test]
    fn renamed_skill_notice_names_both_skills() {
        let notice = renamed_skill_notice("tool-installer", "mcp-installer");
        assert!(notice.contains("`@skill:tool-installer` was renamed to `mcp-installer`"));
        assert!(notice.contains("`@skill:mcp-installer`"));
    }

    #[test]
    fn format_skill_reference_block_includes_metadata_and_read_guidance() {
        let skill = SkillMetadata {
            name: "delegate".to_string(),
            description: "Delegate work between sessions.".to_string(),
            path: "/tmp/skills/delegate/SKILL.md".to_string(),
            alias_path: Some("@system-skills/delegate/SKILL.md".to_string()),
            source: None,
            origin: None,
        };

        let output = format_skill_reference_block(&skill, false);

        assert!(output.contains("**Name:** delegate"));
        assert!(output.contains("**Description:** Delegate work between sessions."));
        assert!(output.contains("**Instructions file alias:** `@system-skills/delegate/SKILL.md`"));
        assert!(output.contains("**Instructions file:** `/tmp/skills/delegate/SKILL.md`"));
        assert!(output.contains("**Base directory alias:** `@system-skills/delegate`"));
        assert!(output.contains("**Base directory:** `/tmp/skills/delegate`"));
        assert!(output.contains("workspace__readFile(path: \"@system-skills/delegate/SKILL.md\")"));
        assert!(output.contains("Required — read before any other action"));
        assert!(output.contains("Skipping the read step is not allowed"));
        assert!(!output.contains("--- Content Start ---"));
    }

    fn skill_with_alias(alias_path: &str) -> SkillMetadata {
        SkillMetadata {
            name: "skill-deployer".to_string(),
            description: "Deploy skills.".to_string(),
            path: "/host/data/system_skills/skill-deployer/SKILL.md".to_string(),
            alias_path: Some(alias_path.to_string()),
            source: None,
            origin: None,
        }
    }

    #[test]
    fn docker_session_shows_container_base_directory_instead_of_host_path() {
        let skill = skill_with_alias("@system-skills/skill-deployer/SKILL.md");

        let output = format_skill_reference_block(&skill, true);

        assert!(output.contains(
            "**Base directory (container):** `/opt/libragent/skills/system/skill-deployer`"
        ));
        assert!(output
            .contains("workspace__readFile(path: \"@system-skills/skill-deployer/SKILL.md\")"));
        assert!(!output.contains("/host/data"));
    }

    #[test]
    fn docker_session_keeps_host_layout_for_skills_without_container_copy() {
        let skill = skill_with_alias("@assistant-skills/skill-deployer/SKILL.md");

        let output = format_skill_reference_block(&skill, true);

        assert!(output.contains("**Base directory:** `/host/data/system_skills/skill-deployer`"));
        assert!(!output.contains("(container)"));
    }
}
