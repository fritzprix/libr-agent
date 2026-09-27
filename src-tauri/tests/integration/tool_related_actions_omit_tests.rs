//! Related Actions must stay rare in builtin tool prefill (issue #1924 step 1).
//!
//! Default-omit via empty `tool_description` next_steps; only high-signal
//! cross-links remain.

use std::collections::BTreeSet;
use tauri_mcp_agent_lib::mcp::builtin::error_guidance::hint_headers::TOOL_RELATED_ACTIONS;
use tauri_mcp_agent_lib::mcp::builtin::service_id::BUILTIN_SERVICE_REGISTRY;
use tauri_mcp_agent_lib::mcp::server::tools::{
    get_all_static_builtin_tools, get_static_tools_for_server,
};

/// Tools allowed to ship a Related Actions section (qualified `server__tool`).
const RELATED_ACTIONS_KEEPERS: &[&str] = &[
    "agent__prepareTeamworkWorkspace",
    "tool__listServers",
    "tool__registerServer",
    "ui__presentInteractive",
    "ui__reportResult",
];

fn qualified_name(server: &str, tool: &str) -> String {
    format!("{server}__{tool}")
}

#[test]
fn related_actions_omitted_except_high_signal_keepers() {
    let keepers: BTreeSet<&str> = RELATED_ACTIONS_KEEPERS.iter().copied().collect();
    let mut with_related = BTreeSet::new();
    let mut related_chars = 0usize;
    let mut total_tools = 0usize;

    for entry in BUILTIN_SERVICE_REGISTRY {
        for tool in get_static_tools_for_server(entry.canonical) {
            total_tools += 1;
            let qname = qualified_name(entry.canonical, &tool.name);
            if let Some(idx) = tool.description.find(TOOL_RELATED_ACTIONS) {
                with_related.insert(qname.clone());
                related_chars += tool.description.len() - idx;
                assert!(
                    keepers.contains(qname.as_str()),
                    "unexpected Related Actions on {qname} (prefill bloat; omit next_steps)"
                );
            }
        }
    }

    assert!(
        total_tools > 40,
        "sanity: expected a full builtin tool surface, got {total_tools}"
    );

    assert_eq!(
        with_related,
        keepers.iter().map(|s| (*s).to_string()).collect(),
        "Related Actions keepers drifted — update RELATED_ACTIONS_KEEPERS intentionally"
    );

    // Soft budget: five keepers should stay well under the old ~9.5k-char footprint.
    assert!(
        related_chars < 2500,
        "Related Actions section chars grew unexpectedly: {related_chars}"
    );
}

#[test]
fn keepers_still_document_completion_and_enablement_contracts() {
    let tools = get_all_static_builtin_tools();
    let by_name: std::collections::HashMap<_, _> = tools
        .into_iter()
        .map(|t| (t.name.clone(), t.description))
        .collect();

    let present = by_name
        .get("presentInteractive")
        .expect("presentInteractive");
    assert!(present.contains(TOOL_RELATED_ACTIONS));
    assert!(present.contains("ui__reportResult"));

    let report = by_name.get("reportResult").expect("reportResult");
    assert!(report.contains(TOOL_RELATED_ACTIONS));
    assert!(report.contains("ui__presentInteractive"));

    let register = by_name.get("registerServer").expect("registerServer");
    assert!(register.contains(TOOL_RELATED_ACTIONS));
    assert!(register.contains("agent__updateAgent"));

    let list = by_name.get("listServers").expect("listServers");
    assert!(list.contains(TOOL_RELATED_ACTIONS));
    assert!(list.contains("active session tools stay fixed"));

    let prepare = by_name
        .get("prepareTeamworkWorkspace")
        .expect("prepareTeamworkWorkspace");
    assert!(prepare.contains(TOOL_RELATED_ACTIONS));
    assert!(prepare.contains("teamwork skill") || prepare.contains("init_task_force"));
}
