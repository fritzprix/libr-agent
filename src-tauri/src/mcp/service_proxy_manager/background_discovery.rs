use super::super::service_proxy::MCPServiceProxy;
use super::super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::persist_tool_cache_for_server;
use super::runtime_updates::{
    apply_batch_step, apply_initialization_complete, apply_server_connecting,
    apply_server_discovering, apply_server_failed, apply_server_ready, emit_runtime_state,
    mutate_runtime_state_store,
};
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::SessionRuntimeTransport;
use crate::services::mcp_server_service::summarize_tool_names;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
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
    tool_discovery_timeout: Duration,
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

async fn commit_runtime_state_update<F>(context: &DiscoveryContext, update: F)
where
    F: FnOnce(&mut crate::agent::runtime_state::SessionRuntimeState),
{
    let runtime_state =
        mutate_runtime_state_store(&context.runtime_states, &context.session_id, update).await;
    emit_runtime_state(
        &context.session_id,
        &runtime_state,
        context.app_handle.as_ref(),
    );
}

async fn await_tool_discovery<T, E, F>(
    future: F,
    timeout: Duration,
    transport: &str,
    server_name: &str,
    session_id: &str,
) -> Result<T, String>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(format!(
            "{} server '{}' tool discovery failed for session '{}': {}",
            transport, server_name, session_id, error
        )),
        Err(_) => Err(format!(
            "{} server '{}' tool discovery timed out after {}s for session '{}'",
            transport,
            server_name,
            timeout.as_secs(),
            session_id
        )),
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
        commit_runtime_state_update(&context, |state| {
            apply_batch_step(
                state,
                format!("Connecting to {} stdio servers", stdio_configs.len()),
            );
            for server_name in stdio_configs.keys() {
                apply_server_connecting(state, server_name, SessionRuntimeTransport::Stdio);
            }
        })
        .await;
    }

    let mut stdio_tasks: JoinSet<()> = JoinSet::new();
    for server_name in stdio_configs.keys() {
        let manager = stdio_manager.clone();
        let proxy = proxy.clone();
        let id_map = context.server_name_to_id.clone();
        let task_context = context.clone();
        let server_name = server_name.clone();
        stdio_tasks.spawn(async move {
            log::debug!(
                "[bg] Fetching tools from stdio server '{}' for session '{}'",
                server_name,
                task_context.session_id
            );
            match await_tool_discovery(
                manager.list_tools(&server_name),
                task_context.tool_discovery_timeout,
                "stdio",
                &server_name,
                &task_context.session_id,
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
                    commit_runtime_state_update(&task_context, |state| {
                        apply_server_ready(
                            state,
                            &server_name,
                            SessionRuntimeTransport::Stdio,
                            tool_count,
                        );
                    })
                    .await;
                }
                Err(error) => {
                    commit_runtime_state_update(&task_context, |state| {
                        apply_server_failed(
                            state,
                            &server_name,
                            SessionRuntimeTransport::Stdio,
                            error.clone(),
                        );
                    })
                    .await;
                    log::error!("[bg] ❌ {}", error);
                }
            }
        });
    }

    while let Some(result) = stdio_tasks.join_next().await {
        if let Err(error) = result {
            log::error!("[bg] stdio server init task panicked: {:?}", error);
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
        commit_runtime_state_update(&context, |state| {
            apply_batch_step(state, "Loading tools from HTTP servers");
        })
        .await;
    }

    let mut http_tasks: JoinSet<()> = JoinSet::new();
    for server_name in http_configs.keys() {
        let manager = http_manager.clone();
        let proxy = proxy.clone();
        let id_map = context.server_name_to_id.clone();
        let task_context = context.clone();
        let server_name = server_name.clone();
        http_tasks.spawn(async move {
            if proxy.has_http_tools_cached(&server_name).await {
                log::info!(
                    "[bg] ⚡ Skipping HTTP server '{}' - tools already cached",
                    server_name
                );
                commit_runtime_state_update(&task_context, |state| {
                    apply_server_ready(state, &server_name, SessionRuntimeTransport::Http, 0);
                })
                .await;
                return;
            }

            let _ = mutate_runtime_state_store(
                &task_context.runtime_states,
                &task_context.session_id,
                |state| {
                    apply_server_discovering(state, &server_name, SessionRuntimeTransport::Http);
                },
            )
            .await;
            log::debug!(
                "[bg] Fetching tools from HTTP server '{}' for session '{}'",
                server_name,
                task_context.session_id
            );
            match await_tool_discovery(
                manager.list_tools(&server_name),
                task_context.tool_discovery_timeout,
                "http",
                &server_name,
                &task_context.session_id,
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
                    commit_runtime_state_update(&task_context, |state| {
                        apply_server_ready(
                            state,
                            &server_name,
                            SessionRuntimeTransport::Http,
                            tool_count,
                        );
                    })
                    .await;
                }
                Err(error) => {
                    commit_runtime_state_update(&task_context, |state| {
                        apply_server_failed(
                            state,
                            &server_name,
                            SessionRuntimeTransport::Http,
                            error.clone(),
                        );
                    })
                    .await;
                    log::error!("[bg] ❌ {}", error);
                }
            }
        });
    }

    while let Some(result) = http_tasks.join_next().await {
        if let Err(error) = result {
            log::error!("[bg] HTTP server init task panicked: {:?}", error);
        }
    }
}

pub(super) async fn spawn_background_tool_loading(
    manager: &MCPServiceProxyManager,
    plan: BackgroundDiscoveryPlan,
) {
    let (ready_tx, _) = tokio::sync::watch::channel(false);
    let ready_tx = Arc::new(ready_tx);
    manager
        .proxy_readiness
        .write()
        .await
        .insert(plan.session_id.clone(), ready_tx.clone());

    let context = DiscoveryContext {
        session_id: plan.session_id,
        app_handle: plan.app_handle,
        runtime_states: manager.runtime_states.clone(),
        server_name_to_id: Arc::new(plan.server_name_to_id),
        tool_discovery_timeout: plan.tool_discovery_timeout,
    };
    tokio::spawn(async move {
        load_stdio_tools(
            context.clone(),
            plan.stdio_manager,
            plan.proxy.clone(),
            plan.stdio_configs,
        )
        .await;

        load_http_tools(
            context.clone(),
            plan.http_manager,
            plan.proxy,
            plan.http_configs,
        )
        .await;

        log::info!(
            "[bg] Tool loading complete for session: {}",
            context.session_id
        );
        let _ = ready_tx.send(true);
        commit_runtime_state_update(&context, |state| {
            apply_initialization_complete(state);
        })
        .await;
    });
}
