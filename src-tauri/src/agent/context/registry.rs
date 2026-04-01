// Context Provider Registry
// Manages and combines multiple context providers into system prompt

use super::ContextProvider;

/// Registry for managing context providers
///
/// Providers are stored with priority (lower = higher priority).
/// System prompts are built by calling each provider in priority order.
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
    /// Providers are sorted by priority (lower first) and queried in parallel.
    ///
    /// # Arguments
    /// * `assistant_id` - Optional assistant ID to pass to context providers
    pub async fn build_context(&self, assistant_id: Option<&str>) -> String {
        // Collect provider references with priorities and sort (lower = earlier)
        let mut providers_with_priority: Vec<(i32, &Box<dyn ContextProvider>)> =
            self.providers.iter().map(|p| (p.priority(), p)).collect();
        providers_with_priority.sort_by_key(|(priority, _)| *priority);

        // Fan-out: query all enabled providers in parallel
        let futures: Vec<_> = providers_with_priority
            .iter()
            .map(|(_priority, provider)| async move {
                if !provider.is_enabled().await {
                    log::debug!("Context provider '{}' is disabled", provider.provider_id());
                    return None;
                }
                match provider.get_context(assistant_id).await {
                    Ok(context) if !context.is_empty() => {
                        log::debug!(
                            "Context provider '{}' contributed {} characters",
                            provider.provider_id(),
                            context.len()
                        );
                        Some(context)
                    }
                    Ok(_) => None,
                    Err(e) => {
                        log::warn!(
                            "Context provider '{}' failed: {}",
                            provider.provider_id(),
                            e
                        );
                        None
                    }
                }
            })
            .collect();

        let sections: Vec<String> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

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
