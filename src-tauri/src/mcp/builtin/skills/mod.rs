use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{ContextVolatility, MCPResult, MCPTool, ServiceContext};
use crate::repositories::assistant_repository::AssistantRepository;
use crate::services::skill_service::SkillMetadata;
use async_trait::async_trait;
use log::{debug, warn};
use serde_json::Value;

#[derive(Debug)]
pub struct SkillsServer {
    #[allow(dead_code)]
    session_id: String,
}

impl SkillsServer {
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

pub const NAME: &str = "skills";

/// Renders the Available Skills context. In Docker sessions the host skill path
/// does not exist in the container, so only the alias is shown (same
/// `docker_session` rule as `format_skill_reference_block`).
fn build_skills_prompt(skills: &[SkillMetadata], docker_session: bool) -> String {
    let skills_xml = skills
        .iter()
        .map(|s| {
            let source_attr = s
                .source
                .as_ref()
                .map(|src| format!(" source=\"{}\"", src))
                .unwrap_or_default();
            let origin_attr = s
                .origin
                .as_ref()
                .map(|origin| format!(" origin=\"{}\"", origin))
                .unwrap_or_default();
            let preferred_location = s.alias_path.as_deref().unwrap_or(&s.path);
            let fallback_location = s
                .alias_path
                .as_ref()
                .filter(|_| !docker_session)
                .map(|_| format!("\n    <fallback_location>{}</fallback_location>", s.path))
                .unwrap_or_default();
            format!(
                "  <skill{}{}>\n    <name>{}</name>\n    <description>{}</description>\n    <location>{}</location>{}\n  </skill>",
                source_attr, origin_attr, s.name, s.description, preferred_location, fallback_location
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let fallback_guidance = if docker_session {
        "This is a Docker session: read skills only through the <location> alias; host paths do not exist in the container.\n"
    } else {
        "When a <fallback_location> tag is present, use it only if alias access is unavailable.\n"
    };
    format!(
        "## Available Skills\n\n\
        You have access to the following skills. The <location> tag specifies the preferred documentation path for each skill.\n\
        Managed skill aliases such as @system-skills/... and @user-skills/... are read-only references. \
        {}\
        To use a skill, you MUST first read its <location> file using `workspace__readFile`. \
        Do not act from the description alone; read the file before any other tool call.\n\n\
        <available_skills>\n{}\n</available_skills>",
        fallback_guidance, skills_xml
    )
}

#[async_trait]
impl BuiltinMCPServer for SkillsServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Provides access to skill documentation and guides"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![] // No tools needed, only context
    }

    async fn call_tool(
        &self,
        _tool_name: &str,
        _args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        Err("Skills server provides context only, no tools".to_string())
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        let system_skills_dir = match crate::services::skill_service::get_system_skills_directory()
        {
            Ok(dir) => dir,
            Err(e) => {
                warn!("Failed to get system skills directory: {}", e);
                return ServiceContext::new(format!(
                    "## Skills\n\n⚠️ Failed to load skills: {}",
                    e
                ))
                .with_volatility(ContextVolatility::Stable);
            }
        };
        let user_skills_dir = match crate::services::skill_service::get_user_skills_directory() {
            Ok(dir) => dir,
            Err(e) => {
                warn!("Failed to get user skills directory: {}", e);
                return ServiceContext::new(format!(
                    "## Skills\n\n⚠️ Failed to load skills: {}",
                    e
                ))
                .with_volatility(ContextVolatility::Stable);
            }
        };

        // Parse options once — used for both scope resolution and disabledSkills filtering.
        let parsed_opts: Option<crate::mcp::types::ServiceContextOptions> =
            options.and_then(|v| serde_json::from_value(v.clone()).ok());
        let assistant_id = parsed_opts.as_ref().and_then(|o| o.assistant_id.as_deref());

        let workspace_skills_dir =
            crate::services::skill_service::get_workspace_skills_directory_for_session(
                &self.session_id,
            )
            .ok();

        // Determine assistant skills directory if assistant_id is provided
        let assistant_skills_dir = if let Some(id) = assistant_id {
            match crate::services::skill_service::get_assistant_skills_directory(id) {
                Ok(dir) => {
                    debug!("Using assistant skills directory: {:?}", dir);
                    Some(dir)
                }
                Err(e) => {
                    warn!("Failed to resolve assistant skills directory: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let mut skills = match crate::services::skill_service::resolve_skills(
            system_skills_dir,
            user_skills_dir,
            assistant_skills_dir,
            workspace_skills_dir,
        )
        .await
        {
            Ok(skills) => skills,
            Err(e) => {
                warn!("Failed to scan skills directory: {}", e);
                return ServiceContext::new(format!(
                    "## Skills\n\n⚠️ Failed to scan skills: {}",
                    e
                ))
                .with_volatility(ContextVolatility::Stable);
            }
        };

        // Filter disabled skills using the assistant_id already resolved above.
        if let Some(id) = assistant_id {
            let repo = crate::state::get_assistant_repository();
            if let Ok(Some(assistant)) = repo.get_assistant(id).await {
                if let Ok(config) = serde_json::from_str::<Value>(&assistant.config) {
                    if let Some(disabled) = config.get("disabledSkills").and_then(|v| v.as_array())
                    {
                        let disabled_set: std::collections::HashSet<String> = disabled
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();

                        skills.retain(|s| !disabled_set.contains(&s.name));
                    }
                }
            }
        }

        if skills.is_empty() {
            debug!("No skills found, returning empty context");
            return ServiceContext::new(String::new()).with_volatility(ContextVolatility::Stable);
        }

        debug!("Building skills context with {} skills", skills.len());

        let docker_session =
            crate::mcp::builtin::workspace::utils::is_session_docker_isolated(&self.session_id)
                .await;
        let prompt = build_skills_prompt(&skills, docker_session);

        ServiceContext::new(prompt)
            .with_structured_state(serde_json::json!({
                "count": skills.len(),
                "skills": skills,
            }))
            .with_volatility(ContextVolatility::Stable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aliased_skill() -> SkillMetadata {
        SkillMetadata {
            name: "delegate".to_string(),
            description: "Delegate work".to_string(),
            path: "/home/u/.local/share/libragent/skills/delegate/SKILL.md".to_string(),
            alias_path: Some("@system-skills/delegate/SKILL.md".to_string()),
            source: None,
            origin: None,
        }
    }

    #[test]
    fn host_mode_keeps_fallback_location() {
        let prompt = build_skills_prompt(&[aliased_skill()], false);
        assert!(prompt.contains("<location>@system-skills/delegate/SKILL.md</location>"));
        assert!(prompt.contains(
            "<fallback_location>/home/u/.local/share/libragent/skills/delegate/SKILL.md</fallback_location>"
        ));
        assert!(prompt.contains("use it only if alias access is unavailable"));
    }

    #[test]
    fn docker_mode_shows_only_the_alias() {
        let prompt = build_skills_prompt(&[aliased_skill()], true);
        assert!(prompt.contains("<location>@system-skills/delegate/SKILL.md</location>"));
        assert!(!prompt.contains("fallback_location"));
        assert!(!prompt.contains("/home/u/.local/share/libragent"));
    }
}
