use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{ContextVolatility, MCPResult, MCPTool, ServiceContext};
use crate::repositories::assistant_repository::AssistantRepository;
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
        let global_skills_dir =
            match crate::services::skill_service::get_configured_skills_directory().await {
                Ok(dir) => std::path::PathBuf::from(dir),
                Err(e) => {
                    warn!("Failed to get skills directory: {}", e);
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
            global_skills_dir,
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

        let skills_xml = skills
            .iter()
            .map(|s| {
                let source_attr = s.source.as_ref().map(|src| format!(" source=\"{}\"", src)).unwrap_or_default();
                format!(
                    "  <skill{}>\n    <name>{}</name>\n    <description>{}</description>\n    <location>{}</location>\n  </skill>",
                    source_attr, s.name, s.description, s.path
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "## Available Skills\n\n\
            You have access to the following skills. The <location> tag specifies the main documentation file for each skill.\n\
            To use a skill, you MUST first read its <location> file using the `readFile` tool. This file contains all necessary instructions and commands.\n\n\
            <available_skills>\n{}\n</available_skills>",
            skills_xml
        );

        ServiceContext::new(prompt)
            .with_structured_state(serde_json::json!({
                "count": skills.len(),
                "skills": skills,
            }))
            .with_volatility(ContextVolatility::Stable)
    }
}
