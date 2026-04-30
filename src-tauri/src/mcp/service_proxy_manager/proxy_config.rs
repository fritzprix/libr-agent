use super::super::session_isolation_config::SessionIsolationConfig;
use crate::agent::runtime_state::{
    SessionRuntimeServerState, SessionRuntimeServerStatus, SessionRuntimeTransport,
};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::state::{get_mcp_server_repository, get_settings_repository};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingProxyDisposition {
    Reuse,
    Recreate,
    Fail,
}

pub fn decide_existing_proxy_disposition(
    existing_builtin_ids: &[String],
    existing_external_server_names: &[String],
    requested_builtin_ids: &[String],
    requested_external_server_names: &[String],
    config_load_failed: bool,
) -> ExistingProxyDisposition {
    let normalize = |values: &[String]| {
        let mut normalized = values.to_vec();
        normalized.sort();
        normalized.dedup();
        normalized
    };

    let existing_builtin_ids = normalize(existing_builtin_ids);
    let existing_external_server_names = normalize(existing_external_server_names);
    let requested_builtin_ids = normalize(requested_builtin_ids);
    let requested_external_server_names = normalize(requested_external_server_names);

    if config_load_failed {
        if existing_builtin_ids == requested_builtin_ids {
            ExistingProxyDisposition::Reuse
        } else {
            ExistingProxyDisposition::Fail
        }
    } else if existing_builtin_ids == requested_builtin_ids
        && existing_external_server_names == requested_external_server_names
    {
        ExistingProxyDisposition::Reuse
    } else {
        ExistingProxyDisposition::Recreate
    }
}

pub(super) struct LoadedServerConfigs {
    pub(super) stdio_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub(super) http_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub(super) server_name_to_id: HashMap<String, String>,
    pub(super) runtime_servers: Vec<SessionRuntimeServerState>,
    pub(super) requested_builtin_ids: Vec<String>,
    pub(super) requested_external_server_names: Vec<String>,
    pub(super) use_external_servers: bool,
    pub(super) config_load_error: Option<String>,
}

impl LoadedServerConfigs {
    pub(super) fn has_external_servers(&self) -> bool {
        !self.stdio_configs.is_empty() || !self.http_configs.is_empty()
    }
}

pub(super) async fn load_requested_server_configs(
    mcp_server_ids: &[String],
    tool_ids: &[String],
    session_id: &str,
) -> LoadedServerConfigs {
    let mut stdio_configs = HashMap::new();
    let mut http_configs = HashMap::new();
    let mut server_name_to_id = HashMap::new();
    let mut config_load_error = None;
    let repo = get_mcp_server_repository();
    let use_external_servers = !mcp_server_ids.is_empty();

    match repo.list().await {
        Ok(models) => {
            log::debug!(
                "Loaded {} MCP server configs from DB for session {} (use_external_servers: {}, allowed_ids: {:?})",
                models.len(),
                session_id,
                use_external_servers,
                mcp_server_ids
            );

            if !use_external_servers {
                log::info!(
                    "Session {} has no external MCP servers configured (mcp_server_ids is empty)",
                    session_id
                );
            } else {
                for model in models {
                    if !mcp_server_ids.contains(&model.id) {
                        log::debug!(
                            "Skipping MCP server '{}' (ID: {}) - not in assistant's mcp_server_ids",
                            model.name,
                            model.id
                        );
                        continue;
                    }

                    match serde_json::from_str::<crate::mcp::types::MCPServerConfig>(&model.config)
                    {
                        Ok(mut config) => {
                            let server_name = config.name.unwrap_or_else(|| model.name.clone());
                            config.name = Some(server_name.clone());
                            server_name_to_id.insert(server_name.clone(), model.id.clone());

                            log::debug!(
                                "Loading MCP server '{}' (ID: {}) into session {}",
                                server_name,
                                model.id,
                                session_id
                            );

                            match config.transport {
                                crate::mcp::types::TransportConfig::Stdio { .. } => {
                                    stdio_configs.insert(server_name, config);
                                }
                                crate::mcp::types::TransportConfig::Http { .. } => {
                                    http_configs.insert(server_name, config);
                                }
                            }
                        }
                        Err(error) => {
                            log::warn!(
                                "Failed to parse config for MCP server '{}' (ID: {}): {}",
                                model.name,
                                model.id,
                                error
                            );
                        }
                    }
                }
            }

            log::info!(
                "Session {} will connect to {} stdio servers and {} HTTP servers",
                session_id,
                stdio_configs.len(),
                http_configs.len()
            );
        }
        Err(error) => {
            config_load_error = Some(error.to_string());
            log::error!(
                "Failed to fetch MCP server configs from DB for session {}: {}",
                session_id,
                error
            );
        }
    }

    let mut requested_builtin_ids = tool_ids.to_vec();
    requested_builtin_ids.sort();
    requested_builtin_ids.dedup();

    let mut requested_external_server_names = stdio_configs.keys().cloned().collect::<Vec<_>>();
    requested_external_server_names.extend(http_configs.keys().cloned());
    requested_external_server_names.sort();
    requested_external_server_names.dedup();

    let mut runtime_servers = stdio_configs
        .keys()
        .cloned()
        .map(|name| SessionRuntimeServerState {
            name,
            transport: SessionRuntimeTransport::Stdio,
            status: SessionRuntimeServerStatus::NotStarted,
            tool_count: 0,
            error: None,
        })
        .collect::<Vec<_>>();
    runtime_servers.extend(
        http_configs
            .keys()
            .cloned()
            .map(|name| SessionRuntimeServerState {
                name,
                transport: SessionRuntimeTransport::Http,
                status: SessionRuntimeServerStatus::NotStarted,
                tool_count: 0,
                error: None,
            }),
    );

    LoadedServerConfigs {
        stdio_configs,
        http_configs,
        server_name_to_id,
        runtime_servers,
        requested_builtin_ids,
        requested_external_server_names,
        use_external_servers,
        config_load_error,
    }
}

pub(super) async fn apply_startup_timeout_settings(
    base_config: SessionIsolationConfig,
) -> SessionIsolationConfig {
    let mut config = base_config;

    if let Ok(settings_repo) = std::panic::catch_unwind(get_settings_repository) {
        if let Ok(Some(model)) = settings_repo.get("systemSettings").await {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct SystemSettings {
                mcp_server_startup_timeout_seconds: Option<u64>,
            }

            if let Ok(settings) = serde_json::from_str::<SystemSettings>(&model.value) {
                if let Some(timeout) = settings.mcp_server_startup_timeout_seconds {
                    log::debug!("Applying user setting: MCP startup timeout = {}s", timeout);
                    config = config.with_startup_timeout(timeout);
                }
            }
        }
    }

    config
}
