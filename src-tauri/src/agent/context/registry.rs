// Context Provider Registry
// Manages and combines multiple context providers into system prompt

use super::ContextProvider;

/// Registry for managing context providers
///
/// Example usage:
/// ```
/// let mut registry = ContextRegistry::new();
/// registry.register(Box::new(SkillsContextProvider::new(settings)));
/// registry.register(Box::new(TimeContextProvider::new()));
///
/// let context = registry.build_context().await;
/// ```
pub struct ContextRegistry {
    providers: Vec<Box<dyn ContextProvider>>,
}

impl std::fmt::Debug for ContextRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextRegistry")
            .field("provider_count", &self.providers.len())
            .finish()
    }
}

impl ContextRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a context provider
    pub fn register(&mut self, provider: Box<dyn ContextProvider>) {
        self.providers.push(provider);
    }

    /// Build combined context from all enabled providers
    /// Providers are sorted by priority (lower first)
    pub async fn build_context(&self) -> String {
        let mut sections = Vec::new();

        // Collect provider references with priorities
        let mut providers_with_priority: Vec<(i32, &Box<dyn ContextProvider>)> =
            self.providers.iter().map(|p| (p.priority(), p)).collect();

        // Sort by priority (lower = earlier)
        providers_with_priority.sort_by_key(|(priority, _)| *priority);

        // Build context from enabled providers
        for (_priority, provider) in providers_with_priority {
            if provider.is_enabled().await {
                match provider.get_context().await {
                    Ok(context) => {
                        if !context.is_empty() {
                            log::debug!(
                                "Context provider '{}' contributed {} characters",
                                provider.provider_id(),
                                context.len()
                            );
                            sections.push(context);
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Context provider '{}' failed: {}",
                            provider.provider_id(),
                            e
                        );
                    }
                }
            } else {
                log::debug!("Context provider '{}' is disabled", provider.provider_id());
            }
        }

        sections.join("\n\n")
    }

    /// Get count of registered providers
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for ContextRegistry {
    fn default() -> Self {
        Self::new()
    }
}
