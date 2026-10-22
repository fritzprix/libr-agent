use std::collections::HashSet;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::presets::{get_recommended_servers, MCPServerPreset};
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RequiredVariable {
    pub key: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone)]
pub(super) struct CatalogEntry {
    pub preset: MCPServerPreset,
    pub transport: Value,
    pub required_variables: Vec<RequiredVariable>,
    pub needs_oauth: bool,
    pub registered: bool,
}

/// `tool__listServers({scope: "catalog"})`: curated presets from `mcp-server.json`.
pub(super) async fn list_catalog(
    query: &str,
    limit: usize,
    offset: usize,
) -> Result<MCPResult, String> {
    let registered_names = match get_mcp_server_repository().list().await {
        Ok(models) => models
            .into_iter()
            .map(|model| model.name.to_lowercase())
            .collect::<HashSet<_>>(),
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to query MCP server list: {}", e),
                ToolGroup::Tool,
            )
            .with_guidance(vec!["Check database connectivity".to_string()])
            .to_mcp_result())
        }
    };

    let entries = build_catalog_entries(get_recommended_servers(), &registered_names, query);
    Ok(render_catalog(&entries, query, limit, offset))
}

pub(super) fn build_catalog_entries(
    presets: Vec<MCPServerPreset>,
    registered_names: &HashSet<String>,
    query: &str,
) -> Vec<CatalogEntry> {
    presets
        .into_iter()
        .filter(|preset| preset_matches_query(preset, query))
        .map(|preset| CatalogEntry {
            transport: preset_transport(&preset),
            required_variables: required_variables(&preset),
            needs_oauth: preset
                .authentication
                .as_ref()
                .is_some_and(|auth| auth.oauth_type == "oauth2.1"),
            registered: registered_names.contains(&preset.name.to_lowercase()),
            preset,
        })
        .collect()
}

fn preset_matches_query(preset: &MCPServerPreset, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    preset.name.to_lowercase().contains(query)
        || preset.category.to_lowercase().contains(query)
        || preset
            .description
            .as_deref()
            .is_some_and(|d| d.to_lowercase().contains(query))
}

fn string_map(value: Option<&Value>) -> Map<String, Value> {
    match value {
        Some(Value::Object(map)) => map
            .iter()
            .filter(|(_, v)| v.is_string())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        _ => Map::new(),
    }
}

/// Same shape the UI builds in `buildServerEntityFromPreset`, ready for `tool__registerServer`.
fn preset_transport(preset: &MCPServerPreset) -> Value {
    let env = string_map(preset.env.as_ref());
    if let (true, Some(url)) = (preset.transport_type == "sse", preset.url.as_ref()) {
        let mut transport = json!({ "type": "http-sse", "url": url, "enableSSE": true });
        if !env.is_empty() {
            transport["headers"] = Value::Object(env);
        }
        return transport;
    }

    let mut transport = json!({
        "type": "stdio",
        "command": preset.command.clone().unwrap_or_else(|| "uvx".to_string()),
        "args": preset.args.clone().unwrap_or_default(),
    });
    if !env.is_empty() {
        transport["env"] = Value::Object(env);
    }
    transport
}

