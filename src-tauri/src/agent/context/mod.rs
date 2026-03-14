// Context Provider Framework
// Provides read-only information to system prompts without tool execution

use async_trait::async_trait;

pub mod registry;
pub mod time_location;

/// Trait for providers that inject read-only context into system prompts
///
/// Unlike `BuiltinMCPServer` which provides tools + stateful context,
/// `ContextProvider` is for information-only providers (skills, time, preferences, etc.)
#[async_trait]
pub trait ContextProvider: Send + Sync {
    /// Unique identifier for this provider
    fn provider_id(&self) -> &str;

    /// Build context section to inject into system prompt
    /// Returns XML, JSON, or plain text format
    ///
    /// # Arguments
    /// * `assistant_id` - Optional assistant ID for assistant-specific context
    async fn get_context(&self, assistant_id: Option<&str>) -> Result<String, String>;

    /// Whether this provider is currently enabled
    /// Can check settings, feature flags, etc.
    async fn is_enabled(&self) -> bool {
        true
    }

    /// Priority order in system prompt (lower number = earlier in prompt)
    /// Default: 100
    /// Suggested ranges:
    /// - 1-10: Critical context (time, user identity)
    /// - 10-50: Documentation (skills, guides)
    /// - 50-100: Preferences and optional context
    fn priority(&self) -> i32 {
        100
    }
}
