//! Portable Org bundle export/import (capability + teamwork SSOT + skill layers).
//!
//! Includes: referenced assistants/MCP, assistant skills, org-root workspace skills,
//! and `teamwork-artifacts/<root>/` (scaffold + deliverables).
//! Does **not** transfer session messages or full workspace trees (v1).
//!
//! One module per Tauri command (#3042): `export`, `inspect` and `import`.
//! The helpers shared between them live here.

mod export;
mod import;
mod inspect;

pub use export::export_org_bundle;
pub use import::import_org_bundle;
pub use inspect::inspect_org_bundle;
use tauri::Emitter;

/// Extract `mcpServerIds` from an assistant config JSON string.
pub fn extract_mcp_server_ids_from_config(config: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(config) else {
        return Vec::new();
    };
    value
        .get("mcpServerIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Sanitize org id for use as a single path segment under `imported-orgs/`.
pub fn sanitize_org_id_for_path(org_id: &str) -> String {
    let cleaned: String = org_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown-org".to_string()
    } else {
        cleaned
    }
}

pub(super) fn emit_progress(window: &tauri::Window, progress: u32) {
    window.emit("org-bundle:progress", progress.min(100)).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_mcp_server_ids() {
        let ids = extract_mcp_server_ids_from_config(
            r#"{"mcpServerIds":["a","b"],"disabledSkills":["x"]}"#,
        );
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn sanitizes_org_id_path_segment() {
        assert_eq!(sanitize_org_id_for_path("org-abc_123"), "org-abc_123");
        assert_eq!(sanitize_org_id_for_path("org/../evil"), "org____evil");
        assert_eq!(sanitize_org_id_for_path(""), "unknown-org");
    }
}
