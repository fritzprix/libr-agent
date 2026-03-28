mod common;

use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri_mcp_agent_lib::mcp::builtin::history::HistoryServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, SessionMetadata, SessionRepository, SessionStatus, SqliteMessageRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::{set_message_repository, set_session_repository};
use tokio::sync::{Mutex, OnceCell};

static TEST_DB: OnceCell<Arc<sea_orm::DatabaseConnection>> = OnceCell::const_new();
static TEST_GUARD: Mutex<()> = Mutex::const_new(());

fn extract_text_content(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn get_or_create_test_db() -> Arc<sea_orm::DatabaseConnection> {
    TEST_DB
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let unique_suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let db_path = std::env::temp_dir().join(format!(
                "libragent-history-builtins-{}-{}.sqlite",
                std::process::id(),
                unique_suffix
            ));
            let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
            let mut options = ConnectOptions::new(db_url);
            options.min_connections(1);
            options.max_connections(1);
            let db = Arc::new(
                Database::connect(options)
                    .await
                    .expect("history test database should connect"),
            );
            tauri_mcp_agent_lib::migration::Migrator::up(&*db, None)
                .await
                .expect("migrations should run");
            set_session_repository(SqliteSessionRepository::new((*db).clone()));
            set_message_repository(SqliteMessageRepository::new((*db).clone()));
            db
        })
        .await
        .clone()
}

