use crate::common;
use serde_json::json;
use std::fs;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::ui::UiServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::services::workspace_service::WorkspaceService;
use tauri_mcp_agent_lib::session::prepare_teamwork_artifact_dir_for_session;

fn make_session(
    session_id: &str,
    parent_id: Option<&str>,
    org_root_id: Option<&str>,
) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Teamwork Preview Session".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: parent_id.map(String::from),
        lineage_id: Some(session_id.to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: org_root_id.map(|_| "org-123".to_string()),
        org_name: org_root_id.map(|_| "Test Org".to_string()),
        org_root_session_id: org_root_id.map(String::from),
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

#[tokio::test]
async fn test_teamwork_path_resolution_and_preview() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());
    tauri_mcp_agent_lib::set_session_repository(repo.clone());

    let session_manager = tauri_mcp_agent_lib::session::get_session_manager().unwrap();

    let root_id = "org-root-session";
    let child_id = "child-session";

    let root_session = make_session(root_id, None, Some(root_id));
    repo.upsert_session(&root_session).await.unwrap();

    let child_session = make_session(child_id, Some(root_id), Some(root_id));
    repo.upsert_session(&child_session).await.unwrap();

    // Prepare teamwork artifact directory for root session
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(session_manager, root_id)
        .await
        .unwrap();

    // Create a teamwork document
    let docs_dir = artifact_dir.join("docs");
    fs::create_dir_all(&docs_dir).unwrap();
    let mastersheet_path = docs_dir.join("VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md");
    fs::write(
        &mastersheet_path,
        "# Monetization Mastersheet\n\n- Revenue: $1,000,000\n",
    )
    .unwrap();

    // 1. Resolve path using @teamwork prefix
    let resolved = WorkspaceService::resolve_path_for_session(
        child_id,
        "@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md",
    )
    .await
    .expect("Should resolve @teamwork alias for child session");
    assert_eq!(resolved, mastersheet_path.canonicalize().unwrap());

    // 2. Resolve path using .libragent/teamwork prefix
    let resolved_lib = WorkspaceService::resolve_path_for_session(
        child_id,
        ".libragent/teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md",
    )
    .await
    .expect("Should resolve .libragent/teamwork alias for child session");
    assert_eq!(resolved_lib, mastersheet_path.canonicalize().unwrap());

    // 3. Resolve path with leading slash /@teamwork/
    let resolved_slash = WorkspaceService::resolve_path_for_session(
        child_id,
        "/@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md",
    )
    .await
    .expect("Should resolve /@teamwork alias for child session");
    assert_eq!(resolved_slash, mastersheet_path.canonicalize().unwrap());

    // 4. Test WorkspaceService::read_file_content with @teamwork alias
    let content_res = WorkspaceService::read_file_content(
        "@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md".to_string(),
        Some(child_id.to_string()),
    )
    .await
    .expect("WorkspaceService::read_file_content should succeed for @teamwork");

    assert!(!content_res.is_binary);
    assert_eq!(content_res.mime_type, "text/markdown");
    assert!(content_res
        .content
        .contains("# Monetization Mastersheet\n\n- Revenue: $1,000,000"));

    // 5. Traversal attempt out of teamwork dir should be blocked
    let traversal_err =
        WorkspaceService::resolve_path_for_session(child_id, "@teamwork/../../outside.txt").await;
    assert!(
        traversal_err.is_err(),
        "Traversal outside teamwork root must be rejected"
    );
}

