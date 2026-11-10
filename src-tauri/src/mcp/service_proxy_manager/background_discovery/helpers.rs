use super::super::retry_policy::FailureKind;
use super::super::runtime_updates::{apply_server_failed, update_runtime_state_store};
use super::types::{DiscoveryContext, DiscoveryFailure};
use crate::agent::runtime_state::SessionRuntimeTransport;
use std::future::Future;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

pub(super) async fn mark_server_failed(
    context: &DiscoveryContext,
    server_name: &str,
    transport: SessionRuntimeTransport,
    error: String,
    kind: FailureKind,
) {
    context
        .retry_states
        .record_failure(&context.session_id, server_name, kind, Instant::now())
        .await;
    let update_result = update_runtime_state_store(
        &context.runtime_states,
        &context.session_id,
        context.app_handle.as_ref(),
        |state| {
            apply_server_failed(state, server_name, transport, error.clone());
        },
    )
    .await;
    if update_result.emitted {
        context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) fn log_background_discovery_metrics(
    session_id: &str,
    stdio_servers: usize,
    http_servers: usize,
    runtime_state_emits: u32,
    total_ms: u128,
    stdio_ms: u128,
    http_ms: u128,
) {
    log::info!(
        "proxy_discovery_metrics session_id={} stdio_servers={} http_servers={} runtime_state_emits={} total_ms={} stdio_ms={} http_ms={}",
        session_id,
        stdio_servers,
        http_servers,
        runtime_state_emits,
        total_ms,
        stdio_ms,
        http_ms
    );
}

pub(super) async fn await_tool_discovery<T, E, F>(
    future: F,
    timeout: Duration,
    transport: &str,
    server_name: &str,
    session_id: &str,
    failure_kind: impl Fn(&E) -> FailureKind,
) -> Result<T, DiscoveryFailure>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(DiscoveryFailure {
            message: format!(
                "{} server '{}' tool discovery failed for session '{}': {}",
                transport, server_name, session_id, error
            ),
            kind: failure_kind(&error),
        }),
        Err(_) => Err(DiscoveryFailure {
            message: format!(
                "{} server '{}' tool discovery timed out after {}s for session '{}'",
                transport,
                server_name,
                timeout.as_secs(),
                session_id
            ),
            kind: FailureKind::Transient,
        }),
    }
}
