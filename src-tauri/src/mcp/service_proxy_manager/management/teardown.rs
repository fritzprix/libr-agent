//! Force-stop teardown: the teardown-fence lifecycle, session resource cleanup,
//! and the foreground-process cancellation markers.

use super::MCPServiceProxyManager;

pub async fn shutdown_stdio_manager_with_timeout(
    stdio_mgr: crate::mcp::session_isolation::SessionMCPManager,
    session_id: &str,
    context: &str,
) {
    match tokio::time::timeout(std::time::Duration::from_secs(3), stdio_mgr.shutdown_all()).await {
        Ok(_) => {
            log::debug!(
                "Successfully shut down stdio processes for session: {} ({})",
                session_id,
                context
            );
        }
        Err(_) => {
            log::warn!(
                "Timeout waiting for stdio processes shutdown in {} for session: {}; continuing",
                context,
                session_id
            );
        }
    }
}

impl MCPServiceProxyManager {
    /// Cancel resources owned by a session without destroying its proxy.
    ///
    /// Soft workflow cancellation keeps the proxy alive so the session can be
    /// resumed, therefore resource cleanup must be routed through the existing
    /// session-bound builtin server instances.
    pub async fn kill_session_processes(&self, session_id: &str) -> Result<usize, String> {
        let Some(proxy) = self.get_proxy(session_id).await else {
            log::debug!(
                "No MCP proxy found while cancelling resources for session {}",
                session_id
            );
            return Ok(0);
        };

        proxy.kill_session_processes().await
    }

    /// Force-stop cleanup for explicit terminate/delete (issue #2633).
    ///
    /// Fences the session against late tool dispatches, marks its builtin
    /// servers terminated so no new session-owned resource can be created, and
    /// kills the resources that already exist — all while the proxy is still
    /// alive, so ownership is available until cleanup completes. Idempotent;
    /// safe to call when no proxy exists. Returns the killed resource count.
    pub async fn terminate_session_resources(&self, session_id: &str) -> usize {
        self.move_teardown_fence(session_id, true).await;

        let Some(proxy) = self.get_proxy(session_id).await else {
            log::debug!(
                "No MCP proxy found while force-stopping resources for session {}",
                session_id
            );
            return 0;
        };

        // Mark before kill: a dispatch that inserted its registry entry after
        // the mark is caught by the per-server guard, and one that raced ahead
        // of the mark is caught by the sweep below (or its late-pid killer).
        proxy.mark_session_terminated();

        let report = proxy.force_stop_session_resources().await;
        if !report.failures.is_empty() {
            // The sweep reports partial cleanup honestly: the killed count is
            // returned even when some servers failed (issue #2633 S2.1).
            log::warn!(
                "Partial force-stop cleanup for session {}: {} resource(s) killed; failures: {}",
                session_id,
                report.killed,
                report.failures.join("; ")
            );
        }
        report.killed
    }

    /// Move the teardown fence for a session and bump its generation.
    ///
    /// Both setting and clearing bump the generation so a `call_tool` that
    /// captured the generation at entry detects the fence moving underneath
    /// it even when the fence was cleared again before its dispatch
    /// (issue #2633 G2.1). The entry is kept so generations stay monotonic.
    async fn move_teardown_fence(&self, session_id: &str, fenced: bool) {
        let mut fences = self.teardown_fenced.lock().await;
        let state = fences.entry(session_id.to_string()).or_default();
        state.generation += 1;
        state.fenced = fenced;
    }

    /// Whether a session is currently fenced by an explicit force-stop.
    pub async fn is_teardown_fenced(&self, session_id: &str) -> bool {
        self.teardown_fenced
            .lock()
            .await
            .get(session_id)
            .is_some_and(|state| state.fenced)
    }

    /// Snapshot the session's fence generation (`0` when never fenced).
    ///
    /// `call_tool` captures this at entry and compares it before dispatch via
    /// [`Self::reject_if_teardown_fence_moved`] (issue #2633 G2.1).
    pub async fn teardown_fence_generation(&self, session_id: &str) -> u64 {
        self.teardown_fenced
            .lock()
            .await
            .get(session_id)
            .map(|state| state.generation)
            .unwrap_or(0)
    }

