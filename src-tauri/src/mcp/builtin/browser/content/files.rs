use crate::mcp::builtin::browser::handle_browser_op_error;
use crate::mcp::types::MCPResult;
use chrono::Utc;
use serde_json::json;

/// Save raw HTML into the agent session workspace (and Docker-attach container).
///
/// Previously wrote under the global `SecureFileManager` root (`global_shared/`),
/// so Harbor Docker-attach trials could not `workspace__readFile` the advertised
/// `extracted-content/…` path. Uses the same session-scoped write pattern as
/// [`save_downloaded_file`], and additionally pushes into the attach container
/// when the agent session is Docker-attach (`save_downloaded_file` does not).
pub async fn save_raw_html_to_file(
    agent_session_id: &str,
    browser_session_id: &str,
    raw_html: &str,
) -> Result<String, String> {
    use crate::services::secure_file_manager::SecureFileManager;

    let session_manager = crate::session::get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(agent_session_id);
    let file_manager = SecureFileManager::new_scoped_with_base_dir(workspace_dir.clone());

    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let file_name = format!("extracted-{}-{}.html", browser_session_id, timestamp);
    let relative_path = {
        let p = std::path::PathBuf::from("extracted-content")
            .join(&file_name)
            .to_string_lossy()
            .to_string();
        #[cfg(target_os = "windows")]
        let p = p.replace('\\', "/");
        p
    };

    file_manager
        .write_file_string(&relative_path, raw_html)
        .await?;

    let host_path = workspace_dir.join(&relative_path);
    if let Some(session) =
        crate::services::container_attach_fs::load_session(agent_session_id).await?
    {
        crate::services::container_attach_fs::push_host_file_to_container(&session, &host_path)
            .await?;
    }

    Ok(relative_path)
}

pub(crate) async fn save_downloaded_file(
    url: &str,
    save_path: &str,
    session_id: Option<&str>,
    bytes: &[u8],
) -> Result<MCPResult, String> {
    let target_session_id = session_id.unwrap_or("default");
    let session_manager = crate::session::get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(target_session_id);
    let file_manager_state =
        crate::services::SecureFileManager::new_scoped_with_base_dir(workspace_dir);

    match file_manager_state.write_file(save_path, bytes).await {
        Ok(_) => {
            let result_text = format!("File successfully downloaded to: {}", save_path);
            let structured_data = json!({
                "url": url,
                "saved_path": save_path,
                "bytes_downloaded": bytes.len()
            });
            Ok(MCPResult::success_with_data(&result_text, structured_data))
        }
        Err(e) => Ok(handle_browser_op_error(
            "Save File",
            e.to_string(),
            vec!["Check the savePath and ensure it is valid within the workspace."],
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_download_workspace_isolation() {
        let url = "https://example.com/dummy.pdf";
        let save_path = "downloads/dummy.pdf";
        let dummy_bytes = b"dummy content";
        let test_session_id = "test-isolation-123";

        // Call our extracted function
        let result = save_downloaded_file(url, save_path, Some(test_session_id), dummy_bytes).await;

        assert!(result.is_ok(), "Function should return Ok");
        let mcp_res = result.unwrap();
        assert!(
            !mcp_res.is_error.unwrap_or(false),
            "MCPResult should not be an error"
        );

        // Verify via the actual session manager
        let actual_sm = crate::session::get_session_manager().unwrap();
        let expected_workspace_dir = actual_sm.get_session_workspace_dir_by_id(test_session_id);
        let expected_path = expected_workspace_dir.join(save_path);

        assert!(
            expected_path.exists(),
            "File should be saved to the isolated workspace at {:?}",
            expected_path
        );
        let saved_content = std::fs::read(&expected_path).unwrap();
        assert_eq!(saved_content, dummy_bytes);
    }
}
