use super::ReferenceResolver;
use crate::mcp::builtin::playbook::{format_playbook_detailed, Playbook};
use crate::repositories::PlaybookRepository;
use async_trait::async_trait;
use tracing::warn;

/// Resolves `@playbook:goal` references by looking up a playbook whose goal
/// matches `arg` (case-insensitive, partial match) within the assistant scope.
///
/// The resolved content includes the full playbook details so the agent can
/// immediately begin executing the workflow without a separate tool call.
pub struct PlaybookReferenceResolver {
    assistant_id: String,
}

impl PlaybookReferenceResolver {
    pub fn new(assistant_id: impl Into<String>) -> Self {
        Self {
            assistant_id: assistant_id.into(),
        }
    }
}

#[async_trait]
impl ReferenceResolver for PlaybookReferenceResolver {
    fn type_name(&self) -> &'static str {
        "playbook"
    }

    /// Search for a playbook whose goal contains `arg`, then return its full
    /// details as text so the agent can execute it.
    async fn resolve(&self, arg: &str) -> Option<String> {
        let repo = crate::state::get_playbook_repository();

        let models = repo
            .search_playbooks(&self.assistant_id, arg)
            .await
            .map_err(|e| warn!("PlaybookResolver: DB error: {}", e))
            .ok()?;

        // Prefer exact goal match first, then take the most recent result
        let model = models
            .iter()
            .find(|m| m.goal.eq_ignore_ascii_case(arg))
            .or_else(|| models.first())?;

        let playbook = Playbook::from_model(model);
        let details = format_playbook_detailed(&playbook);

        Some(format!(
            "Playbook: \"{}\"\n\n{}\n\nYou have been given this playbook as context. Review the workflow steps and success criteria, establish todos based on the steps, and begin execution.",
            playbook.goal, details
        ))
    }
}
