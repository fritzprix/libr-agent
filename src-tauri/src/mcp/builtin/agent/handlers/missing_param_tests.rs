//! A handler called without a required argument returns guidance that names the argument,
//! not an internal error. The server has no session manager, so the handlers must check
//! their arguments before they need one.

use std::sync::Arc;

use sea_orm::Database;
use serde_json::{json, Value};

use super::{
    check_session, compact_session_context, create_org, delete_session, message_to_session,
    spawn_session, stop_session,
};
use crate::mcp::builtin::agent::AgentServer;
use crate::mcp::types::{MCPContent, MCPResult};

const CALLER: &str = "caller-session";

async fn server() -> AgentServer {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    AgentServer::new(CALLER.to_string(), Arc::new(db), None)
        .await
        .expect("server")
}

fn assert_missing_param(result: Result<MCPResult, String>, param: &str) {
    let result = result.expect("a missing argument must not be an internal error");
    assert_eq!(result.is_error, Some(true));
    let text: String = result
        .content
        .iter()
        .flatten()
        .filter_map(|item| match item {
            MCPContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains(&format!("'{param}'")),
        "message must name '{param}': {text}"
    );
    assert!(!text.to_lowercase().contains("system error"), "{text}");
}

#[tokio::test]
async fn check_session_without_session_id() {
    let server = server().await;
    assert_missing_param(check_session(&server, json!({}), CALLER).await, "sessionId");
}

#[tokio::test]
async fn create_org_without_name() {
    let server = server().await;
    assert_missing_param(create_org(&server, json!({}), CALLER).await, "name");
}

#[tokio::test]
async fn compact_session_context_without_session_id() {
    let server = server().await;
    assert_missing_param(
        compact_session_context(&server, json!({}), CALLER).await,
        "sessionId",
    );
}

#[tokio::test]
async fn stop_session_without_session_id() {
    let server = server().await;
    assert_missing_param(stop_session(&server, json!({}), CALLER).await, "sessionId");
}

#[tokio::test]
async fn delete_session_without_session_id() {
    let server = server().await;
    assert_missing_param(
        delete_session(&server, json!({}), CALLER).await,
        "sessionId",
    );
}

#[tokio::test]
async fn message_to_session_without_each_required_arg() {
    let server = server().await;
    let full = json!({ "sessionId": "s-1", "message": "hi", "complexity": "low" });
    for param in ["sessionId", "message", "complexity"] {
        let args = without(&full, param);
        assert_missing_param(message_to_session(&server, args, CALLER).await, param);
    }
}

#[tokio::test]
async fn spawn_session_without_each_required_arg() {
    let server = server().await;
    let full = json!({ "configId": "cfg", "task": "do it", "complexity": "low" });
    for param in ["configId", "task", "complexity"] {
        let args = without(&full, param);
        assert_missing_param(spawn_session(&server, args, CALLER).await, param);
    }
}

fn without(args: &Value, key: &str) -> Value {
    let mut args = args.clone();
    if let Some(map) = args.as_object_mut() {
        map.remove(key);
    }
    args
}
