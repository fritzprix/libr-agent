// Skills Context Provider
// Injects available skills documentation into system prompts

use super::ContextProvider;
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Process-level cache for skills XML (TTL: 60s)
/// Skills change rarely; rescanning every LLM turn is wasteful.
static SKILLS_CACHE: std::sync::OnceLock<Mutex<Option<(String, Instant)>>> =
    std::sync::OnceLock::new();

const SKILLS_CACHE_TTL: Duration = Duration::from_secs(60);

fn get_or_init_cache() -> &'static Mutex<Option<(String, Instant)>> {
    SKILLS_CACHE.get_or_init(|| Mutex::new(None))
}

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

    /// Get skills directory from settings, falling back to default [AppData]/skills
    async fn get_skills_directory(&self) -> Result<String, String> {
        // Reuse get_configured_skills_directory() which already has proper fallback:
        // settings.skillsDirectory → [AppData]/skills (if not configured)
        crate::commands::skill_commands::get_configured_skills_directory().await
    }

    /// Build skills XML from scanned skills directory
    fn build_skills_xml(&self, skills: Vec<serde_json::Value>) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut xml_parts = vec!["<available_skills>".to_string()];

        for skill in skills {
            if let (Some(name), Some(description), Some(file_path)) = (
                skill.get("name").and_then(|v| v.as_str()),
                skill.get("description").and_then(|v| v.as_str()),
                skill.get("filePath").and_then(|v| v.as_str()),
            ) {
                xml_parts.push("  <skill>".to_string());
                xml_parts.push(format!("    <name>{}</name>", name));
                xml_parts.push(format!("    <description>{}</description>", description));
                xml_parts.push(format!("    <location>{}</location>", file_path));
                xml_parts.push("  </skill>".to_string());
            }
        }

        xml_parts.push("</available_skills>".to_string());
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

    async fn get_context(&self, assistant_id: Option<&str>) -> Result<String, String> {
        // Check cache first (TTL: 60s) — skip for assistant-specific skills
        if assistant_id.is_none() {
            let cached = {
                let lock = get_or_init_cache().lock().map_err(|e| e.to_string())?;
                lock.as_ref()
                    .filter(|(_, ts)| ts.elapsed() < SKILLS_CACHE_TTL)
                    .map(|(xml, _)| xml.clone())
            };
            if let Some(xml) = cached {
                log::debug!("Skills context served from cache");
                return Ok(xml);
            }
        }

        // Get global skills directory from settings
        let global_skills_dir = self.get_skills_directory().await?;

        log::debug!(
            "Building skills context from directory: {}",
            &global_skills_dir
        );

        // Determine assistant skills directory if assistant_id is provided
        let assistant_skills_dir = if let Some(id) = assistant_id {
            match crate::session::get_session_manager() {
                Ok(manager) => Some(
                    manager
                        .get_base_data_dir()
                        .join("assistants")
                        .join(id)
                        .join("skills"),
                ),
                Err(e) => {
                    log::warn!("Failed to get session manager for assistant skills: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Use resolve_skills to get the correct skills (override-only logic)
        let skills = crate::services::skill_service::resolve_skills(
            std::path::PathBuf::from(global_skills_dir.clone()),
            assistant_skills_dir,
        )
        .await
        .map_err(|e| format!("Failed to resolve skills: {}", e))?;

        let skill_count = skills.len();

        if skill_count == 0 {
            log::debug!("No skills found");
            return Ok(String::new());
        }

        log::info!(
            "Building skills context with {} skills from {}",
            skill_count,
            global_skills_dir
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

        // Update cache (only for global skills, not assistant-specific)
        if assistant_id.is_none() {
            if let Ok(mut lock) = get_or_init_cache().lock() {
                *lock = Some((xml.clone(), Instant::now()));
            }
        }

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

        assert!(xml.contains("<available_skills>"));
        assert!(xml.contains("<name>test-skill</name>"));
        assert!(xml.contains("<description>A test skill</description>"));
        assert!(xml.contains("<location>/path/to/skill.md</location>"));
        assert!(xml.contains("</available_skills>"));
    }
}
