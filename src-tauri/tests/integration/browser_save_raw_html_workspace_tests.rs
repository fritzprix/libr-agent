//! saveRawHtml must land in the agent session workspace so workspace__readFile
//! can open the advertised extracted-content path (Harbor Docker-attach).

use tauri_mcp_agent_lib::mcp::builtin::browser::save_raw_html_to_file;
use tauri_mcp_agent_lib::session::get_session_manager;

#[tokio::test]
async fn save_raw_html_writes_under_agent_session_workspace() {
    let agent_session_id = "test-saverawhtml-session-workspace";
    let browser_session_id = "BROWSE01";
    let raw_html = "<html><body>gate</body></html>";

    let relative_path = save_raw_html_to_file(agent_session_id, browser_session_id, raw_html)
        .await
        .expect("save_raw_html_to_file should succeed");

    assert!(
        relative_path.starts_with("extracted-content/extracted-BROWSE01-"),
        "unexpected relative path: {relative_path}"
    );
    assert!(
        relative_path.ends_with(".html"),
        "unexpected relative path: {relative_path}"
    );

    let workspace_dir = get_session_manager()
        .expect("session manager")
        .get_session_workspace_dir_by_id(agent_session_id);
    let host_path = workspace_dir.join(&relative_path);
    assert!(
        host_path.is_file(),
        "raw HTML should be readable under the agent session workspace at {:?}",
        host_path
    );
    let saved = std::fs::read_to_string(&host_path).expect("read saved html");
    assert_eq!(saved, raw_html);

    let _ = std::fs::remove_file(&host_path);
    if let Some(parent) = host_path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}
