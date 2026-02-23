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
        self.proxy_readiness
            .write()
            .await
            .insert(session_id.to_string(), tx.clone());
        tx
    }

    /// Return the current number of entries in the proxy_readiness map.
    pub(crate) async fn readiness_entry_count(&self) -> usize {
        self.proxy_readiness.read().await.len()
    }
}
