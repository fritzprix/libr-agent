use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, MCPTool, ServiceContext};
use crate::repositories::assistant_repository::AssistantRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::state::get_settings_repository;
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

    async fn get_skills_directory() -> Result<String, String> {
        let repo = get_settings_repository();

        match repo.get("systemSettings").await {
            Ok(Some(model)) => match serde_json::from_str::<Value>(&model.value) {
                Ok(json) => {
                    if let Some(skills_dir) = json.get("skillsDirectory").and_then(|v| v.as_str()) {
                        if !skills_dir.is_empty() {
                            debug!("Using configured skills directory: {}", skills_dir);
                            return Ok(skills_dir.to_string());
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse systemSettings JSON: {}", e);
                }
            },
            Err(e) => {
                warn!("Failed to get systemSettings from repository: {}", e);
            }
            Ok(None) => {
                debug!("No systemSettings found in repository");
            }
        }

        // Fallback to AppData
        let session_manager = crate::session::get_session_manager()?;
        let fallback_dir = session_manager
            .get_base_data_dir()
            .join("skills")
            .to_string_lossy()
            .to_string();
        debug!("Using default skills directory: {}", fallback_dir);
        Ok(fallback_dir)
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
        let global_skills_dir = match Self::get_skills_directory().await {
            Ok(dir) => std::path::PathBuf::from(dir),
            Err(e) => {
                warn!("Failed to get skills directory: {}", e);
                return ServiceContext {
                    context_prompt: format!("## Skills\n\n⚠️ Failed to load skills: {}", e),
                    structured_state: None,
                };
            }
        };

        // Determine assistant skills directory if assistant_id is provided
        let assistant_skills_dir = if let Some(opts_value) = options {
            if let Ok(opts) = serde_json::from_value::<crate::mcp::types::ServiceContextOptions>(
                opts_value.clone(),
            ) {
                if let Some(assistant_id) = opts.assistant_id {
                    match crate::session::get_session_manager() {
                        Ok(manager) => Some(
                            manager
                                .get_base_data_dir()
                                .join("assistants")
                                .join(assistant_id)
                                .join("skills"),
                        ),
                        Err(e) => {
                            warn!(
                                "Failed to get session manager for assistant resolving: {}",
                                e
                            );
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(ref dir) = assistant_skills_dir {
            debug!("Using assistant skills directory: {:?}", dir);
        }

        let mut skills = match crate::commands::skill_commands::resolve_skills(
            global_skills_dir,
            assistant_skills_dir,
        )
        .await
        {
            Ok(skills) => skills,
            Err(e) => {
                warn!("Failed to scan skills directory: {}", e);
                return ServiceContext {
                    context_prompt: format!("## Skills\n\n⚠️ Failed to scan skills: {}", e),
                    structured_state: None,
                };
            }
        };

        // Filter disabled skills
        if let Some(opts_value) = options {
            if let Ok(opts) = serde_json::from_value::<crate::mcp::types::ServiceContextOptions>(
                opts_value.clone(),
            ) {
                if let Some(assistant_id) = opts.assistant_id {
                    let repo = crate::state::get_assistant_repository();
                    if let Ok(Some(assistant)) = repo.get_assistant(&assistant_id).await {
                        if let Ok(config) = serde_json::from_str::<Value>(&assistant.config) {
                            if let Some(disabled) =
                                config.get("disabledSkills").and_then(|v| v.as_array())
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
            }
        }

        if skills.is_empty() {
            debug!("No skills found, returning empty context");
            return ServiceContext {
                context_prompt: String::new(),
                structured_state: None,
            };
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

        ServiceContext {
            context_prompt: prompt,
            structured_state: Some(serde_json::json!({
                "count": skills.len(),
                "skills": skills,
            })),
        }
    }
}
