use super::super::super::service_proxy::MCPServiceProxy;
use super::super::super::session_isolation::SessionMCPManager;
use super::super::persist_tool_cache_for_server;
use super::super::retry_policy::FailureKind;
use super::super::runtime_updates::{
    apply_server_connecting, apply_server_ready, update_runtime_state_store,
};
use super::helpers::{await_tool_discovery, mark_server_failed};
use super::types::{
    describe_panic_payload, stdio_failure_kind, DiscoveryContext, PanicDiscoveryResult,
};
use crate::agent::runtime_state::SessionRuntimeTransport;
use crate::services::mcp_server_service::summarize_tool_names;
use futures::FutureExt;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::task::JoinSet;

pub(super) async fn load_stdio_tools(
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