async fn seed_history_fixture() -> Arc<sea_orm::DatabaseConnection> {
    let db = get_or_create_test_db().await;
    let session_repo = SqliteSessionRepository::new((*db).clone());
    let message_repo = SqliteMessageRepository::new((*db).clone());

    session_repo
        .upsert_session(&SessionMetadata {
            id: "history-session-a".to_string(),
            name: Some("History Session A".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(r#"{"assistantId":"agent-alpha"}"#.to_string()),
            parent_session_id: None,
            lineage_id: Some("lineage-alpha".to_string()),
            depth: Some(0),
            max_depth: None,
            max_fanout: None,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_100_000,
            last_viewed_at: None,
            last_message_at: Some(1_700_000_120_000),
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            workspace_override: None,
        })
        .await
        .expect("session A should upsert");

    session_repo
        .upsert_session(&SessionMetadata {
            id: "history-session-b".to_string(),
            name: Some("History Session B".to_string()),
            status: SessionStatus::Busy,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(r#"{"assistantId":"agent-beta"}"#.to_string()),
            parent_session_id: None,
            lineage_id: Some("lineage-beta".to_string()),
            depth: Some(0),
            max_depth: None,
            max_fanout: None,
            created_at: 1_700_001_000_000,
            updated_at: 1_700_001_100_000,
            last_viewed_at: None,
            last_message_at: Some(1_700_001_120_000),
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            workspace_override: None,
        })
        .await
        .expect("session B should upsert");

    message_repo
        .insert(&Message {
            id: "history-message-a1".to_string(),
            session_id: "history-session-a".to_string(),
            role: "user".to_string(),
            content: vec![MCPContent::Text {
                text: "Need batch knowledge extraction from the daily history.".to_string(),
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: Some("agent-alpha".to_string()),
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: 1_700_000_110_000,
            updated_at: 1_700_000_110_000,
            source: None,
            error: None,
            metadata: None,
        })
        .await
        .expect("message a1 should insert");

    message_repo
        .insert(&Message {
            id: "history-message-a2".to_string(),
            session_id: "history-session-a".to_string(),
            role: "assistant".to_string(),
            content: vec![MCPContent::Text {
                text: "Knowledge extraction summary: user wants persistent history search."
                    .to_string(),
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: Some("agent-alpha".to_string()),
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: 1_700_000_120_000,
            updated_at: 1_700_000_120_000,
            source: None,
            error: None,
            metadata: None,
        })
        .await
        .expect("message a2 should insert");

    message_repo
        .insert(&Message {
            id: "history-message-b1".to_string(),
            session_id: "history-session-b".to_string(),
            role: "assistant".to_string(),
            content: vec![MCPContent::Text {
                text: "Unrelated beta session content.".to_string(),
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: Some("agent-beta".to_string()),
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: 1_700_001_120_000,
            updated_at: 1_700_001_120_000,
            source: None,
            error: None,
            metadata: None,
        })
        .await
        .expect("message b1 should insert");

    let unicode_heavy_text = format!("HEADER {}{}", "A".repeat(93), "낮".repeat(100));
    message_repo
        .insert(&Message {
            id: "history-message-unicode".to_string(),
            session_id: "history-session-a".to_string(),
            role: "assistant".to_string(),
            content: vec![MCPContent::Text {
                text: unicode_heavy_text,
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: Some("agent-alpha".to_string()),
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: 1_700_000_125_000,
            updated_at: 1_700_000_125_000,
            source: None,
            error: None,
            metadata: None,
        })
        .await
        .expect("unicode message should insert");

    let large_text = "L".repeat(3_500);
    message_repo
        .insert(&Message {
            id: "history-message-large".to_string(),
            session_id: "history-session-a".to_string(),
            role: "tool".to_string(),
            content: vec![MCPContent::Text {
                text: large_text,
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: Some("agent-alpha".to_string()),
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: 1_700_000_130_000,
            updated_at: 1_700_000_130_000,
            source: None,
            error: None,
            metadata: None,
        })
        .await
        .expect("large message should insert");

    db
}

#[test]
fn history_list_tool_status_filter_has_no_default() {
    let list_tool = HistoryServer::tools_static()
        .into_iter()
        .find(|tool| tool.name == "list")
        .expect("list tool should exist");

    let properties = match &list_tool.input_schema.schema_type {
        JSONSchemaType::Object {
            properties: Some(properties),
            ..
        } => properties,
        other => panic!("expected object schema, got {other:?}"),
    };

    let status_schema = properties
        .get("status")
        .expect("list tool should expose status filter");
    assert!(
        status_schema.default.is_none(),
        "optional status filter must not imply a default value"
    );
}

#[tokio::test]
async fn history_list_filters_sessions_and_exposes_ids() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let result = server
        .call_tool(
            "list",
            json!({
                "agentId": "agent-alpha",
                "status": "idle",
                "page": 1,
                "pageSize": 10
            }),
            None,
        )
        .await
        .expect("list should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(text.contains("history-session-a"));
    assert!(!text.contains("history-session-b"));
    assert!(text.contains("Use readSession(sessionId=\"...\")"));

    let structured = result
        .structured_content
        .expect("structured content expected");
    assert_eq!(structured["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(structured["sessions"][0]["sessionId"], "history-session-a");
    assert_eq!(structured["sessions"][0]["messageCount"], 4);
}

#[tokio::test]
async fn history_read_session_and_message_are_paginated() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let read_session = server
        .call_tool(
            "readSession",
            json!({
                "sessionId": "history-session-a",
                "page": 1,
                "pageSize": 2
            }),
            None,
        )
        .await
        .expect("readSession should succeed");

    let structured = read_session
        .structured_content
        .expect("structured content expected");
    assert_eq!(structured["session"]["sessionId"], "history-session-a");
    assert_eq!(structured["messages"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(structured["messages"]["totalItems"], 4);

    let read_message = server
        .call_tool(
            "readMessage",
            json!({
                "messageId": "history-message-large",
                "offsetChars": 0,
                "maxChars": 4000
            }),
            None,
        )
        .await
        .expect("readMessage should succeed");

    let structured = read_message
        .structured_content
        .expect("structured content expected");
    assert_eq!(structured["message"]["chunkLength"], 3000);
    assert_eq!(structured["message"]["hasMore"], true);
    assert_eq!(structured["message"]["nextOffset"], 3000);
    assert_eq!(
        structured["message"]["contentChunk"]
            .as_str()
            .expect("chunk text"),
        "L".repeat(3000)
    );
}

#[tokio::test]
async fn history_search_returns_filtered_snippets() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let result = server
        .call_tool(
            "search",
            json!({
                "query": "knowledge extraction summary",
                "agentId": "agent-alpha",
                "roles": ["assistant"],
                "page": 1,
                "pageSize": 10
            }),
            Some("history-session-a".to_string()),
        )
        .await
        .expect("search should succeed");

    let text = extract_text_content(&result);
    assert!(text.contains("history-message-a2"));
    assert!(!text.contains("history-message-b1"));
    assert!(text.contains("session=history-session-a"));
    assert!(text.contains("Use readSession(sessionId=\"...\")"));
    assert!(text.contains("Use readMessage(messageId=\"...\")"));

    let structured = result
        .structured_content
        .expect("structured content expected");
    let matches = structured["matches"].as_array().expect("matches array");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["sessionId"], "history-session-a");
    assert_eq!(matches[0]["messageId"], "history-message-a2");
    assert_eq!(matches[0]["role"], "assistant");
    assert!(matches[0]["snippet"]
        .as_str()
        .expect("snippet")
        .contains("Knowledge extraction summary"));
}

#[tokio::test]
async fn history_list_is_stably_sorted_for_pagination() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let first_page = server
        .call_tool(
            "list",
            json!({
                "page": 1,
                "pageSize": 1
            }),
            None,
        )
        .await
        .expect("first list page should succeed");
    let first_structured = first_page
        .structured_content
        .expect("structured content expected");
    assert_eq!(
        first_structured["sessions"][0]["sessionId"],
        "history-session-b"
    );

    let second_page = server
        .call_tool(
            "list",
            json!({
                "page": 2,
                "pageSize": 1
            }),
            None,
        )
        .await
        .expect("second list page should succeed");
    let second_structured = second_page
        .structured_content
        .expect("structured content expected");
    assert_eq!(
        second_structured["sessions"][0]["sessionId"],
        "history-session-a"
    );
}

#[tokio::test]
async fn history_search_handles_multibyte_snippets_without_panic() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let result = server
        .call_tool(
            "search",
            json!({
                "query": "HEADER",
                "agentId": "agent-alpha",
                "roles": ["assistant"],
                "page": 1,
                "pageSize": 10
            }),
            Some("history-session-a".to_string()),
        )
        .await
        .expect("search should succeed");

    assert_eq!(result.is_error, Some(false));
    let structured = result
        .structured_content
        .expect("structured content expected");
    let matches = structured["matches"].as_array().expect("matches array");
    assert!(
        matches
            .iter()
            .any(|item| item["messageId"] == "history-message-unicode"),
        "unicode message should appear in search matches"
    );
    let unicode_match = matches
        .iter()
        .find(|item| item["messageId"] == "history-message-unicode")
        .expect("unicode match");
    assert!(unicode_match["snippet"]
        .as_str()
        .expect("snippet")
        .contains('낮'));
}

#[tokio::test]
async fn history_search_reports_missing_session_as_not_found() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let result = server
        .call_tool(
            "search",
            json!({
                "query": "history",
                "sessionId": "missing-session"
            }),
            Some("history-session-a".to_string()),
        )
        .await
        .expect("search should return an MCP error result");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(text.contains("Session 'missing-session' not found"));
    assert!(!text.contains("did not match the provided filters"));
}

#[tokio::test]
async fn history_read_responses_keep_follow_up_ids_in_text() {
    let _guard = TEST_GUARD.lock().await;
    let db = seed_history_fixture().await;
    let server = HistoryServer::new("history-session-a".to_string(), db)
        .await
        .expect("server should initialize");

    let read_session = server
        .call_tool(
            "readSession",
            json!({
                "sessionId": "history-session-a",
                "page": 1,
                "pageSize": 2
            }),
            None,
        )
        .await
        .expect("readSession should succeed");
    let read_session_text = extract_text_content(&read_session);
    assert!(read_session_text.contains("history-session-a"));
    assert!(read_session_text.contains("history-message-a1"));
    assert!(read_session_text.contains("history-message-a2"));
    assert!(read_session_text.contains("Use readMessage(messageId=\"...\")"));

    let read_message = server
        .call_tool(
            "readMessage",
            json!({
                "messageId": "history-message-large",
                "offsetChars": 0,
                "maxChars": 3000
            }),
            None,
        )
        .await
        .expect("readMessage should succeed");
    let read_message_text = extract_text_content(&read_message);
    assert!(read_message_text.contains("history-message-large"));
    assert!(read_message_text.contains("history-session-a"));
    assert!(read_message_text.contains("Next offset: 3000"));
    assert!(read_message_text.contains(
        "Use readMessage(messageId=\"history-message-large\", offsetChars=3000) for the next chunk",
    ));
}
