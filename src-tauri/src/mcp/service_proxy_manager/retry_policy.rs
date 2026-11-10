//! Limits automatic retries of failed MCP servers.
//!
//! Re-entering a session retries its `Failed` servers. Without a limit, a failure that does not
//! recover (for example, a session image that cannot be pulled) restarts the server on every
//! trigger. Each server gets an increasing delay between automatic retries and a maximum number
//! of them. A manual retry or a new proxy (a config change) starts again from zero.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Automatic retries allowed after the first failure.
pub(super) const MAX_AUTO_RETRIES: u32 = 5;
const BASE_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RetryTrigger {
    /// Session re-entry. Follows the backoff and the limit.
    Automatic,
    /// The user asked for a retry. Starts the failure count again from zero.
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailureKind {
    /// A later attempt can succeed (crash, timeout, network error).
    Transient,
    /// Every attempt fails the same way until the user changes something (for example, the
    /// session container cannot be prepared). Automatic retries stop.
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServerRetryState {
    failures: u32,
    next_retry_at: Instant,
    permanent: bool,
}

impl ServerRetryState {
    fn can_auto_retry(&self, now: Instant) -> bool {
        !self.permanent && self.failures <= MAX_AUTO_RETRIES && now >= self.next_retry_at
    }
}

/// Delay before the next automatic retry after `failures` failures (1-based).
pub(super) fn retry_delay(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(16);
    BASE_RETRY_DELAY
        .saturating_mul(1 << doublings)
        .min(MAX_RETRY_DELAY)
}

/// Retry state per session, then per server name.
#[derive(Debug, Clone, Default)]
pub(super) struct RetryStateStore {
    inner: Arc<RwLock<HashMap<String, HashMap<String, ServerRetryState>>>>,
}

impl RetryStateStore {
    pub(super) async fn record_failure(
        &self,
        session_id: &str,
        server_name: &str,
        kind: FailureKind,
        now: Instant,
    ) {
        let mut sessions = self.inner.write().await;
        let servers = sessions.entry(session_id.to_string()).or_default();
        let failures = servers.get(server_name).map_or(0, |state| state.failures) + 1;
        let permanent = kind == FailureKind::Permanent
            || servers
                .get(server_name)
                .is_some_and(|state| state.permanent);
        servers.insert(
            server_name.to_string(),
            ServerRetryState {
                failures,
                next_retry_at: now + retry_delay(failures),
                permanent,
            },
        );
    }

    /// Forgets the failures of a server: it connected, or the user asked for a retry.
    pub(super) async fn clear_server(&self, session_id: &str, server_name: &str) {
        let mut sessions = self.inner.write().await;
        if let Some(servers) = sessions.get_mut(session_id) {
            servers.remove(server_name);
            if servers.is_empty() {
                sessions.remove(session_id);
            }
        }
    }

    pub(super) async fn clear_session(&self, session_id: &str) {
        self.inner.write().await.remove(session_id);
    }

    /// Names (of `server_names`) that an automatic retry may start at `now`. A server with no
    /// recorded failure may always start.
    pub(super) async fn auto_retry_allowed(
        &self,
        session_id: &str,
        server_names: &[String],
        now: Instant,
    ) -> Vec<String> {
        let sessions = self.inner.read().await;
        let servers = sessions.get(session_id);
        server_names
            .iter()
            .filter(|name| {
                servers
                    .and_then(|servers| servers.get(name.as_str()))
                    .is_none_or(|state| state.can_auto_retry(now))
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn delay_doubles_after_each_failure_up_to_the_cap() {
        assert_eq!(retry_delay(1), Duration::from_secs(5));
        assert_eq!(retry_delay(2), Duration::from_secs(10));
        assert_eq!(retry_delay(3), Duration::from_secs(20));
        assert_eq!(retry_delay(4), Duration::from_secs(40));
        assert_eq!(retry_delay(7), MAX_RETRY_DELAY);
        assert_eq!(retry_delay(u32::MAX), MAX_RETRY_DELAY);
    }

    #[tokio::test]
    async fn a_server_with_no_failure_may_retry() {
        let store = RetryStateStore::default();
        let allowed = store
            .auto_retry_allowed("s", &names(&["a"]), Instant::now())
            .await;
        assert_eq!(allowed, names(&["a"]));
    }

    #[tokio::test]
    async fn retry_waits_for_the_delay_and_the_delay_increases() {
        let store = RetryStateStore::default();
        let start = Instant::now();
        let server = names(&["a"]);

        store
            .record_failure("s", "a", FailureKind::Transient, start)
            .await;
        assert!(store
            .auto_retry_allowed("s", &server, start)
            .await
            .is_empty());
        let after_first = start + retry_delay(1);
        assert_eq!(
            store.auto_retry_allowed("s", &server, after_first).await,
            server
        );

        store
            .record_failure("s", "a", FailureKind::Transient, after_first)
            .await;
        assert!(store
            .auto_retry_allowed("s", &server, after_first + retry_delay(1))
            .await
            .is_empty());
        assert_eq!(
            store
                .auto_retry_allowed("s", &server, after_first + retry_delay(2))
                .await,
            server
        );
    }

    #[tokio::test]
    async fn retries_stop_at_the_limit() {
        let store = RetryStateStore::default();
        let server = names(&["a"]);
        let mut now = Instant::now();

        // The first failure, then MAX_AUTO_RETRIES failed retries.
        for failures in 1..=MAX_AUTO_RETRIES + 1 {
            store
                .record_failure("s", "a", FailureKind::Transient, now)
                .await;
            now += retry_delay(failures);
        }
        assert!(store.auto_retry_allowed("s", &server, now).await.is_empty());
        assert!(store
            .auto_retry_allowed("s", &server, now + Duration::from_secs(86_400))
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn a_permanent_failure_is_not_retried_automatically() {
        let store = RetryStateStore::default();
        let now = Instant::now();
        store
            .record_failure("s", "a", FailureKind::Permanent, now)
            .await;
        assert!(store
            .auto_retry_allowed("s", &names(&["a"]), now + MAX_RETRY_DELAY)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn clearing_a_server_or_session_allows_a_retry_again() {
        let store = RetryStateStore::default();
        let now = Instant::now();
        let both = names(&["a", "b"]);
        store
            .record_failure("s", "a", FailureKind::Permanent, now)
            .await;
        store
            .record_failure("s", "b", FailureKind::Permanent, now)
            .await;

        store.clear_server("s", "a").await;
        assert_eq!(
            store.auto_retry_allowed("s", &both, now).await,
            names(&["a"])
        );

        store.clear_session("s").await;
        assert_eq!(store.auto_retry_allowed("s", &both, now).await, both);
    }
}
