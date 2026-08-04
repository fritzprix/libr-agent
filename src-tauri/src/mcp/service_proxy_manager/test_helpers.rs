use super::management::ProxyReadinessEntry;
use super::MCPServiceProxyManager;
use std::sync::Arc;

#[cfg(test)]
impl MCPServiceProxyManager {
    /// Inject a pending (false) readiness entry for a session.
    ///
    /// Returns the `Arc<Sender>` so the test can fire the signal manually,
    /// allowing tests to verify `wait_until_proxy_ready` blocks and then unblocks.
    pub(crate) async fn inject_pending_readiness_for_test(
        &self,
        session_id: &str,
    ) -> Arc<tokio::sync::watch::Sender<bool>> {
        let (tx, _rx) = tokio::sync::watch::channel(false);
        let tx = Arc::new(tx);
        self.proxy_readiness.write().await.insert(
            session_id.to_string(),
            ProxyReadinessEntry {
                ready_tx: tx.clone(),
                app_handle: None,
            },
        );
        tx
    }

    /// Return the current number of entries in the proxy_readiness map.
    pub(crate) async fn readiness_entry_count(&self) -> usize {
        self.proxy_readiness.read().await.len()
    }

    /// Whether a session still has a stdio manager entry (test probe for destroy hygiene).
    pub(crate) async fn has_stdio_manager_for_test(&self, session_id: &str) -> bool {
        self.session_stdio_managers
            .read()
            .await
            .contains_key(session_id)
    }

    /// Whether a session still has an HTTP manager entry (test probe for destroy hygiene).
    pub(crate) async fn has_http_manager_for_test(&self, session_id: &str) -> bool {
        self.session_http_managers
            .read()
            .await
            .contains_key(session_id)
    }

    /// Whether a per-session creation guard remains registered.
    pub(crate) async fn has_creation_guard_for_test(&self, session_id: &str) -> bool {
        self.creation_guards.lock().await.contains_key(session_id)
    }
}
