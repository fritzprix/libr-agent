use super::super::plugin_options;
use super::config::snapshot_to_mcp_config_json_in;
use super::PluginExpansion;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub fn session_plugin_mcp_records(
    session_store: &Path,
    trust_dir: Option<&Path>,
) -> (
    Vec<(
        crate::services::hooks::registry::PluginSourceRecord,
        PluginMcpServerSnapshot,
    )>,
    BTreeMap<String, std::path::PathBuf>,
) {
    use crate::services::hooks::registry::list_session_store_sources;
    let records = list_session_store_sources(trust_dir, session_store);
    let mut out = Vec::new();
    let mut roots = BTreeMap::new();
    for record in records {
        let source_id = record.manifest.source_id.clone();
        roots.insert(source_id, record.root_dir.clone());
        // #2927: a disabled plugin contributes no servers, approved or not.
        if !record.manifest.mcp_approved || !record.manifest.enabled {
            continue;
        }
        for snapshot in record.manifest.mcp_servers.clone() {
            out.push((record.clone(), snapshot));
        }
    }
    (out, roots)
}

/// One session-scoped plugin MCP server resolved for session load (#2924 D1):
/// the store config JSON (plugin-root placeholders expanded, plugin-source
/// marker set) plus the runtime name and pseudo-id the session proxy keys it
/// under (`plugin:<source_id>:<name>` — no DB row exists for these).
pub struct SessionPluginServerConfig {
    pub server_name: String,
    pub pseudo_id: String,
    pub config: crate::mcp::types::MCPServerConfig,
}

/// Load every approved session-plugin server from one session plugin store
/// into ready-to-connect configs (the D1 loader used by
/// `load_requested_server_configs`). Invalid entries are skipped with a
/// warning; `taken_session_names` keeps runtime names unique across the
/// session's servers, same as DB-loaded servers. `trust_dir` is the
/// out-of-workspace trust record directory for this session (#2942 S3.2).
pub async fn build_session_plugin_server_configs(
    session_store: &Path,
    session_id: &str,
    trust_dir: Option<&Path>,
    taken_session_names: &mut HashSet<String>,
) -> Vec<SessionPluginServerConfig> {
    use crate::mcp::types::MCPServerConfig;
    use crate::mcp::utils::unique_session_server_name;

    let (records, _roots) = session_plugin_mcp_records(session_store, trust_dir);
    let mut out = Vec::new();
    for (record, snapshot) in records {
        let source_id = record.manifest.source_id.clone();
        let root = record.root_dir.clone();
        // #2926: session-scope plugins get their session data dir and their
        // resolved `userConfig` values, exactly like their hooks do. The
        // option sidecar is read from the trust directory, never from the
        // agent-writable workspace store (#2926 gatekeeper G1.1), and the
        // secure namespace is scoped to this session (G1.2).
        let (options, warning) = plugin_options::load_option_values(
            trust_dir.unwrap_or(session_store),
            &source_id,
            Some(session_id),
        )
        .await;
        if let Some(warning) = warning {
            log::warn!("{warning}");
        }
        let data_dir =
            crate::services::hooks::registry::get_session_plugin_data_dir(session_id, &source_id)
                .ok();
        let config_json = match snapshot_to_mcp_config_json_in(
            &snapshot,
            &PluginExpansion {
                root: &root,
                data_dir: data_dir.as_deref(),
                options: &options,
            },
            &source_id,
        ) {
            Ok(json) => json,
            Err(error) => {
                log::warn!(
                    "Session {}: plugin {} MCP server '{}' skipped: {}",
                    session_id,
                    source_id,
                    snapshot.name,
                    error
                );
                continue;
            }
        };
        let mut config = match serde_json::from_value::<MCPServerConfig>(config_json) {
            Ok(config) => config,
            Err(error) => {
                log::warn!(
                    "Session {}: plugin {} MCP server '{}' invalid config skipped: {}",
                    session_id,
                    source_id,
                    snapshot.name,
                    error
                );
                continue;
            }
        };
        let pseudo_id = format!(
            "{}{}:{}",
            crate::mcp::types::PLUGIN_SOURCE_PREFIX,
            source_id,
            snapshot.name
        );
        let server_name =
            unique_session_server_name(&snapshot.name, &pseudo_id, taken_session_names);
        config.name = Some(server_name.clone());
        out.push(SessionPluginServerConfig {
            server_name,
            pseudo_id,
            config,
        });
    }
    out
}
