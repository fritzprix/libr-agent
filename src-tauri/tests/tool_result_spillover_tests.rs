use std::fs;

mod common;

use tauri_mcp_agent_lib::agent::tools::{
    spill_oversized_tool_result_messages, tool_result_preview_content_limit_bytes,
    TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
};
use tauri_mcp_agent_lib::mcp::types::{MCPContent, ServiceInfo};
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, SessionMetadata, SessionRepository, SessionStatus, SqliteMessageRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::get_session_manager;
use tauri_mcp_agent_lib::set_message_repository;
use tokio::sync::OnceCell;

static TEST_DB: OnceCell<sea_orm::DatabaseConnection> = OnceCell::const_new();

async fn test_db() -> sea_orm::DatabaseConnection {
    TEST_DB
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            set_message_repository(SqliteMessageRepository::new(db.clone()));
            db
        })
        .await
        .clone()
}

fn build_session_metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Spillover regression".to_string()),
        status,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        workspace_override: None,
    }
}

async fn load_persisted_message(session_id: &str) -> Message {
    let repo = SqliteMessageRepository::new(test_db().await);
    let page = repo
        .get_page(session_id, 1, 10)
        .await
        .expect("message query should succeed");
    page.items
        .into_iter()
        .next()
        .expect("expected at least one message for session")
}

fn make_tool_message(session_id: &str, tool_call_id: &str, text: &str) -> Message {
    Message {
        id: format!("message-{tool_call_id}"),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: Some(tool_call_id.to_string()),
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at: 0,
        updated_at: 0,
        source: Some(MessageSource::Tool),
        error: None,
        metadata: None,
    }
}

#[test]
fn tool_result_spillover_defaults_to_16kb_and_keeps_preview_headroom() {
    assert_eq!(TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES, 16 * 1024);
    assert_eq!(
        tool_result_preview_content_limit_bytes(TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES),
        14 * 1024
    );
}

#[tokio::test]
async fn spillover_pointer_is_what_gets_persisted_to_repository() {
    let db = test_db().await;
    let repo = SqliteMessageRepository::new(db.clone());
    let session_id = format!("spillover-persist-{}", uuid::Uuid::new_v4());
    let tool_call_id = "tool_call_large_persist";
    let original_text =
        "workflow tool output ".repeat((TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES / 21) + 200);

    let session_metadata = build_session_metadata(&session_id, SessionStatus::Busy);
    let sqlite_session_repo = SqliteSessionRepository::new(db.clone());
    sqlite_session_repo
        .upsert_session(&session_metadata)
        .await
        .expect("session row should exist for message persistence");

    let original_message = Message {
        id: format!("message-{tool_call_id}"),
        session_id: session_id.clone(),
        role: "tool".to_string(),
        content: vec![
            MCPContent::Text {
                text: original_text.clone(),
                is_error: None,
            },
            MCPContent::Resource {
                resource: serde_json::json!({
                    "kind": "test-ui-resource",
                    "title": "UI payload should survive spillover"
                }),
                service_info: ServiceInfo {
                    server_name: "builtin.ui".to_string(),
                    tool_name: "presentInteractive".to_string(),
                    backend_type: "BuiltInRust".to_string(),
                },
            },
        ],
        tool_calls: None,
        tool_call_id: Some(tool_call_id.to_string()),
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at: 0,
        updated_at: 0,
        source: Some(MessageSource::Tool),
        error: None,
        metadata: None,
    };

    let processed = spill_oversized_tool_result_messages(&session_id, vec![original_message])
        .await
        .expect("spillover rewrite should succeed");
    let rewritten_message = &processed[0];

    let MCPContent::Text { text, .. } = &rewritten_message.content[0] else {
        panic!("first tool content should be pointer text");
    };
    assert!(
        text.contains("output truncated"),
        "rewritten message should contain a truncation notice"
    );
    assert!(
        text.contains("readFile({\"path\":"),
        "spillover notice should tell the agent how to inspect the spillover file"
    );
    assert!(
        text.contains("Do not call `readFile({\"path\":"),
        "spillover notice should warn against rereading the saved file without a line range"
    );
    assert!(
        text.contains("To continue after the inline preview"),
        "spillover notice should give an explicit follow-up command after the preview"
    );
    assert!(
        text.contains(&original_text[..128]),
        "rewritten message should keep a preview of the original tool output"
    );
    assert!(
        text.len() < TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        "rewritten spillover preview should stay below the spillover threshold"
    );
    assert!(
        matches!(rewritten_message.content[1], MCPContent::Resource { .. }),
        "non-text UI content must survive the spillover rewrite unchanged"
    );

    let start = text.find('`').expect("path opening backtick") + 1;
    let end = text[start..]
        .find('`')
        .map(|offset| start + offset)
        .expect("path closing backtick");
    let relative_path = &text[start..end];

    let workspace_dir = get_session_manager()
        .expect("session manager")
        .get_session_workspace_dir_by_id(&session_id);
    let spilled_file = workspace_dir.join(relative_path);
    let spilled_text = fs::read_to_string(&spilled_file).expect("spilled file should exist");
    assert_eq!(spilled_text, original_text);

    repo.insert(rewritten_message)
        .await
        .expect("rewritten tool message should persist");
    let persisted_message = load_persisted_message(&session_id).await;
    let MCPContent::Text {
        text: persisted_text,
        ..
    } = &persisted_message.content[0]
    else {
        panic!("persisted message should keep the pointer text");
    };
    assert_eq!(
        persisted_text, text,
        "db persistence should receive the rewritten pointer text"
    );
    assert!(
        matches!(persisted_message.content[1], MCPContent::Resource { .. }),
        "db persistence must keep non-text UI content alongside the spillover pointer"
    );

    let _ = fs::remove_dir_all(workspace_dir);
}

