//! Proxy readiness state machine: the pure decision, the blocking wait, and the
//! timeout finalize that opens Session Ready for waiters.

use super::super::runtime_updates::{apply_discovery_timeout_finalize, update_runtime_state_store};
use super::MCPServiceProxyManager;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyReadinessState {
    MissingProxy,
    Ready,
    AwaitSignal,
}

/// Per-session readiness watch + optional app handle for timeout finalize emits.
#[derive(Clone)]
pub struct ProxyReadinessEntry {
    pub ready_tx: Arc<tokio::sync::watch::Sender<bool>>,
    pub app_handle: Option<AppHandle>,
}

enum WaitOutcome {
    Ready,
    WatchError(String),
    TimedOut,
}

pub fn decide_proxy_readiness_state(
    proxy_exists: bool,
    has_readiness_signal: bool,
) -> ProxyReadinessState {
    if !proxy_exists {
        ProxyReadinessState::MissingProxy
    } else if has_readiness_signal {
        ProxyReadinessState::AwaitSignal
    } else {
        ProxyReadinessState::Ready
    }
}

impl MCPServiceProxyManager {
    /// Wait for background tool loading to complete for a session.
    ///
    /// Sessions backed only by builtin tools are immediately ready once the proxy exists
    /// (no entry in the map). Sessions with external stdio/HTTP servers signal readiness
    /// via a `watch::channel`.
    ///
    /// # Arguments
    /// * `session_id` - The session to wait for
    /// * `timeout_secs` - Maximum seconds to wait before returning an error
    ///
    /// # Returns
    /// * `Ok(())` - Tools are loaded (or no external servers)
    /// * `Err(String)` - Timeout or channel error
    pub async fn wait_until_proxy_ready(
        &self,
        session_id: &str,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let readiness_signal = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).cloned()
        };

        match decide_proxy_readiness_state(
            self.get_proxy(session_id).await.is_some(),
            readiness_signal.is_some(),
        ) {
            ProxyReadinessState::MissingProxy => {
                return Err(format!("No MCP proxy exists for session: {}", session_id));
            }
            ProxyReadinessState::Ready => {
                return Ok(());
            }
            ProxyReadinessState::AwaitSignal => {}
        }

        let mut rx = readiness_signal
            .expect("readiness signal must exist when state requires waiting")
            .ready_tx
            .subscribe();

        if *rx.borrow() {
            return Ok(()); // Already signaled true
        }

        // Convert away from `watch::Ref` before any further `.await` — that guard
        // is !Send and must not be held across awaits inside tokio::spawn tasks.
        let wait_outcome = match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            rx.wait_for(|v| *v),
        )
        .await
        {
            Ok(Ok(ready)) => {
                drop(ready);
                WaitOutcome::Ready
            }
            Ok(Err(e)) => WaitOutcome::WatchError(e.to_string()),
            Err(_) => WaitOutcome::TimedOut,
        };

        match wait_outcome {
            WaitOutcome::Ready => Ok(()),
            WaitOutcome::WatchError(e) => {
                // Discovery may still be waiting on a slow/failed stdio server while HTTP
                // (and builtin) tools are already usable. Finalize Session Ready (TimedOut
                // pending servers) instead of only clearing the wait map.
                if self.get_proxy(session_id).await.is_some() {
                    log::warn!(
                        "Proxy readiness watch failed for session {}: {}; finalizing with currently available tools",
                        session_id,
                        e
                    );
                    self.finalize_discovery_for_wait_timeout(
                        session_id,
                        &format!("Proxy readiness watch failed: {e}"),
                    )
                    .await;
                    return Ok(());
                }
                Err(format!("Proxy readiness watch error: {}", e))
            }
            WaitOutcome::TimedOut => {
                if self.get_proxy(session_id).await.is_some() {
                    log::warn!(
                        "Proxy tool loading timed out after {}s for session: {}; finalizing with currently available tools",
                        timeout_secs,
                        session_id
                    );
                    self.finalize_discovery_for_wait_timeout(
                        session_id,
                        &format!(
                            "Tool discovery timed out after {timeout_secs}s waiting for proxy readiness"
                        ),
                    )
                    .await;
                    return Ok(());
                }
                Err(format!(
                    "Proxy tool loading timed out after {}s for session: {}",
                    timeout_secs, session_id
                ))
            }
        }
    }

    /// Mark pending MCP servers TimedOut, flip Session Ready, wake waiters.
    /// Idempotent when initialization already left `pending`.
    pub(super) async fn finalize_discovery_for_wait_timeout(&self, session_id: &str, reason: &str) {
        let entry = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).cloned()
        };
        let app_handle = entry.as_ref().and_then(|e| e.app_handle.clone());

        let update_result = update_runtime_state_store(
            &self.runtime_states,
            session_id,
            app_handle.as_ref(),
            |state| {
                let applied = apply_discovery_timeout_finalize(state, reason);
                // Idempotent TimedOut skip must still open Session Ready for waiters
                // (e.g. empty external set already Success but ready was cleared).
                if !applied && state.proxy.exists {
                    state.proxy.ready = true;
                }
            },
        )
        .await;

        if let Some(entry) = entry {
            entry.ready_tx.send_replace(true);
        }
        self.proxy_readiness.write().await.remove(session_id);

        if update_result.changed {
            log::info!(
                "Finalized MCP discovery on wait timeout for session {} (emitted={})",
                session_id,
                update_result.emitted
            );
        }
    }
}
