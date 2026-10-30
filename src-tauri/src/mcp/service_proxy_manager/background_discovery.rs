use super::super::service_proxy::MCPServiceProxy;
use super::super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::management::ProxyReadinessEntry;
use super::persist_tool_cache_for_server;
use super::retry_policy::{FailureKind, RetryStateStore};
use super::runtime_updates::{
    apply_discovery_task_failed, apply_discovery_timeout_finalize, apply_initialization_complete,
    apply_server_connecting, apply_server_discovering, apply_server_failed, apply_server_ready,
    update_runtime_state_store,
};
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::SessionRuntimeTransport;
use crate::mcp::session_isolation::SessionMCPError;
use crate::services::mcp_server_service::summarize_tool_names;
use futures::FutureExt;
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tauri::AppHandle;
use tokio::task::JoinSet;

type RuntimeStateStore =
    Arc<tokio::sync::RwLock<HashMap<String, crate::agent::runtime_state::SessionRuntimeState>>>;

#[derive(Clone)]
struct DiscoveryContext {
    session_id: String,
    app_handle: Option<AppHandle>,
    runtime_states: RuntimeStateStore,
    server_name_to_id: Arc<HashMap<String, String>>,
    /// How long the session waits before it carries on without pending servers.
    tool_discovery_timeout: Duration,
    /// Safety cap for one server; a server is only given up on after this (or on exit/error).
    server_discovery_cap: Duration,
    runtime_state_emits: Arc<AtomicU32>,
    retry_states: RetryStateStore,
}

/// Why tool discovery of one server failed, and whether a later attempt can succeed.
#[derive(Debug)]
struct DiscoveryFailure {
    message: String,
    kind: FailureKind,
}

fn stdio_failure_kind(error: &SessionMCPError) -> FailureKind {
    match error {
        SessionMCPError::ContainerUnavailable(_) => FailureKind::Permanent,
        _ => FailureKind::Transient,
    }
}

fn server_discovery_cap(session_wait: Duration) -> Duration {
    Duration::from_secs(crate::config::mcp_server_startup_cap_seconds(
        session_wait.as_secs(),
    ))
}

struct PanicDiscoveryResult {
    server_name: String,
    panic_error: Option<String>,
}

pub(super) struct BackgroundDiscoveryPlan {
    pub(super) session_id: String,
    pub(super) proxy: Arc<MCPServiceProxy>,
    pub(super) stdio_manager: SessionMCPManager,
    pub(super) http_manager: HttpSessionManager,
    pub(super) stdio_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub(super) http_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub(super) server_name_to_id: HashMap<String, String>,
    pub(super) tool_discovery_timeout: Duration,
    pub(super) app_handle: Option<AppHandle>,
}

fn describe_panic_payload(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    "unknown panic payload".to_string()
}