fn required_variables(preset: &MCPServerPreset) -> Vec<RequiredVariable> {
    let Some(Value::Object(definitions)) = preset.variable_definitions.as_ref() else {
        return Vec::new();
    };
    definitions
        .iter()
        .filter(|(_, def)| {
            def.get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|(key, def)| RequiredVariable {
            key: key.clone(),
            label: def.get("label").and_then(Value::as_str).map(str::to_string),
            description: def
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
            target: def
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("env")
                .to_string(),
        })
        .collect()
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

pub(super) fn render_catalog(
    entries: &[CatalogEntry],
    query: &str,
    limit: usize,
    offset: usize,
) -> MCPResult {
    if entries.is_empty() {
        return SuccessHint::new(
            format!("No catalog presets match '{}'.", query),
            vec![
                "Try a broader query or omit it to see the full catalog".to_string(),
                "If no preset fits, a web or npm search is possible, but warn the user that the source is not curated".to_string(),
            ],
        )
        .to_mcp_result();
    }

    let total = entries.len();
    let page: Vec<&CatalogEntry> = entries.iter().skip(offset).take(limit).collect();

    let mut body = format!(
        "Found {} curated MCP presets{}:\n\n| Preset | Category | Transport | Required input | Registered | Description |\n|---|---|---|---|---|---|\n",
        total,
        if query.is_empty() {
            String::new()
        } else {
            format!(" matching '{}'", query)
        }
    );
    for entry in &page {
        let mut needs: Vec<String> = entry
            .required_variables
            .iter()
            .map(|v| v.key.clone())
            .collect();
        if entry.needs_oauth {
            needs.push("OAuth".to_string());
        }
        let needs = if needs.is_empty() {
            "none".to_string()
        } else {
            needs.join(", ")
        };
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            escape_cell(&entry.preset.name),
            escape_cell(&entry.preset.category),
            entry.transport["type"].as_str().unwrap_or("-"),
            escape_cell(&needs),
            if entry.registered { "yes" } else { "no" },
            escape_cell(entry.preset.description.as_deref().unwrap_or("")),
        ));
    }
    if offset.saturating_add(limit) < total {
        body.push_str(&format!(
            "\n*(Showing {} to {} of {}. Call again with offset: {} to see more)*",
            offset + 1,
            offset + page.len(),
            total,
            offset.saturating_add(limit)
        ));
    }

    let presets = page
        .iter()
        .map(|entry| {
            let mut item = json!({
                "name": entry.preset.name,
                "category": entry.preset.category,
                "description": entry.preset.description,
                "transport": entry.transport,
                "requiredVariables": entry.required_variables.iter().map(|v| json!({
                    "key": v.key,
                    "label": v.label,
                    "description": v.description,
                    "target": v.target,
                })).collect::<Vec<_>>(),
                "registered": entry.registered,
            });
            if let Some(auth) = entry.preset.authentication.as_ref() {
                if let Ok(auth) = serde_json::to_value(auth) {
                    item["authentication"] = auth;
                }
            }
            item
        })
        .collect::<Vec<_>>();

    SuccessHint::new(
        body,
        vec![
            "Install a preset with tool__registerServer using its name and the transport from data.presets[] (and authentication when present).".to_string(),
            "Ask the user for each required input and replace placeholder values such as YOUR_* before registering.".to_string(),
            "Presets marked registered already exist: use tool__updateServer instead of registering a duplicate.".to_string(),
        ],
    )
    .to_mcp_result_with_data(Some(json!({
        "scope": "catalog",
        "query": query,
        "offset": offset,
        "limit": limit,
        "totalResults": total,
        "presets": presets,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_entries(registered: &[&str]) -> Vec<CatalogEntry> {
        let names = registered.iter().map(|n| n.to_string()).collect();
        build_catalog_entries(get_recommended_servers(), &names, "")
    }

    #[test]
    fn catalog_returns_every_preset_and_marks_registered() {
        let presets = get_recommended_servers();
        let entries = all_entries(&["yahoo-finance"]);
        assert_eq!(entries.len(), presets.len());
        for entry in &entries {
            assert_eq!(entry.registered, entry.preset.name == "yahoo-finance");
        }
    }

    #[test]
    fn query_matches_description_and_category() {
        let names = HashSet::new();
        let by_desc = build_catalog_entries(get_recommended_servers(), &names, "stock");
        assert!(by_desc.iter().any(|e| e.preset.name == "yahoo-finance"));
        let by_category = build_catalog_entries(get_recommended_servers(), &names, "search");
        assert!(by_category.iter().any(|e| e.preset.name == "exa"));
        assert!(!by_category.iter().any(|e| e.preset.name == "yahoo-finance"));
    }

    #[test]
    fn stdio_preset_transport_is_ready_for_register_server() {
        let entries = all_entries(&[]);
        let yahoo = entries
            .iter()
            .find(|e| e.preset.name == "yahoo-finance")
            .expect("yahoo-finance preset");
        assert_eq!(yahoo.transport["type"], "stdio");
        assert_eq!(yahoo.transport["command"], "npx");
        assert!(yahoo.transport.get("env").is_none());
        assert!(yahoo.required_variables.is_empty());
    }

    #[test]
    fn http_preset_exposes_headers_and_required_variables() {
        let entries = all_entries(&[]);
        let github = entries
            .iter()
            .find(|e| e.preset.name == "github")
            .expect("github preset");
        assert_eq!(github.transport["type"], "http-sse");
        assert!(github.transport["url"].is_string());
        assert!(github.transport["headers"]["Authorization"].is_string());
        assert_eq!(github.required_variables.len(), 1);
        assert_eq!(github.required_variables[0].key, "GITHUB_PAT");
        assert_eq!(github.required_variables[0].target, "bearer-token");
    }

    #[test]
    fn render_paginates_and_reports_registered_flag() {
        let entries = all_entries(&["github"]);
        let github_index = entries
            .iter()
            .position(|e| e.preset.name == "github")
            .expect("github preset");
        let result = render_catalog(&entries, "", 1, github_index);
        let data = result.structured_content.expect("structured data");
        assert_eq!(data["totalResults"], entries.len());
        let presets = data["presets"].as_array().expect("presets array");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0]["name"], "github");
        assert_eq!(presets[0]["registered"], true);
        assert_eq!(presets[0]["requiredVariables"][0]["key"], "GITHUB_PAT");
    }
}