#[tokio::test]
async fn tool_result_spillover_writes_large_tool_output_to_workspace_file() {
    let session_id = format!("spillover-test-{}", uuid::Uuid::new_v4());
    let original_text =
        "large tool output ".repeat((TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES / 18) + 200);

    let processed = spill_oversized_tool_result_messages(
        &session_id,
        vec![make_tool_message(
            &session_id,
            "tool_call_large",
            &original_text,
        )],
    )
    .await
    .expect("spillover should succeed");

    let message = &processed[0];
    let MCPContent::Text { text, .. } = &message.content[0] else {
        panic!("expected text content");
    };

    assert!(text.contains("output truncated"));
    assert!(text.contains(".libragent/tool-results/"));
    assert!(text.contains("readFile({\"path\":"));
    assert!(
        text.contains(&original_text[..128]),
        "spillover output should preserve a visible preview"
    );
    assert!(
        text.len() < TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        "spillover preview should stay below the inline threshold"
    );

    let start = text.find('`').expect("path opening backtick") + 1;
    let end = text[start..]
        .find('`')
        .map(|offset| start + offset)
        .expect("path closing backtick");
    let relative_path = &text[start..end];

    let session_manager = get_session_manager().expect("session manager");
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
    let spilled_file = workspace_dir.join(relative_path);

    let spilled_text = fs::read_to_string(&spilled_file).expect("spilled file should exist");
    assert_eq!(spilled_text, original_text);

    let _ = fs::remove_dir_all(workspace_dir);
}

#[tokio::test]
async fn tool_result_spillover_leaves_small_tool_output_inline() {
    let session_id = format!("spillover-test-{}", uuid::Uuid::new_v4());
    let original_text = "small tool output";

    let processed = spill_oversized_tool_result_messages(
        &session_id,
        vec![make_tool_message(
            &session_id,
            "tool_call_small",
            original_text,
        )],
    )
    .await
    .expect("small tool output should remain inline");

    let message = &processed[0];
    let MCPContent::Text { text, .. } = &message.content[0] else {
        panic!("expected text content");
    };

    assert_eq!(text, original_text);
}
