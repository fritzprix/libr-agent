use crate::entity::mcp_server;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn extract_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn format_capability_list(values: &[String]) -> String {
    if values.is_empty() {
        "None".to_string()
    } else {
        values.join(", ")
    }
}

pub(super) fn build_server_name_lookup(
    server_models: &[mcp_server::Model],
) -> HashMap<String, String> {
    server_models
        .iter()
        .map(|model| (model.id.clone(), model.name.clone()))
        .collect()
}

pub(super) fn resolve_external_server_labels(
    external_ids: &[String],
    server_name_lookup: &HashMap<String, String>,
) -> Vec<String> {
    external_ids
        .iter()
        .map(|server_id| {
            server_name_lookup
                .get(server_id)
                .map(|name| format!("{} (ID: {})", name, server_id))
                .unwrap_or_else(|| format!("Unknown server (ID: {})", server_id))
        })
        .collect()
}

pub(super) fn format_external_server_refs(
    external_ids: &[String],
    server_name_lookup: &HashMap<String, String>,
) -> String {
    format_capability_list(&resolve_external_server_labels(
        external_ids,
        server_name_lookup,
    ))
}
