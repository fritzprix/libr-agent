// Skills Context Provider
// Injects available skills documentation into system prompts

use super::ContextProvider;
use crate::commands::skill_commands::scan_skills_directory;
use crate::state::get_settings_repository;
use async_trait::async_trait;

/// Context provider for skills documentation
///
/// Scans the configured skills directory and builds XML documentation
/// for all available skills to inject into system prompts.
pub struct SkillsContextProvider;

impl SkillsContextProvider {
    /// Create a new skills context provider
    pub fn new() -> Self {
        Self
    }

    /// Get skills directory from settings
    async fn get_skills_directory(&self) -> Result<String, String> {
        use crate::repositories::settings_repository::SettingsRepository;

        let settings_repo = get_settings_repository();

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SystemSettings {
            skills_directory: String,
        }

        match settings_repo.get("systemSettings").await {
            Ok(Some(model)) => {
                let settings: SystemSettings = serde_json::from_str(&model.value)
                    .map_err(|e| format!("Failed to parse system settings: {}", e))?;
                Ok(settings.skills_directory)
            }
            Ok(None) => Err("System settings not found".to_string()),
            Err(e) => Err(format!("Failed to get system settings: {}", e)),
        }
    }

    /// Build skills XML from scanned skills directory
    fn build_skills_xml(&self, skills: Vec<serde_json::Value>) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut xml_parts = vec![
            "<skills>".to_string(),
            "Here is a list of skills that contain domain specific knowledge on a variety of topics.".to_string(),
            "Each skill comes with a description of the topic and a file path that contains the detailed instructions.".to_string(),
            "When a user asks you to perform a task that falls within the domain of a skill, use the 'read_file' tool to acquire the full instructions from the file URI.".to_string(),
        ];

        for skill in skills {
            if let (Some(name), Some(description), Some(file_path)) = (
                skill.get("name").and_then(|v| v.as_str()),
                skill.get("description").and_then(|v| v.as_str()),
                skill.get("filePath").and_then(|v| v.as_str()),
            ) {
                xml_parts.push("<skill>".to_string());
                xml_parts.push(format!("<name>{}</name>", name));
                xml_parts.push(format!("<description>{}</description>", description));
                xml_parts.push(format!("<file>{}</file>", file_path));
                xml_parts.push("</skill>".to_string());
            }
        }

        xml_parts.push("</skills>".to_string());
        xml_parts.join("\n")
    }
}

#[async_trait]
impl ContextProvider for SkillsContextProvider {
    fn provider_id(&self) -> &str {
        "skills"
    }

    fn priority(&self) -> i32 {
        10 // Early in prompt - documentation reference
    }

    async fn get_context(&self) -> Result<String, String> {
        // Get skills directory from settings
        let skills_dir = self.get_skills_directory().await?;

        log::debug!("Building skills context from directory: {}", &skills_dir);

        // Scan skills directory
        let skills = scan_skills_directory(skills_dir.clone())
            .await
            .map_err(|e| format!("Failed to scan skills directory: {}", e))?;

        let skill_count = skills.len();

        if skill_count == 0 {
            log::debug!("No skills found in directory: {}", skills_dir);
            return Ok(String::new());
        }

        log::info!(
            "Building skills context with {} skills from {}",
            skill_count,
            skills_dir
        );

        // Convert SkillMetadata to JSON values for XML building
        let skills_json: Vec<serde_json::Value> = skills
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "filePath": s.path,
                })
            })
            .collect();

        // Build XML
        let xml = self.build_skills_xml(skills_json);

        Ok(xml)
    }

    async fn is_enabled(&self) -> bool {
        // Skills are always enabled if directory is configured
        // Could add a feature flag here if needed
        true
    }
}

impl Default for SkillsContextProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_skills_xml_empty() {
        let provider = SkillsContextProvider::new();
        let xml = provider.build_skills_xml(vec![]);
        assert_eq!(xml, "");
    }

    #[test]
    fn test_build_skills_xml_single() {
        let provider = SkillsContextProvider::new();

        let skills = vec![json!({
            "name": "test-skill",
            "description": "A test skill",
            "filePath": "/path/to/skill.md"
        })];

        let xml = provider.build_skills_xml(skills);

        assert!(xml.contains("<skills>"));
        assert!(xml.contains("<name>test-skill</name>"));
        assert!(xml.contains("<description>A test skill</description>"));
        assert!(xml.contains("<file>/path/to/skill.md</file>"));
        assert!(xml.contains("</skills>"));
    }
}
