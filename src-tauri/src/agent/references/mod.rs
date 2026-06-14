use async_trait::async_trait;
use regex::Regex;
use std::collections::HashSet;

mod file;
mod playbook;
mod skill;

pub use file::{list_relative_paths_in_root, list_workspace_relative_paths, FileReferenceResolver};
pub use playbook::PlaybookReferenceResolver;
pub use skill::SkillReferenceResolver;

fn reference_dedupe_key(type_name: &str, arg: &str) -> String {
    if type_name.eq_ignore_ascii_case("skill") {
        format!("{type_name}:{}", arg.to_ascii_lowercase())
    } else {
        format!("{type_name}:{arg}")
    }
}

const MULTIPLE_SKILLS_REFERENCE_PREAMBLE: &str = "**Multiple skills referenced:** Read all \
instruction files listed below before calling any other tool or taking action on the user's \
message. Apply each skill only where it fits the task.";

/// Resolves a single `@type:arg` reference to its injectable text content.
#[async_trait]
pub trait ReferenceResolver: Send + Sync {
    /// The token type this resolver handles (e.g. "skill", "file").
    fn type_name(&self) -> &'static str;

    /// Attempt to resolve the given argument to injectable text.
    /// Returns `None` if the reference cannot be found (silently skipped).
    async fn resolve(&self, arg: &str) -> Option<String>;

    /// When `true`, resolved content is appended after the original user text.
    /// When `false` (default), resolved content is prepended before the user text.
    fn append_after_user_text(&self) -> bool {
        false
    }
}

/// Registry of all active reference resolvers.
/// Parses `@type:arg` tokens from user message text and injects resolved content.
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

    /// Parse all `@type:arg` tokens from `text`, resolve each, and return the message
    /// text with resolved reference blocks injected.
    ///
    /// Most resolvers prepend content before the user text. Skill references append
    /// metadata and read guidance after the user text so `@skill:name ...` sentences
    /// stay intact at the top of the payload.
    /// Returns the original `text` unchanged if no references exist.
    pub async fn preprocess_message_text(&self, text: &str) -> String {
        // Matches @word:non-whitespace tokens anywhere in the text
        let re = match Regex::new(r"@([\w]+):([\S]+)") {
            Ok(r) => r,
            Err(_) => return text.to_string(),
        };

        let mut prefix_parts: Vec<String> = Vec::new();
        // Blocks from resolvers with `append_after_user_text()` (skill today; others stay prefix).
        let mut skill_suffix_parts: Vec<String> = Vec::new();
        let mut other_suffix_parts: Vec<String> = Vec::new();
        let mut seen_resolved_keys: HashSet<String> = HashSet::new();
        let mut seen_unresolved_tokens: HashSet<String> = HashSet::new();
        let mut unresolved_tokens: Vec<String> = Vec::new();

        for cap in re.captures_iter(text) {
            let type_name = &cap[1];
            let arg = &cap[2];
            let token = cap[0].to_string();

            let mut resolved = false;
            for resolver in &self.resolvers {
                if resolver.type_name() == type_name {
                    if let Some(content) = resolver.resolve(arg).await {
                        let dedupe_key = reference_dedupe_key(type_name, arg);
                        if !seen_resolved_keys.insert(dedupe_key) {
                            resolved = true;
                            break;
                        }

                        let block = format!("## Reference: @{}:{}\n\n{}", type_name, arg, content);
                        if resolver.append_after_user_text() {
                            skill_suffix_parts.push(block);
                        } else {
                            prefix_parts.push(block);
                        }
                        resolved = true;
                    }
                    break;
                }
            }

            if !resolved && seen_unresolved_tokens.insert(token.clone()) {
                unresolved_tokens.push(token);
            }
        }

        if !unresolved_tokens.is_empty() {
            let notice = unresolved_tokens
                .iter()
                .map(|t| format!("`{}` (not found)", t))
                .collect::<Vec<_>>()
                .join(", ");
            other_suffix_parts.push(format!(
                "⚠️ The following references could not be resolved and their content was NOT injected: {}",
                notice
            ));
        }

        let mut suffix_parts: Vec<String> = Vec::new();
        if skill_suffix_parts.len() > 1 {
            suffix_parts.push(MULTIPLE_SKILLS_REFERENCE_PREAMBLE.to_string());
        }
        suffix_parts.extend(skill_suffix_parts);
        suffix_parts.extend(other_suffix_parts);

        if prefix_parts.is_empty() && suffix_parts.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();

        if !prefix_parts.is_empty() {
            result = format!("{}\n\n---\n\n{}", prefix_parts.join("\n\n---\n\n"), result);
        }

        if !suffix_parts.is_empty() {
            result = format!("{}\n\n---\n\n{}", result, suffix_parts.join("\n\n---\n\n"));
        }

        result
    }
}

impl Default for ReferenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default registry with all built-in resolvers pre-registered.
/// Requires `session_id` so file resolver can access the correct workspace.
/// Requires `assistant_id` so playbook resolver can scope lookups to the assistant.
pub async fn build_default_registry(
    session_id: &str,
    assistant_id: Option<&str>,
) -> ReferenceRegistry {
    let mut registry = ReferenceRegistry::new();
    registry.register(Box::new(SkillReferenceResolver::new(
        session_id,
        assistant_id,
    )));
    registry.register(Box::new(FileReferenceResolver::new(session_id)));
    if let Some(aid) = assistant_id {
        if !aid.is_empty() {
            registry.register(Box::new(PlaybookReferenceResolver::new(aid)));
        }
    }
    registry
}