    /// Reject an operation on a force-terminated session (issue #2633).
    ///
    /// Used by proxy creation/ensure paths so neither a late tool dispatch nor
    /// an automatic wake can lazily recreate builtin instances after teardown.
    pub async fn reject_if_teardown_fenced(&self, session_id: &str) -> Result<(), String> {
        if self.is_teardown_fenced(session_id).await {
            return Err(format!(
                "{} Session '{}' was terminated; its tool session is closed. Start a new workflow (or resume) before calling tools.",
                Self::TEARDOWN_FENCE_REJECTION_MARKER, session_id
            ));
        }
        Ok(())
    }

    /// Reject when the session is fenced now, or when its fence generation
    /// moved since `generation` was captured at dispatch entry (issue #2633
    /// G2.1). A later clear therefore cannot validate an older dispatch.
    pub async fn reject_if_teardown_fence_moved(
        &self,
        session_id: &str,
        generation: u64,
    ) -> Result<(), String> {
        let fences = self.teardown_fenced.lock().await;
        if let Some(state) = fences.get(session_id) {
            if state.fenced {
                return Err(format!(
                    "{} Session '{}' was terminated; its tool session is closed. Start a new workflow (or resume) before calling tools.",
                    Self::TEARDOWN_FENCE_REJECTION_MARKER, session_id
                ));
            }
            if state.generation != generation {
                return Err(format!(
                    "{} Session '{}' was force-stopped and reopened while this tool call was resolving; the late dispatch was rejected. Send a new message if needed.",
                    Self::TEARDOWN_FENCE_REJECTION_MARKER, session_id
                ));
            }
        }
        Ok(())
    }

    /// Stable marker embedded in every force-stop fence rejection message so
    /// callers can tell a correct refusal apart from a real failure
    /// (issue #2633).
    pub const TEARDOWN_FENCE_REJECTION_MARKER: &'static str = "[force-stop]";

    /// Whether a tool/ensure error is a force-stop fence refusal rather than
    /// a real failure (issue #2633 G2.3: a correct refusal must not be
    /// reported as a session error).
    pub fn is_teardown_fence_rejection(error: &str) -> bool {
        error.contains(Self::TEARDOWN_FENCE_REJECTION_MARKER)
    }

    /// Clear the force-stop fence for a session.
    ///
    /// Called only at explicit user/API entry points (session create, the
    /// explicit resume command, a user or API message, UI message injection,
    /// session reload) so a later valid message can work again after an
    /// explicit terminate. Automatic wakes (MCP channel notifications,
    /// scheduled jobs, compaction continuation, background resume,
    /// agent-tool messages) must never call this. The generation is bumped so
    /// dispatches captured before the clear stay rejected (issue #2633 G2.1).
    pub async fn clear_teardown_fence(&self, session_id: &str) {
        self.move_teardown_fence(session_id, false).await;
    }

    /// Mark that a user cancel stopped a foreground process. The result of the
    /// running tool consumes this marker so the workflow can continue normally.
    pub async fn mark_process_cancel_pending(&self, session_id: &str) {
        self.process_cancel_pending
            .lock()
            .await
            .insert(session_id.to_string());
    }

    /// Consume the foreground-process cancellation marker for a session.
    pub async fn take_process_cancel_pending(&self, session_id: &str) -> bool {
        self.process_cancel_pending.lock().await.remove(session_id)
    }

    /// Clear the process-cancellation marker. Called before each tool runs and
    /// when a new workflow starts, so a stale cancel does not mark a later tool.
    pub async fn clear_process_cancel_pending(&self, session_id: &str) {
        self.process_cancel_pending.lock().await.remove(session_id);
    }

    /// Run one tool call and report whether a process cancel was marked while
    /// it ran. A marker left from before the call does not count.
    pub async fn run_with_process_cancel_scope<F, T>(&self, session_id: &str, tool: F) -> (T, bool)
    where
        F: std::future::Future<Output = T>,
    {
        self.clear_process_cancel_pending(session_id).await;
        let output = tool.await;
        let cancelled = self.take_process_cancel_pending(session_id).await;
        (output, cancelled)
    }
}
