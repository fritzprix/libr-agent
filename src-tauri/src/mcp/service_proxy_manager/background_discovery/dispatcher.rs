use super::super::management::ProxyReadinessEntry;
use super::super::runtime_updates::{
    apply_discovery_task_failed, apply_discovery_timeout_finalize, apply_initialization_complete,
    update_runtime_state_store,
};
use super::super::MCPServiceProxyManager;
use super::helpers::log_background_discovery_metrics;
use super::http_loader::load_http_tools;
use super::stdio_loader::load_stdio_tools;
use super::types::{
    server_discovery_cap, BackgroundDiscoveryPlan, DiscoveryContext, DiscoveryFinishStats,
    RetryFailedServersParams,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub(in super::super) async fn spawn_background_tool_loading(
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

pub(super) async fn finish_background_discovery(
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

/// Spawns background discovery to retry a set of failed MCP servers for an existing session.
/// Does not touch proxy readiness (the session is already active/ready).
pub(in super::super) async fn spawn_retry_failed_servers(
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
