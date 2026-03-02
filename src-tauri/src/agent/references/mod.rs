use async_trait::async_trait;
use regex::Regex;

mod file;
mod skill;

pub use file::{list_workspace_relative_paths, FileReferenceResolver};
pub use skill::SkillReferenceResolver;

/// Resolves a single `@type:arg` reference to its injectable text content.
#[async_trait]
pub trait ReferenceResolver: Send + Sync {
    /// The token type this resolver handles (e.g. "skill", "file").
    fn type_name(&self) -> &'static str;

    /// Attempt to resolve the given argument to injectable text.
    /// Returns `None` if the reference cannot be found (silently skipped).
    async fn resolve(&self, arg: &str) -> Option<String>;
}

/// Registry of all active reference resolvers.
/// Parses `@type:arg` tokens from user message text and prepends resolved content.
pub struct ReferenceRegistry {
    resolvers: Vec<Box<dyn ReferenceResolver>>,
}

impl ReferenceRegistry {
    pub fn new() -> Self {
        Self { resolvers: vec![] }
    }

    /// Register a new resolver. Call once at startup.
    pub fn register(&mut self, resolver: Box<dyn ReferenceResolver>) {
        self.resolvers.push(resolver);
    }

    /// Parse all `@type:arg` tokens from `text`, resolve each, and return
    /// the message text with resolved content prepended.
    /// References that cannot be resolved are silently skipped.
    /// Returns the original `text` unchanged if no references exist.
    pub async fn preprocess_message_text(&self, text: &str) -> String {
        // Matches @word:non-whitespace tokens anywhere in the text
        let re = match Regex::new(r"@([\w]+):([\S]+)") {
            Ok(r) => r,
            Err(_) => return text.to_string(),
        };

        let mut prefix_parts: Vec<String> = Vec::new();
        let mut unresolved_tokens: Vec<String> = Vec::new();

        for cap in re.captures_iter(text) {
            let type_name = &cap[1];
            let arg = &cap[2];
            let token = cap[0].to_string();

            let mut resolved = false;
            for resolver in &self.resolvers {
                if resolver.type_name() == type_name {
                    if let Some(content) = resolver.resolve(arg).await {
                        prefix_parts.push(format!(
                            "## Reference: @{}:{}\n\n{}",
                            type_name, arg, content
                        ));
                        resolved = true;
                    }
                    break;
                }
            }

            // Replace unresolved tokens inline so the AI knows the reference failed
            if !resolved {
                unresolved_tokens.push(token);
            }
        }

        // Build inline notice for unresolved references
        if !unresolved_tokens.is_empty() {
            let notice = unresolved_tokens
                .iter()
                .map(|t| format!("`{}` (not found)", t))
                .collect::<Vec<_>>()
                .join(", ");
            prefix_parts.push(format!(
                "⚠️ The following references could not be resolved and their content was NOT injected: {}",
                notice
            ));
        }

        if prefix_parts.is_empty() {
            return text.to_string();
        }

        format!("{}\n\n---\n\n{}", prefix_parts.join("\n\n---\n\n"), text)
    }
}

impl Default for ReferenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default registry with all built-in resolvers pre-registered.
/// Requires `session_id` so file resolver can access the correct workspace.
pub async fn build_default_registry(session_id: &str) -> ReferenceRegistry {
    let mut registry = ReferenceRegistry::new();
    registry.register(Box::new(SkillReferenceResolver));
    registry.register(Box::new(FileReferenceResolver::new(session_id)));
    registry
}