async fn mark_server_failed(
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

fn log_background_discovery_metrics(
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

async fn await_tool_discovery<T, E, F>(
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

async fn load_stdio_tools(
    context: DiscoveryContext,
    stdio_manager: SessionMCPManager,
    proxy: Arc<MCPServiceProxy>,
    stdio_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
) {
    if !stdio_configs.is_empty() {
        log::info!(
            "[bg] Loading tools for {} stdio servers in parallel (session: {})",
            stdio_configs.len(),
            context.session_id
        );
        let update_result = update_runtime_state_store(
            &context.runtime_states,
            &context.session_id,
            context.app_handle.as_ref(),
            |state| {
                for server_name in stdio_configs.keys() {
                    apply_server_connecting(state, server_name, SessionRuntimeTransport::Stdio);
                }
            },
        )
        .await;
        if update_result.emitted {
            context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
        }
    }

    let mut stdio_tasks: JoinSet<PanicDiscoveryResult> = JoinSet::new();
    for server_name in stdio_configs.keys() {
        let manager = stdio_manager.clone();
        let proxy = proxy.clone();
        let id_map = context.server_name_to_id.clone();
        let task_context = context.clone();
        let server_name = server_name.clone();
        stdio_tasks.spawn(async move {
            let panic_server_name = server_name.clone();
            let panic_server_name_for_message = panic_server_name.clone();
            let panic_session_id = task_context.session_id.clone();
            match AssertUnwindSafe(async move {
                log::debug!(
                    "[bg] Fetching tools from stdio server '{}' for session '{}'",
                    server_name,
                    task_context.session_id
                );
                match await_tool_discovery(
                    manager.list_tools_with_init_timeout(&server_name, task_context.server_discovery_cap),
                    task_context.server_discovery_cap,
                    "stdio",
                    &server_name,
                    &task_context.session_id,
                    stdio_failure_kind,
                )
                .await
                {
                    Ok(tools) => {
                        let tool_count = tools.len();
                        log::info!(
                            "[bg] ✅ Fetched {} tools from stdio server '{}' for session '{}': raw=[{}]",
                            tool_count,
                            server_name,
                            task_context.session_id,
                            summarize_tool_names(&tools)
                        );
                        if let Some(server_id) = id_map.get(&server_name) {
                            persist_tool_cache_for_server(
                                &server_name,
                                Some(server_id.as_str()),
                                "stdio",
                                &tools,
                            )
                            .await;
                        }
                        let prefixed_tools: Vec<_> = tools
                            .into_iter()
                            .map(|mut tool| {
                                tool.name = format!("{}__{}", server_name, tool.name);
                                tool
                            })
                            .collect();
                        log::info!(
                            "[bg] Session-visible stdio tools for '{}' in session '{}': [{}]",
                            server_name,
                            task_context.session_id,
                            summarize_tool_names(&prefixed_tools)
                        );
                        proxy
                            .set_session_stdio_tools(server_name.clone(), prefixed_tools)
                            .await;
                        task_context
                            .retry_states
                            .clear_server(&task_context.session_id, &server_name)
                            .await;
                        let update_result = update_runtime_state_store(
                            &task_context.runtime_states,
                            &task_context.session_id,
                            task_context.app_handle.as_ref(),
                            |state| {
                                apply_server_ready(
                                    state,
                                    &server_name,
                                    SessionRuntimeTransport::Stdio,
                                    tool_count,
                                );
                            },
                        )
                        .await;
                        if update_result.emitted {
                            task_context
                                .runtime_state_emits
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(failure) => {
                        log::error!("[bg] ❌ {}", failure.message);
                        mark_server_failed(
                            &task_context,
                            &server_name,
                            SessionRuntimeTransport::Stdio,
                            failure.message,
                            failure.kind,
                        )
                        .await;
                    }
                }
            })
            .catch_unwind()
            .await
            {
                Ok(()) => PanicDiscoveryResult {
                    server_name: panic_server_name,
                    panic_error: None,
                },
                Err(payload) => PanicDiscoveryResult {
                    server_name: panic_server_name,
                    panic_error: Some(format!(
                        "stdio server '{}' tool discovery panicked for session '{}': {}",
                        panic_server_name_for_message,
                        panic_session_id,
                        describe_panic_payload(payload.as_ref())
                    )),
                },
            }
        });
    }

    while let Some(result) = stdio_tasks.join_next().await {
        match result {
            Ok(PanicDiscoveryResult {
                server_name,
                panic_error: Some(error),
            }) => {
                log::error!("[bg] ❌ {}", error);
                mark_server_failed(
                    &context,
                    &server_name,
                    SessionRuntimeTransport::Stdio,
                    error,
                    FailureKind::Transient,
                )
                .await;
            }
            Ok(PanicDiscoveryResult {
                panic_error: None, ..
            }) => {}
            Err(error) => {
                log::error!("[bg] stdio server init task join failed: {:?}", error);
            }
        }
    }
}

async fn load_http_tools(
    context: DiscoveryContext,
    http_manager: HttpSessionManager,
    proxy: Arc<MCPServiceProxy>,
    http_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
) {
    if !http_configs.is_empty() {
        log::info!(
            "[bg] Loading tools for {} HTTP servers in parallel (session: {})",
            http_configs.len(),
            context.session_id
        );
        let update_result = update_runtime_state_store(
            &context.runtime_states,
            &context.session_id,
            context.app_handle.as_ref(),
            |state| {
                for server_name in http_configs.keys() {
                    apply_server_connecting(state, server_name, SessionRuntimeTransport::Http);
                }
            },
        )
        .await;
        if update_result.emitted {
            context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
        }
    }

    let mut http_tasks: JoinSet<PanicDiscoveryResult> = JoinSet::new();
    for server_name in http_configs.keys() {
        let manager = http_manager.clone();
        let proxy = proxy.clone();
        let id_map = context.server_name_to_id.clone();
        let task_context = context.clone();
        let server_name = server_name.clone();
        http_tasks.spawn(async move {
            let panic_server_name = server_name.clone();
            let panic_server_name_for_message = panic_server_name.clone();
            let panic_session_id = task_context.session_id.clone();
            match AssertUnwindSafe(async move {
                if proxy.has_http_tools_cached(&server_name).await {
                    log::info!(
                        "[bg] ⚡ Skipping HTTP server '{}' - tools already cached",
                        server_name
                    );
                    task_context
                        .retry_states
                        .clear_server(&task_context.session_id, &server_name)
                        .await;
                    let update_result = update_runtime_state_store(
                        &task_context.runtime_states,
                        &task_context.session_id,
                        task_context.app_handle.as_ref(),
                        |state| {
                            apply_server_ready(
                                state,
                                &server_name,
                                SessionRuntimeTransport::Http,
                                0,
                            );
                        },
                    )
                    .await;
                    if update_result.emitted {
                        task_context
                            .runtime_state_emits
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    return;
                }

                let update_result = update_runtime_state_store(
                    &task_context.runtime_states,
                    &task_context.session_id,
                    task_context.app_handle.as_ref(),
                    |state| {
                        apply_server_discovering(
                            state,
                            &server_name,
                            SessionRuntimeTransport::Http,
                        );
                    },
                )
                .await;
                if update_result.emitted {
                    task_context
                        .runtime_state_emits
                        .fetch_add(1, Ordering::Relaxed);
                }
                log::debug!(
                    "[bg] Fetching tools from HTTP server '{}' for session '{}'",
                    server_name,
                    task_context.session_id
                );
                match await_tool_discovery(
                    manager.list_tools(&server_name),
                    task_context.server_discovery_cap,
                    "http",
                    &server_name,
                    &task_context.session_id,
                    |_| FailureKind::Transient,
                )
                .await
                {
                    Ok(tools) => {
                        let tool_count = tools.len();
                        log::info!(
                            "[bg] ✅ Fetched {} tools from HTTP server '{}' for session '{}': raw=[{}]",
                            tool_count,
                            server_name,
                            task_context.session_id,
                            summarize_tool_names(&tools)
                        );
                        if let Some(server_id) = id_map.get(&server_name) {
                            persist_tool_cache_for_server(
                                &server_name,
                                Some(server_id.as_str()),
                                "http",
                                &tools,
                            )
                            .await;
                        }
                        let prefixed_tools: Vec<_> = tools
                            .into_iter()
                            .map(|mut tool| {
                                tool.name = format!("{}__{}", server_name, tool.name);
                                tool
                            })
                            .collect();
                        log::info!(
                            "[bg] Session-visible HTTP tools for '{}' in session '{}': [{}]",
                            server_name,
                            task_context.session_id,
                            summarize_tool_names(&prefixed_tools)
                        );
                        proxy
                            .set_session_http_tools(server_name.clone(), prefixed_tools)
                            .await;
                        task_context
                            .retry_states
                            .clear_server(&task_context.session_id, &server_name)
                            .await;
                        let update_result = update_runtime_state_store(
                            &task_context.runtime_states,
                            &task_context.session_id,
                            task_context.app_handle.as_ref(),
                            |state| {
                                apply_server_ready(
                                    state,
                                    &server_name,
                                    SessionRuntimeTransport::Http,
                                    tool_count,
                                );
                            },
                        )
                        .await;
                        if update_result.emitted {
                            task_context
                                .runtime_state_emits
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(failure) => {
                        log::error!("[bg] ❌ {}", failure.message);
                        mark_server_failed(
                            &task_context,
                            &server_name,
                            SessionRuntimeTransport::Http,
                            failure.message,
                            failure.kind,
                        )
                        .await;
                    }
                }
            })
            .catch_unwind()
            .await
            {
                Ok(()) => PanicDiscoveryResult {
                    server_name: panic_server_name,
                    panic_error: None,
                },
                Err(payload) => PanicDiscoveryResult {
                    server_name: panic_server_name,
                    panic_error: Some(format!(
                        "http server '{}' tool discovery panicked for session '{}': {}",
                        panic_server_name_for_message,
                        panic_session_id,
                        describe_panic_payload(payload.as_ref())
                    )),
                },
            }
        });
    }

    while let Some(result) = http_tasks.join_next().await {
        match result {
            Ok(PanicDiscoveryResult {
                server_name,
                panic_error: Some(error),
            }) => {
                log::error!("[bg] ❌ {}", error);
                mark_server_failed(
                    &context,
                    &server_name,
                    SessionRuntimeTransport::Http,
                    error,
                    FailureKind::Transient,
                )
                .await;
            }
            Ok(PanicDiscoveryResult {
                panic_error: None, ..
            }) => {}
            Err(error) => {
                log::error!("[bg] HTTP server init task join failed: {:?}", error);
            }
        }
    }
}

pub(super) async fn spawn_background_tool_loading(
    manager: &MCPServiceProxyManager,
    plan: BackgroundDiscoveryPlan,
) {
    let (ready_tx, _) = tokio::sync::watch::channel(false);
    let ready_tx = Arc::new(ready_tx);
    manager.proxy_readiness.write().await.insert(
        plan.session_id.clone(),
        ProxyReadinessEntry {
            ready_tx: ready_tx.clone(),
            app_handle: plan.app_handle.clone(),
        },
    );

    let context = DiscoveryContext {
        session_id: plan.session_id,
        app_handle: plan.app_handle,
        runtime_states: manager.runtime_states.clone(),
        server_name_to_id: Arc::new(plan.server_name_to_id),
        tool_discovery_timeout: plan.tool_discovery_timeout,
        server_discovery_cap: server_discovery_cap(plan.tool_discovery_timeout),
        runtime_state_emits: Arc::new(AtomicU32::new(0)),
        retry_states: manager.server_retry_states.clone(),
    };
    tokio::spawn(async move {
        let total_start = Instant::now();
        let stdio_manager = plan.stdio_manager;
        let http_manager = plan.http_manager;
        let stdio_proxy = plan.proxy.clone();
        let http_proxy = plan.proxy;
        let stdio_configs = plan.stdio_configs;
        let http_configs = plan.http_configs;
        let stdio_server_count = stdio_configs.len();
        let http_server_count = http_configs.len();
        let discovery_timeout = context.tool_discovery_timeout;

        // Keep discovery on a JoinHandle so a deadline can finalize Session Ready
        // without cancelling in-flight stdio/HTTP loading (late Ready still updates).
        let discovery_context = context.clone();
        let mut discovery_handle = tokio::spawn(async move {
            let stdio_fut = async {
                let started = Instant::now();
                load_stdio_tools(
                    discovery_context.clone(),
                    stdio_manager,
                    stdio_proxy,
                    stdio_configs,
                )
                .await;
                started.elapsed().as_millis()
            };
            let http_fut = async {
                let started = Instant::now();
                load_http_tools(
                    discovery_context.clone(),
                    http_manager,
                    http_proxy,
                    http_configs,
                )
                .await;
                started.elapsed().as_millis()
            };
            tokio::join!(stdio_fut, http_fut)
        });

        tokio::select! {
            join_result = &mut discovery_handle => {
                match join_result {
                    Ok((stdio_ms, http_ms)) => {
                        finish_background_discovery(
                            &context,
                            &ready_tx,
                            DiscoveryFinishStats {
                                stdio_server_count,
                                http_server_count,
                                total_start,
                                stdio_ms,
                                http_ms,
                                already_finalized_by_deadline: false,
                            },
                        )
                        .await;
                    }
                    Err(error) => {
                        log::error!(
                            "[bg] Tool discovery task join failed for session {}: {:?}",
                            context.session_id,
                            error
                        );
                        ready_tx.send_replace(true);
                        let update_result = update_runtime_state_store(
                            &context.runtime_states,
                            &context.session_id,
                            context.app_handle.as_ref(),
                            |state| {
                                // The task is dead: its servers failed, they are not
                                // "still starting" (which `TimedOut` would claim).
                                apply_discovery_task_failed(
                                    state,
                                    "Tool discovery task failed before completion",
                                );
                            },
                        )
                        .await;
                        if update_result.emitted {
                            context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            _ = tokio::time::sleep(discovery_timeout) => {
                let reason = format!(
                    "Tool discovery timed out after {}s",
                    discovery_timeout.as_secs()
                );
                log::warn!(
                    "[bg] {} for session {}; marking pending servers TimedOut and setting Session Ready",
                    reason,
                    context.session_id
                );
                // Use send_replace, not send: receiver_count may be 0.
                ready_tx.send_replace(true);
                let update_result = update_runtime_state_store(
                    &context.runtime_states,
                    &context.session_id,
                    context.app_handle.as_ref(),
                    |state| {
                        apply_discovery_timeout_finalize(state, &reason);
                    },
                )
                .await;
                if update_result.emitted {
                    context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
                }

                // Keep loading after Ready: late apply_server_ready may overwrite
                // TimedOut→Ready and refresh tools; finish recomputes summary.
                match discovery_handle.await {
                    Ok((stdio_ms, http_ms)) => {
                        finish_background_discovery(
                            &context,
                            &ready_tx,
                            DiscoveryFinishStats {
                                stdio_server_count,
                                http_server_count,
                                total_start,
                                stdio_ms,
                                http_ms,
                                already_finalized_by_deadline: true,
                            },
                        )
                        .await;
                    }
                    Err(error) => {
                        log::error!(
                            "[bg] Late tool discovery join failed for session {}: {:?}",
                            context.session_id,
                            error
                        );
                        // The task that was still starting `TimedOut` servers is dead, so
                        // they failed; leaving `TimedOut` would show "starting" forever.
                        let update_result = update_runtime_state_store(
                            &context.runtime_states,
                            &context.session_id,
                            context.app_handle.as_ref(),
                            |state| {
                                apply_discovery_task_failed(
                                    state,
                                    "Tool discovery task failed before completion",
                                );
                            },
                        )
                        .await;
                        if update_result.emitted {
                            context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    });
}

struct DiscoveryFinishStats {
    stdio_server_count: usize,
    http_server_count: usize,
    total_start: Instant,
    stdio_ms: u128,
    http_ms: u128,
    already_finalized_by_deadline: bool,
}

async fn finish_background_discovery(
    context: &DiscoveryContext,
    ready_tx: &Arc<tokio::sync::watch::Sender<bool>>,
    stats: DiscoveryFinishStats,
) {
    log::info!(
        "[bg] Tool loading complete for session: {} (deadline_finalize={})",
        context.session_id,
        stats.already_finalized_by_deadline
    );
    // Use send_replace, not send: the initial watch receiver is dropped at
    // channel creation, and soft readiness waits may also drop before discovery
    // finishes. tokio::watch::Sender::send refuses to update when
    // receiver_count == 0, which left later waiters stuck on false until timeout.
    ready_tx.send_replace(true);
    let update_result = update_runtime_state_store(
        &context.runtime_states,
        &context.session_id,
        context.app_handle.as_ref(),
        apply_initialization_complete,
    )
    .await;
    if update_result.emitted {
        context.runtime_state_emits.fetch_add(1, Ordering::Relaxed);
    }

    log_background_discovery_metrics(
        &context.session_id,
        stats.stdio_server_count,
        stats.http_server_count,
        context.runtime_state_emits.load(Ordering::Relaxed),
        stats.total_start.elapsed().as_millis(),
        stats.stdio_ms,
        stats.http_ms,
    );
}

/// Parameters for retrying failed servers in the background.
pub(super) struct RetryFailedServersParams {
    pub proxy: Arc<MCPServiceProxy>,
    pub stdio_manager: SessionMCPManager,
    pub http_manager: HttpSessionManager,
    pub stdio_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub http_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub server_name_to_id: HashMap<String, String>,
    pub tool_discovery_timeout: Duration,
    pub app_handle: Option<AppHandle>,
}

/// Spawns background discovery to retry a set of failed MCP servers for an existing session.
/// Does not touch proxy readiness (the session is already active/ready).
pub(super) async fn spawn_retry_failed_servers(
    manager: &MCPServiceProxyManager,
    session_id: String,
    params: RetryFailedServersParams,
) {
    let context = DiscoveryContext {
        session_id: session_id.clone(),
        app_handle: params.app_handle,
        runtime_states: manager.runtime_states.clone(),
        server_name_to_id: Arc::new(params.server_name_to_id),
        tool_discovery_timeout: params.tool_discovery_timeout,
        server_discovery_cap: server_discovery_cap(params.tool_discovery_timeout),
        runtime_state_emits: Arc::new(AtomicU32::new(0)),
        retry_states: manager.server_retry_states.clone(),
    };

    tokio::spawn(async move {
        let stdio_fut = load_stdio_tools(
            context.clone(),
            params.stdio_manager,
            params.proxy.clone(),
            params.stdio_configs,
        );
        let http_fut = load_http_tools(
            context.clone(),
            params.http_manager,
            params.proxy,
            params.http_configs,
        );
        tokio::join!(stdio_fut, http_fut);

        update_runtime_state_store(
            &context.runtime_states,
            &context.session_id,
            context.app_handle.as_ref(),
            |state| {
                state.recompute_summary();
            },
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_cap_outlasts_the_session_wait() {
        assert_eq!(server_discovery_cap(Duration::from_secs(30)).as_secs(), 600);
        assert_eq!(
            server_discovery_cap(Duration::from_secs(900)).as_secs(),
            900
        );
    }

    #[tokio::test]
    async fn a_slow_server_is_not_given_up_at_the_session_wait() {
        let session_wait = Duration::from_millis(20);
        let slow = || async {
            tokio::time::sleep(Duration::from_millis(120)).await;
            Ok::<_, String>(3)
        };

        // The session-wait budget alone would have timed this server out (the old behavior).
        let at_session_wait =
            await_tool_discovery(slow(), session_wait, "stdio", "s", "sess", |_: &String| {
                FailureKind::Transient
            })
            .await;
        let timed_out = at_session_wait.unwrap_err();
        assert!(timed_out.message.contains("timed out"));
        assert_eq!(timed_out.kind, FailureKind::Transient);

        // With the server cap it keeps loading and becomes ready.
        let at_cap = await_tool_discovery(
            slow(),
            server_discovery_cap(session_wait),
            "stdio",
            "s",
            "sess",
            |_: &String| FailureKind::Transient,
        )
        .await;
        assert_eq!(at_cap.unwrap(), 3);
    }

    #[tokio::test]
    async fn a_server_that_errors_is_failed_immediately() {
        let result = await_tool_discovery(
            async { Err::<(), _>("connection closed") },
            server_discovery_cap(Duration::from_secs(30)),
            "stdio",
            "s",
            "sess",
            |_: &&str| FailureKind::Transient,
        )
        .await;
        let error = result.unwrap_err().message;
        assert!(error.contains("tool discovery failed") && !error.contains("timed out"));
    }

    #[tokio::test]
    async fn an_unavailable_session_container_is_a_permanent_failure() {
        let result = await_tool_discovery(
            async {
                Err::<(), _>(SessionMCPError::ContainerUnavailable(
                    "Cannot pull Docker image 'missing:latest'".to_string(),
                ))
            },
            server_discovery_cap(Duration::from_secs(30)),
            "stdio",
            "s",
            "sess",
            stdio_failure_kind,
        )
        .await;
        let failure = result.unwrap_err();
        assert_eq!(failure.kind, FailureKind::Permanent);
        assert!(failure
            .message
            .contains("Session container is not available"));

        let crashed = SessionMCPError::InitFailed("connection closed".to_string());
        assert_eq!(stdio_failure_kind(&crashed), FailureKind::Transient);
    }
}
