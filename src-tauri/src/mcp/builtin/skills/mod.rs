use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, MCPTool, ServiceContext};
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

#[async_trait]
impl BuiltinMCPServer for SkillsServer {
    fn name(&self) -> &str {
        "skills"
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

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let skills_dir = match Self::get_skills_directory().await {
            Ok(dir) => dir,
            Err(e) => {
                warn!("Failed to get skills directory: {}", e);
                return ServiceContext {
                    context_prompt: format!("## Skills\n\n⚠️ Failed to load skills: {}", e),
                    structured_state: None,
                };
            }
        };

        let skills = match crate::commands::skill_commands::scan_skills_directory(skills_dir).await
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
                format!(
                    "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n    <location>{}</location>\n  </skill>",
                    s.name, s.description, s.path
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
