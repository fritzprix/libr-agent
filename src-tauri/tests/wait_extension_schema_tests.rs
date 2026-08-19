//! Windows-safe coverage for `x-libragent-wait` tool schema extension.

use tauri_mcp_agent_lib::mcp::builtin::agent::tools::all_tools as agent_tools;
use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::terminal_tools::create_wait_for_process_tool;
use tauri_mcp_agent_lib::mcp::wait_extension::{
    check_session_wait_extension_json, wait_for_process_wait_extension_json,
};

#[test]
fn check_session_tool_declares_wait_extension() {
    let tool = agent_tools()
        .into_iter()
        .find(|tool| tool.name == "checkSession")
        .expect("checkSession tool");

    let extension = tool
        .libragent_wait
        .as_ref()
        .expect("checkSession must declare x-libragent-wait");

    assert_eq!(extension.resource_id_param, "sessionId");
    assert_eq!(extension.wait_param.as_deref(), Some("wait"));
    assert_eq!(extension.snapshot_mode.wait, Some(false));
    assert_eq!(extension.blocking_mode.wait, Some(true));

    let serialized = serde_json::to_value(&tool).expect("serialize tool");
    assert_eq!(
        serialized.get("x-libragent-wait"),
        Some(&check_session_wait_extension_json())
    );
}

#[test]
fn wait_for_process_tool_declares_wait_extension() {
    let tool = create_wait_for_process_tool();
    let extension = tool
        .libragent_wait
        .as_ref()
        .expect("waitForProcess must declare x-libragent-wait");

    assert_eq!(extension.resource_id_param, "processId");
    assert!(extension.wait_param.is_none());
    assert_eq!(extension.snapshot_mode.timeout, Some(0));

    let serialized = serde_json::to_value(&tool).expect("serialize tool");
    assert_eq!(
        serialized.get("x-libragent-wait"),
        Some(&wait_for_process_wait_extension_json())
    );
}