#[tokio::test]
async fn test_ui_report_result_with_teamwork_deliverables() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());
    tauri_mcp_agent_lib::set_session_repository(repo.clone());

    let session_manager = tauri_mcp_agent_lib::session::get_session_manager().unwrap();

    let root_id = "org-root-session-report";
    let root_session = make_session(root_id, None, Some(root_id));
    repo.upsert_session(&root_session).await.unwrap();

    // Prepare teamwork artifact directory
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(session_manager, root_id)
        .await
        .unwrap();

    let docs_dir = artifact_dir.join("docs");
    fs::create_dir_all(&docs_dir).unwrap();
    let file_path = docs_dir.join("VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md");
    fs::write(&file_path, "# Strategy\n\nFull details.\n").unwrap();

    let server = UiServer::new();
    let result = server
        .call_tool(
            "reportResult",
            json!({
                "title": "Completed Analysis",
                "status": "success",
                "result": "Here is the master sheet: [@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md](@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md)",
                "export_paths": [
                    "@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md"
                ]
            }),
            Some(root_id.to_string()),
        )
        .await
        .expect("reportResult should succeed");

    let structured = result
        .structured_content
        .as_ref()
        .expect("should have structured_content");

    let deliverables = structured["deliverables"]
        .as_array()
        .expect("should have deliverables array");
    assert_eq!(deliverables.len(), 1);

    let item = &deliverables[0];
    assert_eq!(
        item["path"].as_str(),
        Some("@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md")
    );
    assert_eq!(
        item["name"].as_str(),
        Some("VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md")
    );
    assert_eq!(item["extension"].as_str(), Some("md"));
    assert_eq!(item["exists"].as_bool(), Some(true));
    assert!(item["size_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(item["absolute_path"].as_str().is_some());
}

#[tokio::test]
async fn test_zip_export_with_teamwork_and_workspace_files() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());
    tauri_mcp_agent_lib::set_session_repository(repo.clone());

    let session_manager = tauri_mcp_agent_lib::session::get_session_manager().unwrap();

    let root_id = "org-root-export-zip";
    let root_session = make_session(root_id, None, Some(root_id));
    repo.upsert_session(&root_session).await.unwrap();

    let ws_dir = session_manager.get_session_workspace_dir_by_id(root_id);
    fs::create_dir_all(&ws_dir).unwrap();
    let readme_path = ws_dir.join("README.md");
    fs::write(&readme_path, "# Test Project\n").unwrap();

    // Prepare teamwork artifact directory
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(session_manager, root_id)
        .await
        .unwrap();

    let docs_dir = artifact_dir.join("docs");
    fs::create_dir_all(&docs_dir).unwrap();
    let mastersheet_path = docs_dir.join("VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md");
    fs::write(
        &mastersheet_path,
        "# Monetization Mastersheet\n\n- Projected ROI: 500%\n",
    )
    .unwrap();

    // 1. Test FileExportService::create_zip_export
    use tauri_mcp_agent_lib::services::file_export_service::FileExportService;
    let export_result = FileExportService::create_zip_export(
        root_id,
        vec![
            "@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md".to_string(),
            "README.md".to_string(),
        ],
        "marketing_bundle",
    )
    .await
    .expect("create_zip_export should succeed with mixed workspace and teamwork files");

    assert_eq!(export_result.file_count, Some(2));
    assert!(export_result.filename.starts_with("marketing_bundle_"));
    assert!(export_result.filename.ends_with(".zip"));

    let cursor = std::io::Cursor::new(export_result.content);
    let mut zip_archive = zip::ZipArchive::new(cursor).expect("Should parse generated ZIP archive");

    // Verify file names inside ZIP
    {
        let entry_teamwork = zip_archive
            .by_name("@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md")
            .expect("@teamwork file should be present in ZIP");
        assert_eq!(
            entry_teamwork.size(),
            fs::metadata(&mastersheet_path).unwrap().len()
        );
    }

    {
        let entry_readme = zip_archive
            .by_name("README.md")
            .expect("README.md should be present in ZIP");
        assert_eq!(
            entry_readme.size(),
            fs::metadata(&readme_path).unwrap().len()
        );
    }

    // 2. Test WorkspaceServer MCP handle_export with ZIP package
    use std::sync::Arc;
    use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
    let server = WorkspaceServer::new(root_id.to_string(), Arc::new(session_manager.clone()));

    let mcp_res = server
        .call_tool(
            "export",
            json!({
                "paths": [
                    "@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md",
                    "README.md"
                ],
                "name": "mcp_export_bundle"
            }),
            Some(root_id.to_string()),
        )
        .await
        .expect("MCP export tool should succeed");

    assert!(!mcp_res.is_error.unwrap_or(false));
    let content_text = match &mcp_res.content.as_ref().expect("content expected")[0] {
        tauri_mcp_agent_lib::mcp::types::MCPContent::Text { text } => text,
        _ => panic!("Expected text content"),
    };
    assert!(content_text.contains("ZIP package 'mcp_export_bundle' created successfully"));
    assert!(content_text.contains("Contains 2 files"));

    // Check that the generated zip file in .libragent/exports/packages exists and has the entries
    let exports_packages = ws_dir.join(".libragent").join("exports").join("packages");
    let zip_entry = fs::read_dir(exports_packages)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("mcp_export_bundle_")
        })
        .expect("Generated ZIP file should exist");

    let file = fs::File::open(zip_entry.path()).unwrap();
    let mut mcp_zip = zip::ZipArchive::new(file).unwrap();
    assert!(mcp_zip
        .by_name("@teamwork/docs/VIRTUAL-INFLUENCER-MONETIZATION-MASTERSHEET.md")
        .is_ok());
    assert!(mcp_zip.by_name("README.md").is_ok());
}
