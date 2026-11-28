//! The Rust LLM executor (#2704) answering requests with no WebView, against a
//! loopback OpenAI-compatible server.
//!
//! `configure` pins the switch, the endpoint, the "no WebView" fact and the
//! delivery target for each test, so nothing reads or sets the process
//! environment.
//!
//! Limit: the session handlers (`handle_llm_response`, tool execution,
//! settlement) clone the app handle, and a mock-runtime handle reinterpreted as
//! the `Wry` one cannot be cloned (it crashes). So results are delivered to a
//! recording sink here, and the session loop itself is covered by its own tests.

use crate::common;

use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use warp::Filter;

use indexmap::IndexMap;
use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::events::AgentEvent;
use tauri_mcp_agent_lib::agent::llm::rust_executor::{
    cancel, configure, dispatch_compaction, dispatch_completion, Endpoint, ExecutorConfig,
    ResponseSink,
};
use tauri_mcp_agent_lib::agent::llm::types::{
    AgentRuntimeError, CompactRequest, CompactionParentRequest, CompletionRequest,
};
use tauri_mcp_agent_lib::agent::session_manager::CompactStreamMeta;
use tauri_mcp_agent_lib::mcp::schema::{JSONSchema, JSONSchemaType};
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPTool};
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{SettingsRepository, SqliteSettingsRepository};
use tauri_mcp_agent_lib::set_settings_repository;
use tokio::sync::Mutex as AsyncMutex;

static TEST_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());

const PROVIDER: &str = "custom:mock";
const WAIT: Duration = Duration::from_secs(20);

/// A loopback server that answers from a script and keeps every request body.
struct MockProvider {
    endpoint: Endpoint,
    requests: Arc<Mutex<Vec<Value>>>,
}

fn sse(chunks: &[Value]) -> warp::http::Response<String> {
    let mut body: String = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect();
    body.push_str("data: [DONE]\n\n");
    warp::http::Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap()
}

/// `replies[n]` answers the n-th request; the last one repeats after that.
async fn mock_provider(replies: Vec<Vec<Value>>) -> MockProvider {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let counter = Arc::new(AtomicUsize::new(0));
    let route = warp::post()
        .and(warp::path!("v1" / "chat" / "completions"))
        .and(warp::body::json())
        .map(move |body: Value| {
            seen.lock().unwrap().push(body);
            let index = counter
                .fetch_add(1, Ordering::SeqCst)
                .min(replies.len() - 1);
            sse(&replies[index])
        });
    let (addr, server): (SocketAddr, _) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);
    MockProvider {
        endpoint: Endpoint {
            base_url: format!("http://{addr}/v1"),
            api_key: None,
        },
        requests,
    }
}

fn text_reply(text: &str) -> Vec<Value> {
    vec![
        json!({"choices":[{"delta":{"content":text},"finish_reason":"stop"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":4,"total_tokens":16}}),
    ]
}

fn empty_reply() -> Vec<Value> {
    vec![
        json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":1,"completion_tokens":0,"total_tokens":1}}),
    ]
}

/// First `fail_count` POSTs get `status`; later ones get `reply` as SSE.
async fn fail_then_sse(fail_count: usize, status: u16, reply: Vec<Value>) -> MockProvider {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let counter = Arc::new(AtomicUsize::new(0));
    let route = warp::post()
        .and(warp::path!("v1" / "chat" / "completions"))
        .and(warp::body::json())
        .map(move |body: Value| {
            seen.lock().unwrap().push(body);
            let index = counter.fetch_add(1, Ordering::SeqCst);
            if index < fail_count {
                return warp::http::Response::builder()
                    .status(status)
                    .header("content-type", "text/plain")
                    .body("provider exploded".to_string())
                    .unwrap();
            }
            sse(&reply)
        });
    let (addr, server): (SocketAddr, _) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);
    MockProvider {
        endpoint: Endpoint {
            base_url: format!("http://{addr}/v1"),
            api_key: None,
        },
        requests,
    }
}

/// A loopback server that records POST /v1/messages (the Anthropic Messages
/// path, `{base_url}/messages`) and answers with an empty Messages body.
async fn mock_anthropic_messages() -> (Endpoint, Arc<Mutex<Vec<Value>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let route = warp::post()
        .and(warp::path!("v1" / "messages"))
        .and(warp::body::json())
        .map(move |body: Value| {
            seen.lock().unwrap().push(body);
            warp::reply::json(&json!({"content": []}))
        });
    let (addr, server): (SocketAddr, _) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);
    (
        Endpoint {
            base_url: format!("http://{addr}/v1"),
            api_key: None,
        },
        requests,
    )
}

async fn install_settings(pairs: &[(&str, Value)]) -> SqliteSettingsRepository {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSettingsRepository::new(db);
    for (key, value) in pairs {
        repo.set(key, value.clone()).await.expect("set setting");
    }
    set_settings_repository(repo.clone());
    repo
}

/// A loopback server that always answers POST /v1/chat/completions with
/// `status`, and keeps the request bodies it saw.
async fn failing_provider(status: u16) -> (Endpoint, Arc<Mutex<Vec<Value>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let route = warp::post()
        .and(warp::path!("v1" / "chat" / "completions"))
        .and(warp::body::json())
        .map(move |body: Value| {
            seen.lock().unwrap().push(body);
            warp::reply::with_status(
                "provider exploded".to_string(),
                warp::http::StatusCode::from_u16(status).unwrap(),
            )
        });
    let (addr, server): (SocketAddr, _) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);
    (
        Endpoint {
            base_url: format!("http://{addr}/v1"),
            api_key: None,
        },
        requests,
    )
}

fn tool_call_reply(name: &str) -> Vec<Value> {
    vec![
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":name,"arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13}}),
    ]
}

/// Records what the executor delivers.
#[derive(Default)]
struct RecordingSink {
    responses: Mutex<Vec<Message>>,
    stream_events: Mutex<Vec<AgentEvent>>,
    llm_errors: Mutex<Vec<AgentRuntimeError>>,
    compactions: Mutex<Vec<(String, CompactStreamMeta)>>,
    compact_errors: Mutex<Vec<String>>,
    fallback_notices: Mutex<Vec<AgentEvent>>,
    /// Names in delivery order, so a test can check what was emitted first.
    events: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ResponseSink for RecordingSink {
    fn llm_stream_delta(&self, event: AgentEvent) {
        self.stream_events.lock().unwrap().push(event);
    }
    fn llm_stream_reset(&self, event: AgentEvent) {
        // Same ordered list as the deltas, so a test can check that a reset
        // falls between the discarded attempt and the retry's answer.
        self.stream_events.lock().unwrap().push(event);
    }
    fn llm_fallback_notice(&self, event: AgentEvent) {
        self.fallback_notices.lock().unwrap().push(event);
        self.events
            .lock()
            .unwrap()
            .push("llm_fallback_notice".to_string());
    }
    async fn llm_response(&self, _session_id: String, message: Message) -> Result<(), String> {
        self.events.lock().unwrap().push("llm_response".to_string());
        self.responses.lock().unwrap().push(message);
        Ok(())
    }
    async fn llm_error(&self, _session_id: String, error: AgentRuntimeError) -> Result<(), String> {
        self.llm_errors.lock().unwrap().push(error);
        Ok(())
    }
    async fn compact_response(
        &self,
        _session_id: &str,
        _request: &CompactRequest,
        summary: String,
        meta: CompactStreamMeta,
    ) -> Result<(), String> {
        self.compactions.lock().unwrap().push((summary, meta));
        Ok(())
    }
    async fn compact_error(
        &self,
        _session_id: String,
        error: AgentRuntimeError,
    ) -> Result<(), String> {
        self.compact_errors
            .lock()
            .unwrap()
            .push(error.display_message);
        Ok(())
    }
}

async fn wait_for(what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for {what}");
}

fn message(session_id: &str, id: &str, role: &str, text: &str, created_at: i64) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
}

fn text_of(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn request(
    session_id: &str,
    response_id: &str,
    provider: &str,
    messages: Vec<Message>,
) -> CompletionRequest {
    CompletionRequest {
        session_id: session_id.to_string(),
        response_message_id: response_id.to_string(),
        messages,
        model: "mock-model".to_string(),
        provider: provider.to_string(),
        system_prompt: Some("rust executor test".to_string()),
        max_tokens: None,
        available_tools: None,
    }
}

fn app() -> &'static tauri::AppHandle {
    let mock_app: &'static tauri::App<MockRuntime> = Box::leak(Box::new(tauri::test::mock_app()));
    // Only borrowed, never cloned or used for state: see the module docs.
    unsafe {
        &*(mock_app.handle() as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    }
}

fn config(endpoint: Endpoint) -> ExecutorConfig {
    ExecutorConfig {
        enabled: true,
        endpoint_override: Some(endpoint),
        assume_no_webview: true,
    }
}

/// The delivery target a test hands to `dispatch_*` in place of the session
/// manager that production finds in app state.
fn deliver_to(sink: &Arc<RecordingSink>) -> impl FnOnce() -> Option<Arc<dyn ResponseSink>> {
    let sink: Arc<dyn ResponseSink> = sink.clone();
    move || Some(sink)
}

/// A tool-call turn reaches the session as an assistant turn, and the tool
/// result goes back to the provider paired with that call.
#[tokio::test]
async fn a_tool_call_turn_is_delivered_and_its_result_is_sent_back_paired() {
    let _guard = TEST_GUARD.lock().await;
    let session_id = "rust-executor-tool-turn";
    let provider = mock_provider(vec![
        tool_call_reply("playbook__listPlaybooks"),
        text_reply("All done."),
    ])
    .await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let handle = app();
    let user = message(session_id, "u1", "user", "list the playbooks", 1_000);

    assert!(dispatch_completion(
        handle,
        &request(session_id, "resp-1", PROVIDER, vec![user.clone()]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the tool-call turn", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;
    let turn = sink.responses.lock().unwrap()[0].clone();
    assert_eq!(turn.id, "resp-1");
    let calls = turn.tool_calls.clone().expect("a tool call");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "playbook__listPlaybooks");

    // The session runs the tool and asks again with the result in the history.
    let mut result = message(session_id, "t1", "tool", "[]", 2_000);
    result.tool_call_id = Some(calls[0].id.clone());
    let follow_up = request(session_id, "resp-2", PROVIDER, vec![user, turn, result]);
    assert!(dispatch_completion(handle, &follow_up, deliver_to(&sink))
        .await
        .unwrap());
    wait_for("the final answer", || {
        sink.responses.lock().unwrap().len() == 2
    })
    .await;

    assert_eq!(text_of(&sink.responses.lock().unwrap()[1]), "All done.");
    let requests = provider.requests.lock().unwrap();
    let second = requests[1]["messages"].as_array().unwrap();
    let tool_message = second
        .iter()
        .find(|m| m["role"] == "tool")
        .expect("the tool result is sent back");
    assert_eq!(tool_message["tool_call_id"], calls[0].id.as_str());
}

/// The turn streams out as ordered deltas for the session and response, and
/// the delivered message is the same answer the deltas add up to.
#[tokio::test]
async fn a_turn_streams_ordered_deltas_before_its_final_message() {
    let _guard = TEST_GUARD.lock().await;
    let session_id = "rust-executor-stream";
    let provider = mock_provider(vec![vec![
        json!({"choices":[{"delta":{"reasoning_content":"Think. "}}]}),
        json!({"choices":[{"delta":{"reasoning_content":"Done."}}]}),
        json!({"choices":[{"delta":{"content":"Hel"}}]}),
        json!({"choices":[{"delta":{"content":"lo"},"finish_reason":"stop"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}),
    ]])
    .await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message(session_id, "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request(session_id, "resp-stream", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the streamed turn", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    let events = sink.stream_events.lock().unwrap();
    let mut thinking = String::new();
    let mut content = String::new();
    let mut last_phase = 0;
    for event in events.iter() {
        let AgentEvent::LlmStreamDelta {
            session_id: event_session,
            response_message_id,
            provider: event_provider,
            delta,
            ..
        } = event
        else {
            panic!("only stream deltas are expected: {event:?}");
        };
        assert_eq!(event_session, session_id);
        assert_eq!(response_message_id, "resp-stream");
        assert_eq!(event_provider, PROVIDER);
        if let Some(text) = &delta.thinking {
            assert_eq!(last_phase, 0, "reasoning comes before the answer");
            thinking.push_str(text);
        }
        if let Some(text) = &delta.content {
            last_phase = 1;
            content.push_str(text);
        }
    }
    assert_eq!(thinking, "Think. Done.");
    assert_eq!(content, "Hello");
    let delivered = sink.responses.lock().unwrap()[0].clone();
    assert_eq!(text_of(&delivered), content);
    assert_eq!(delivered.thinking.as_deref(), Some(thinking.as_str()));
}

#[tokio::test]
async fn a_provider_the_executor_cannot_run_fails_fast_without_a_webview() {
    let _guard = TEST_GUARD.lock().await;
    let provider = mock_provider(vec![text_reply("unused")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s", "u1", "user", "hello", 1_000);

    let error = dispatch_completion(
        app(),
        &request("s", "resp-1", "mistral", vec![user]),
        deliver_to(&sink),
    )
    .await
    .expect_err("nothing can answer a Mistral request without a WebView");

    assert!(error.display_message.contains("mistral"));
    assert!(provider.requests.lock().unwrap().is_empty());
    assert!(sink.responses.lock().unwrap().is_empty());
}

fn compact_request(session_id: &str) -> CompactRequest {
    CompactRequest {
        session_id: session_id.to_string(),
        session_name: "test session".to_string(),
        messages: vec![
            message(session_id, "u1", "user", "hello", 1_000),
            message(session_id, "a1", "assistant", "hi", 2_000),
        ],
        to_id: "compact-1".to_string(),
        compacted_delta_count: 2,
        parent_request: Some(CompactionParentRequest {
            model: "mock-model".to_string(),
            provider: PROVIDER.to_string(),
            system_prompt: Some("rust executor test".to_string()),
            session_context: None,
            available_tools: None,
        }),
        resume_completion_after_compact: false,
    }
}

/// A compaction round trip delivers the summary with its usage, and the
/// request the provider sees carries no tools (the summary must be plain text).
#[tokio::test]
async fn a_compaction_round_trip_delivers_the_summary_and_its_usage() {
    let _guard = TEST_GUARD.lock().await;
    let session_id = "rust-executor-compaction";
    let provider = mock_provider(vec![text_reply("A tidy summary.")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));

    assert!(
        dispatch_compaction(app(), &compact_request(session_id), deliver_to(&sink))
            .await
            .unwrap()
    );
    wait_for("the compaction summary", || {
        !sink.compactions.lock().unwrap().is_empty()
    })
    .await;

    let (summary, meta) = sink.compactions.lock().unwrap()[0].clone();
    assert_eq!(summary, "A tidy summary.");
    assert_eq!(meta.finish_reason.as_deref(), Some("stop"));
    assert_eq!(meta.prompt_tokens, Some(12));
    assert_eq!(meta.completion_tokens, Some(4));
    assert_eq!(meta.total_tokens, Some(16));

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].get("tools").is_none());
    // The parent request's system prompt is carried over as one system
    // message ahead of the two messages being compacted.
    let messages = requests[0]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["role"].as_str(), Some("system"));
    assert_eq!(messages[0]["content"].as_str(), Some("rust executor test"));
    assert_eq!(messages[1]["role"].as_str(), Some("user"));
    assert_eq!(messages[2]["role"].as_str(), Some("assistant"));
}

/// An HTTP failure from the provider reaches `compact_error`, not a summary.
#[tokio::test]
async fn a_failing_compaction_call_is_reported_through_compact_error() {
    let _guard = TEST_GUARD.lock().await;
    let session_id = "rust-executor-compaction-error";
    let (endpoint, requests) = failing_provider(502).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(endpoint));

    assert!(
        dispatch_compaction(app(), &compact_request(session_id), deliver_to(&sink))
            .await
            .unwrap()
    );
    wait_for("the compaction error", || {
        !sink.compact_errors.lock().unwrap().is_empty()
    })
    .await;

    assert!(sink.compact_errors.lock().unwrap()[0].contains("HTTP 502"));
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert!(sink.compactions.lock().unwrap().is_empty());
}

/// The executor took the request but the app has no session manager: the
/// request fails with a named error instead of falling through to a WebView
/// listener that may not exist. The lookup is passed in because a mock handle
/// cannot reach app state (see the module docs).
fn error_code(error: &AgentRuntimeError) -> Option<&str> {
    error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref())
}

#[tokio::test]
async fn a_completion_without_a_session_manager_fails_with_a_named_error() {
    let _guard = TEST_GUARD.lock().await;
    let provider = mock_provider(vec![text_reply("unused")]).await;
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s", "u1", "user", "hello", 1_000);

    let error = dispatch_completion(app(), &request("s", "resp-1", PROVIDER, vec![user]), || {
        None
    })
    .await
    .expect_err("no session manager means nothing can receive the result");

    assert_eq!(error_code(&error), Some("RUST_EXECUTOR_NO_SESSION_MANAGER"));
    assert!(
        provider.requests.lock().unwrap().is_empty(),
        "no provider call may be made for a result nobody can receive"
    );
}

#[tokio::test]
async fn a_compaction_without_a_session_manager_fails_with_a_named_error() {
    let _guard = TEST_GUARD.lock().await;
    let provider = mock_provider(vec![text_reply("unused")]).await;
    let _config = configure(config(provider.endpoint.clone()));

    let error = dispatch_compaction(app(), &compact_request("s"), || None)
        .await
        .expect_err("no session manager means nothing can receive the summary");

    assert_eq!(error_code(&error), Some("RUST_EXECUTOR_NO_SESSION_MANAGER"));
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn empty_completion_is_retried_once_then_delivers_text() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_provider(vec![empty_reply(), text_reply("recovered")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-empty-ok", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-empty-ok", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the recovered answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "recovered");
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn empty_completion_twice_fails_with_empty_response_code() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_provider(vec![empty_reply(), empty_reply()]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-empty-err", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-empty-err", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the empty-response error", || {
        !sink.llm_errors.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert!(sink.responses.lock().unwrap().is_empty());
    assert_eq!(
        error_code(&sink.llm_errors.lock().unwrap()[0]),
        Some("RUST_EXECUTOR_EMPTY_RESPONSE")
    );
}

#[tokio::test]
async fn fallback_model_answers_when_primary_fails() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        (
            "advancedSettings",
            json!({ "maxRetries": 0, "retryDelay": 1 }),
        ),
        (
            "fallbackModel",
            json!({ "provider": "openai", "model": "fallback-model" }),
        ),
    ])
    .await;
    // Primary attempt fails once; fallback (RetryPolicy::NONE) then succeeds.
    let provider = fail_then_sse(1, 500, text_reply("from fallback")).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-fb-ok", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-ok", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the fallback answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "from fallback");
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

/// #2961 G1.1: a rejected primary request (HTTP 400) reaches the chat as
/// exactly one notice — before the fallback answer — naming the status and
/// the fallback model, with the raw server error.
#[tokio::test]
async fn a_rejected_primary_emits_one_fallback_notice_before_the_fallback_answer() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        (
            "advancedSettings",
            json!({ "maxRetries": 0, "retryDelay": 1 }),
        ),
        (
            "fallbackModel",
            json!({ "provider": "openai", "model": "fallback-model" }),
        ),
    ])
    .await;
    let provider = fail_then_sse(1, 400, text_reply("from fallback")).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-fb-notice", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-notice", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the fallback answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    let notices = sink.fallback_notices.lock().unwrap();
    assert_eq!(notices.len(), 1, "one fallback must be one notice");
    match &notices[0] {
        AgentEvent::LlmFallbackNotice {
            session_id,
            status_code,
            fallback_provider,
            fallback_model,
            error_message,
        } => {
            assert_eq!(session_id, "s-fb-notice");
            assert_eq!(*status_code, Some(400));
            assert_eq!(fallback_provider, "openai");
            assert_eq!(fallback_model, "fallback-model");
            assert!(
                error_message.contains("HTTP 400"),
                "the raw server error names the status: {error_message}"
            );
        }
        other => panic!("not a fallback notice: {other:?}"),
    }
    // The notice arrives before the answer settles the session.
    let events = sink.events.lock().unwrap();
    assert_eq!(
        events.first().map(String::as_str),
        Some("llm_fallback_notice")
    );
    assert_eq!(events.last().map(String::as_str), Some("llm_response"));
}

/// #2961 acceptance: when no fallback runs, no notice appears.
#[tokio::test]
async fn a_succeeded_primary_emits_no_fallback_notice() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        (
            "advancedSettings",
            json!({ "maxRetries": 0, "retryDelay": 1 }),
        ),
        (
            "fallbackModel",
            json!({ "provider": "openai", "model": "fallback-model" }),
        ),
    ])
    .await;
    let provider = mock_provider(vec![text_reply("from primary")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-fb-none", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-none", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the primary answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "from primary");
    assert!(sink.fallback_notices.lock().unwrap().is_empty());
}

/// #2961: a primary failure with no fallback configured errors out with no notice.
#[tokio::test]
async fn no_fallback_configured_emits_no_fallback_notice() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[(
        "advancedSettings",
        json!({ "maxRetries": 0, "retryDelay": 1 }),
    )])
    .await;
    let provider = fail_then_sse(1, 400, text_reply("unused")).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-fb-nocfg", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-nocfg", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the primary failure", || {
        !sink.llm_errors.lock().unwrap().is_empty()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(sink.responses.lock().unwrap().is_empty());
    assert!(sink.fallback_notices.lock().unwrap().is_empty());
}

#[tokio::test]
async fn fallback_skipped_when_same_provider_and_model() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        (
            "advancedSettings",
            json!({ "maxRetries": 0, "retryDelay": 1 }),
        ),
        (
            "fallbackModel",
            json!({ "provider": PROVIDER, "model": "mock-model" }),
        ),
    ])
    .await;
    let provider = fail_then_sse(1, 500, text_reply("unused")).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-fb-same", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-same", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the primary failure", || {
        provider.requests.lock().unwrap().len() >= 1
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert!(sink.responses.lock().unwrap().is_empty());
    assert!(sink.fallback_notices.lock().unwrap().is_empty());
}

#[tokio::test]
async fn fallback_skipped_when_unsupported() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        (
            "advancedSettings",
            json!({ "maxRetries": 0, "retryDelay": 1 }),
        ),
        (
            "fallbackModel",
            json!({ "provider": "mistral", "model": "mistral-large" }),
        ),
    ])
    .await;
    let provider = fail_then_sse(1, 500, text_reply("unused")).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-fb-unsup", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-unsup", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the primary failure", || {
        provider.requests.lock().unwrap().len() >= 1
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert!(sink.responses.lock().unwrap().is_empty());
    assert!(sink.fallback_notices.lock().unwrap().is_empty());
}

#[tokio::test]
async fn configured_anthropic_base_url_routes_to_rust() {
    let _guard = TEST_GUARD.lock().await;
    let (endpoint, requests) = mock_anthropic_messages().await;
    let _settings = install_settings(&[(
        "serviceConfigs",
        json!({ "anthropic": { "apiKey": "sk-ant", "baseUrl": endpoint.base_url.clone() } }),
    )])
    .await;
    // No endpoint_override: the Rust path must reach the Settings Base URL.
    let _config = configure(ExecutorConfig {
        enabled: true,
        endpoint_override: None,
        assume_no_webview: true,
    });
    let sink = Arc::new(RecordingSink::default());
    let user = message("s-abase", "u1", "user", "hi", 1_000);
    let _ = dispatch_completion(
        app(),
        &request("s-abase", "resp-1", "anthropic", vec![user]),
        deliver_to(&sink),
    )
    .await;
    // Before #2797 an Anthropic session with a custom Base URL stayed on the
    // WebView (the URL would be ignored) and failed without one. Now the Rust
    // executor honors the configured URL, so the Messages request must reach
    // it — the recorded body is the proof of the route.
    wait_for(
        "the Rust path to call the configured Anthropic Base URL",
        || !requests.lock().unwrap().is_empty(),
    )
    .await;
    let body = requests.lock().unwrap()[0].clone();
    assert_eq!(body["model"], "mock-model");
}

#[tokio::test]
async fn fallback_returns_primary_error_when_fallback_also_fails() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        (
            "advancedSettings",
            json!({ "maxRetries": 0, "retryDelay": 1 }),
        ),
        (
            "fallbackModel",
            json!({ "provider": "openai", "model": "fallback-model" }),
        ),
    ])
    .await;
    let (endpoint, requests) = failing_provider(500).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(endpoint));
    let user = message("s-fb-both", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-fb-both", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("primary and fallback attempts", || {
        requests.lock().unwrap().len() >= 2
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(requests.lock().unwrap().len(), 2);
    assert!(sink.responses.lock().unwrap().is_empty());
}

#[tokio::test]
async fn default_max_output_tokens_is_sent_when_request_omits_max_tokens() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[(
        "advancedSettings",
        json!({ "defaultMaxOutputTokens": 2048, "maxRetries": 0 }),
    )])
    .await;
    let provider = mock_provider(vec![text_reply("ok")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-max", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request("s-max", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the answer", || !sink.responses.lock().unwrap().is_empty()).await;

    let body = &provider.requests.lock().unwrap()[0];
    assert_eq!(body["max_completion_tokens"], 2048);
}

#[tokio::test]
async fn compaction_request_carries_no_thinking_parameter() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[(
        "advancedSettings",
        json!({ "thinkingEffort": "high", "defaultMaxOutputTokens": 1024 }),
    )])
    .await;
    let provider = mock_provider(vec![text_reply("summary")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));

    assert!(dispatch_compaction(
        app(),
        &compact_request("s-compact-think"),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the compaction summary", || {
        !sink.compactions.lock().unwrap().is_empty()
    })
    .await;

    let body = &provider.requests.lock().unwrap()[0];
    assert!(body.get("reasoning_effort").is_none());
    assert!(body.get("thinking").is_none());
    assert_eq!(body["max_completion_tokens"], 1024);
}

#[tokio::test]
async fn streamed_completion_records_usage_timing_details_on_saved_message() {
    let _guard = TEST_GUARD.lock().await;
    let session_id = "rust-executor-usage-timing";
    let provider = mock_provider(vec![text_reply("Hello with timing.")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message(session_id, "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request(session_id, "resp-timing-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the timed answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    let turn = sink.responses.lock().unwrap()[0].clone();
    let usage = turn.usage.expect("saved assistant message carries usage");
    let details = usage
        .get("details")
        .expect("saved usage carries timing details");
    let eval_duration = details
        .get("evalDuration")
        .and_then(Value::as_f64)
        .expect("evalDuration is a number");
    assert!(
        eval_duration > 0.0,
        "expected evalDuration > 0, got {eval_duration}"
    );
    assert!(
        details
            .get("promptEvalDuration")
            .and_then(Value::as_f64)
            .is_some_and(|ms| ms >= 0.0),
        "expected non-negative promptEvalDuration, got {details}"
    );
    assert!(
        details
            .get("timeToFirstToken")
            .and_then(Value::as_f64)
            .is_some_and(|ms| ms >= 0.0),
        "expected non-negative timeToFirstToken, got {details}"
    );
}

/// A reply whose tool-call arguments are cut off mid-JSON — the shape the
/// invalid-args policy discards and resamples (#2967).
fn truncated_tool_call_reply(name: &str) -> Vec<Value> {
    vec![
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_trunc","function":{"name":name,"arguments":"{\"command\": \"pnpm rust:test --lib"}}]},"finish_reason":"tool_calls"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":9,"total_tokens":19}}),
    ]
}

/// The #2967 flow: the handler discards a reply with invalid tool-call
/// arguments and requests a replacement completion from inside the delivery
/// of the running call. This sink mirrors the two steps the session-manager
/// sink performs in production at that moment: re-dispatch through the real
/// executor entry, then finish the delivery through an await point (the
/// production resample paths pass await points after dispatching the
/// replacement — the tool-loop resample records its attempt after it).
#[derive(Default)]
struct ResamplingSink {
    /// Filled by `new`; lets the sink re-dispatch through its own Arc.
    weak: std::sync::Weak<ResamplingSink>,
    resampled_text: Mutex<Option<String>>,
    delivery_done: AtomicBool,
    resample_requested: AtomicBool,
}

impl ResamplingSink {
    fn new(weak: std::sync::Weak<ResamplingSink>) -> Self {
        Self {
            weak,
            resampled_text: Mutex::new(None),
            delivery_done: AtomicBool::new(false),
            resample_requested: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl ResponseSink for ResamplingSink {
    async fn llm_response(&self, session_id: String, reply: Message) -> Result<(), String> {
        let invalid = reply.tool_calls.as_ref().is_some_and(|calls| {
            calls.iter().any(|call| {
                !serde_json::from_str::<Value>(&call.function.arguments)
                    .is_ok_and(|value| value.is_object())
            })
        });
        if !invalid {
            *self.resampled_text.lock().unwrap() = Some(text_of(&reply));
            return Ok(());
        }
        // Discard the invalid reply and request the replacement from inside
        // this delivery, exactly where production does it.
        let Some(this) = self.weak.upgrade() else {
            return Err("sink dropped mid-delivery".to_string());
        };
        let sink: Arc<dyn ResponseSink> = this;
        let user = message(session_id.as_str(), "u1", "user", "run it", 1_000);
        let dispatched = dispatch_completion(
            app(),
            &request(&session_id, "resp-2", PROVIDER, vec![user]),
            move || Some(sink),
        )
        .await
        .map_err(|error| error.display_message.clone())?;
        if !dispatched {
            return Err("the mock provider must answer in Rust".to_string());
        }
        self.resample_requested.store(true, Ordering::SeqCst);
        // The call that requested the replacement must survive this await and
        // finish its own delivery; aborting it here is what left #2967
        // unsettled — its reply processing died with the call.
        tokio::task::yield_now().await;
        self.delivery_done.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn llm_error(&self, _session_id: String, error: AgentRuntimeError) -> Result<(), String> {
        *self.resampled_text.lock().unwrap() = Some(format!("ERROR: {}", error.display_message));
        Ok(())
    }
    async fn compact_response(
        &self,
        _session_id: &str,
        _request: &CompactRequest,
        _summary: String,
        _meta: CompactStreamMeta,
    ) -> Result<(), String> {
        Ok(())
    }
    async fn compact_error(
        &self,
        _session_id: String,
        _error: AgentRuntimeError,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// #2967: a resample requested from inside the running call must not abort
/// that call. The turn settles: the running delivery completes through its
/// await point after the replacement dispatch, and the resampled answer
/// arrives — both inside the issue's 10 s settle window.
#[tokio::test]
async fn a_resample_requested_from_inside_a_running_call_settles_the_turn() {
    let _guard = TEST_GUARD.lock().await;
    let session_id = "rust-executor-resample-selfreplace";
    let provider = mock_provider(vec![
        truncated_tool_call_reply("workspace__runShell"),
        text_reply("Resampled answer."),
    ])
    .await;
    let sink = Arc::new_cyclic(|weak| ResamplingSink::new(weak.clone()));
    let _config = configure(config(provider.endpoint.clone()));
    let user = message(session_id, "u1", "user", "run it", 1_000);

    let sink_for_dispatch: Arc<dyn ResponseSink> = sink.clone();
    assert!(dispatch_completion(
        app(),
        &request(session_id, "resp-1", PROVIDER, vec![user]),
        move || { Some(sink_for_dispatch) }
    )
    .await
    .unwrap());

    // The issue's acceptance window: the turn settles within 10 s.
    let settle = Instant::now();
    wait_for("the resampled answer to arrive", || {
        sink.resampled_text.lock().unwrap().is_some()
    })
    .await;
    assert!(
        settle.elapsed() < Duration::from_secs(10),
        "the turn must settle inside the issue's 10 s window"
    );
    // The call that requested the resample was not aborted by it: its
    // delivery finished through the await point after the replacement
    // dispatch. Before the #2967 fix the abort dropped this delivery.
    assert!(
        sink.delivery_done.load(Ordering::SeqCst),
        "the running call must finish its own delivery after the resample dispatch"
    );
    assert_eq!(
        sink.resampled_text.lock().unwrap().as_deref(),
        Some("Resampled answer.")
    );
    // The mock saw exactly the original call and the resample.
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// #3089: Gemini MALFORMED_FUNCTION_CALL retry without tools
// ---------------------------------------------------------------------------

/// A Gemini mock whose first `fail_count` requests answer with `fail_status`,
/// and whose request at index `hold_from` (if any) answers with an SSE body
/// that never yields data: it can only end when the client drops the
/// connection, which sets `disconnected`. A cancel test waits on
/// `disconnected` — the mock seeing the held call's connection die is the
/// proof the abort stopped the task, and no timer can race it.
struct GeminiMock {
    provider: MockProvider,
    disconnected: Arc<AtomicBool>,
}

/// Held-open SSE body: never yields, so its `Drop` means the client closed
/// the connection while the response was still open.
struct HeldSseBody {
    disconnected: Arc<AtomicBool>,
}

impl Drop for HeldSseBody {
    fn drop(&mut self) {
        self.disconnected.store(true, Ordering::SeqCst);
    }
}

impl futures::stream::Stream for HeldSseBody {
    type Item = Result<Vec<u8>, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Pending
    }
}

/// A loopback server on the Gemini `:streamGenerateContent` path that answers
/// from a script and keeps every request body. Unlike the OpenAI mock, the
/// stream has no `[DONE]` sentinel: a Gemini stream simply ends.
async fn mock_gemini(replies: Vec<Vec<Value>>) -> MockProvider {
    mock_gemini_with(replies, 0, 500, None).await.provider
}

async fn mock_gemini_with(
    replies: Vec<Vec<Value>>,
    fail_count: usize,
    fail_status: u16,
    hold_from: Option<usize>,
) -> GeminiMock {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let counter = Arc::new(AtomicUsize::new(0));
    let disconnected = Arc::new(AtomicBool::new(false));
    let disconnected_in_route = disconnected.clone();
    let route = warp::post()
        .and(warp::path::tail())
        .and(warp::body::json())
        .and_then(move |_: warp::path::Tail, body: Value| {
            seen.lock().unwrap().push(body);
            let index = counter.fetch_add(1, Ordering::SeqCst);
            // Failed attempts do not consume a scripted reply.
            let reply_index = index.saturating_sub(fail_count);
            let body = gemini_sse(&replies[reply_index.min(replies.len() - 1)]);
            let disconnected = disconnected_in_route.clone();
            async move {
                if index < fail_count {
                    return Ok(warp::http::Response::builder()
                        .status(fail_status)
                        .header("content-type", "text/plain")
                        .body(warp::hyper::Body::from("provider exploded".to_string()))
                        .unwrap());
                }
                if hold_from == Some(index) {
                    return Ok(warp::http::Response::builder()
                        .header("content-type", "text/event-stream")
                        .body(warp::hyper::Body::wrap_stream(HeldSseBody { disconnected }))
                        .unwrap());
                }
                Ok::<_, warp::Rejection>(body.map(warp::hyper::Body::from))
            }
        });
    let (addr, server): (SocketAddr, _) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);
    GeminiMock {
        provider: MockProvider {
            endpoint: Endpoint {
                base_url: format!("http://{addr}/v1beta"),
                api_key: Some("gemini-test-key".to_string()),
            },
            requests,
        },
        disconnected,
    }
}

/// The SSE body of one Gemini stream: one `data:` line per chunk, no sentinel.
fn gemini_sse(chunks: &[Value]) -> warp::http::Response<String> {
    let body: String = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect();
    warp::http::Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap()
}

/// A Gemini stream that ends in `MALFORMED_FUNCTION_CALL` with `text` as the
/// only content — the shape the retry must throw away.
fn gemini_malformed_reply(text: &str) -> Vec<Value> {
    vec![
        json!({"candidates":[{"content":{"parts":[{"text":text}]},"finishReason":"MALFORMED_FUNCTION_CALL"}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":2}}),
    ]
}

/// A normal Gemini text stream.
fn gemini_text_reply(text: &str) -> Vec<Value> {
    vec![
        json!({"candidates":[{"content":{"parts":[{"text":text}]}}]}),
        json!({"candidates":[{"content":{"parts":[]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":3}}),
    ]
}

/// One declared tool, so the executor has something to drop for the retry.
fn one_tool() -> MCPTool {
    MCPTool {
        name: "mock__tool".to_string(),
        title: None,
        description: "A mock tool.".to_string(),
        input_schema: JSONSchema {
            schema_type: JSONSchemaType::Object {
                properties: Some(IndexMap::new()),
                required: None,
                additional_properties: None,
                property_names: None,
                min_properties: None,
                max_properties: None,
            },
            title: None,
            description: None,
            default: None,
            examples: None,
            enum_values: None,
            const_value: None,
            one_of: None,
        },
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

/// A completion request that offers the model a tool.
fn request_with_tools(
    session_id: &str,
    response_id: &str,
    provider: &str,
    messages: Vec<Message>,
) -> CompletionRequest {
    CompletionRequest {
        available_tools: Some(vec![one_tool()]),
        ..request(session_id, response_id, provider, messages)
    }
}

/// #3089: a Gemini turn ending in `MALFORMED_FUNCTION_CALL` is retried one
/// time with no tools, and the retry's answer is what reaches the chat.
/// The attempt streams live like any other turn; an `LlmStreamReset`
/// withdraws its text before the retry's answer arrives.
#[tokio::test]
async fn gemini_malformed_function_call_retries_once_without_tools() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_gemini(vec![
        gemini_malformed_reply("First-attempt text."),
        gemini_text_reply("Recovered."),
    ])
    .await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-gm-retry", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-gm-retry", "resp-1", "gemini", vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the retried answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    // The first try carried the tool; the retry carried none.
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(1));
    assert!(
        requests[1].get("tools").is_none(),
        "the retry must send no tools: {:#}",
        requests[1]
    );
    // Only the retry's answer reaches the chat; the malformed text does not.
    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "Recovered.");
    assert!(sink.llm_errors.lock().unwrap().is_empty());
    // The attempt streamed live (turns stream even when a retry is possible),
    // the reset withdrew it, and only then did the retry's answer stream.
    let events = sink.stream_events.lock().unwrap();
    let content_position = |needle: &str| {
        events.iter().position(|event| {
            matches!(event, AgentEvent::LlmStreamDelta { delta, .. }
                if delta
                    .content
                    .as_deref()
                    .is_some_and(|content| content.contains(needle)))
        })
    };
    let discarded = content_position("First-attempt text.")
        .expect("the attempt streams live, like any other turn");
    let reset = events
        .iter()
        .position(|event| matches!(event, AgentEvent::LlmStreamReset { .. }))
        .expect("the discarded attempt is withdrawn before the retry");
    let recovered = content_position("Recovered.").expect("the retry streams live");
    assert!(
        discarded < reset && reset < recovered,
        "the reset must fall between the discarded text and the retry: {events:?}"
    );
}

/// #3089: a successful Gemini turn that offers tools streams live deltas —
/// the malformed-retry path must not hold deltas back for normal turns.
#[tokio::test]
async fn gemini_turn_with_tools_streams_live_deltas() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_gemini(vec![gemini_text_reply("Answered.")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-gm-stream", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-gm-stream", "resp-1", "gemini", vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the delivered answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "Answered.");
    let events = sink.stream_events.lock().unwrap();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AgentEvent::LlmStreamDelta { delta, .. }
                if delta
                    .content
                    .as_deref()
                    .is_some_and(|content| content.contains("Answered."))
        )),
        "a successful Gemini turn must stream its answer live: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::LlmStreamReset { .. })),
        "a successful turn has nothing to withdraw: {events:?}"
    );
}

/// A Gemini stream that ends in `MALFORMED_FUNCTION_CALL` with no content at
/// all — the shape the empty-response check used to swallow before the
/// finish reason could trigger the retry (#3089).
fn gemini_malformed_reply_without_content() -> Vec<Value> {
    vec![
        json!({"candidates":[{"content":{"parts":[]},"finishReason":"MALFORMED_FUNCTION_CALL"}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":0}}),
    ]
}

/// #3089: a content-free `MALFORMED_FUNCTION_CALL` is retried without tools
/// too — the finish reason decides the retry before the empty-response check.
#[tokio::test]
async fn gemini_malformed_function_call_without_content_retries_once() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_gemini(vec![
        gemini_malformed_reply_without_content(),
        gemini_text_reply("Recovered."),
    ])
    .await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-gm-empty", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-gm-empty", "resp-1", "gemini", vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the retried answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    let requests = provider.requests.lock().unwrap();
    assert!(
        requests[1].get("tools").is_none(),
        "the retry must send no tools: {:#}",
        requests[1]
    );
    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "Recovered.");
    assert!(sink.llm_errors.lock().unwrap().is_empty());
}

/// #3089: the retry belongs to the individual provider call — a Gemini
/// fallback model behind another provider gets its own tool-less retry, and
/// the fallback notice is emitted exactly once.
#[tokio::test]
async fn gemini_fallback_model_behind_another_provider_retries_its_malformed_call() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[
        ("advancedSettings", json!({ "maxRetries": 0 })),
        (
            "fallbackModel",
            json!({ "provider": "gemini", "model": "gemini-2.5-flash" }),
        ),
    ])
    .await;
    // 1: the primary (non-Gemini) call fails; 2: the Gemini fallback answers
    // malformed; 3: the fallback's own tool-less retry succeeds.
    let provider = mock_gemini_with(
        vec![
            gemini_malformed_reply("Bad tool."),
            gemini_text_reply("Recovered."),
        ],
        1,
        500,
        None,
    )
    .await
    .provider;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-gm-fb", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-gm-fb", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the retried fallback answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(provider.requests.lock().unwrap().len(), 3);
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests[1]["tools"].as_array().map(Vec::len), Some(1));
    assert!(
        requests[2].get("tools").is_none(),
        "the Gemini fallback's retry must send no tools: {:#}",
        requests[2]
    );
    assert_eq!(text_of(&sink.responses.lock().unwrap()[0]), "Recovered.");
    assert_eq!(
        sink.fallback_notices.lock().unwrap().len(),
        1,
        "the retry must not repeat the fallback notice"
    );
    assert!(sink.llm_errors.lock().unwrap().is_empty());
}

/// #3089: a second malformed result is not retried again — the session
/// settles with a `MALFORMED_FUNCTION_CALL` error.
#[tokio::test]
async fn gemini_malformed_function_call_is_not_retried_twice() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_gemini(vec![gemini_malformed_reply("Still broken.")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-gm-twice", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-gm-twice", "resp-1", "gemini", vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the settled error", || {
        !sink.llm_errors.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(
        provider.requests.lock().unwrap().len(),
        2,
        "exactly one retry, never a second"
    );
    assert!(sink.responses.lock().unwrap().is_empty());
    let errors = sink.llm_errors.lock().unwrap();
    assert_eq!(
        serde_json::to_value(&errors[0].error_type).expect("error type is serializable"),
        "MALFORMED_FUNCTION_CALL"
    );
}

/// #3089: the retry is Gemini-only — another provider ending with the same
/// finish reason is delivered as-is, without a second call.
#[tokio::test]
async fn malformed_function_call_retry_is_gemini_only() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let provider = mock_provider(vec![vec![
        json!({"choices":[{"delta":{"content":"OpenAI-shaped."},"finish_reason":"malformed_function_call"}]}),
    ]])
    .await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-non-gemini", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-non-gemini", "resp-1", PROVIDER, vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    wait_for("the delivered answer", || {
        !sink.responses.lock().unwrap().is_empty()
    })
    .await;

    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(
        text_of(&sink.responses.lock().unwrap()[0]),
        "OpenAI-shaped."
    );
    assert!(sink.llm_errors.lock().unwrap().is_empty());
}

/// #3089: a cancel during the tool-less retry stops the call — nothing is
/// delivered after the abort. The retry's response is held open forever, so
/// the mock seeing its connection drop is proof the call task itself died:
/// a `cancel` that failed to abort the task leaves the body (and any chance
/// of delivery) alive until this wait times out — the test cannot pass
/// vacuously.
#[tokio::test]
async fn cancel_during_the_malformed_function_call_retry_delivers_nothing() {
    let _guard = TEST_GUARD.lock().await;
    let _settings = install_settings(&[("advancedSettings", json!({ "maxRetries": 0 }))]).await;
    let mock = mock_gemini_with(
        vec![
            gemini_malformed_reply("First-attempt text."),
            gemini_text_reply("Never delivered."),
        ],
        0,
        500,
        Some(1),
    )
    .await;
    let provider = &mock.provider;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone()));
    let user = message("s-gm-cancel", "u1", "user", "hi", 1_000);

    assert!(dispatch_completion(
        app(),
        &request_with_tools("s-gm-cancel", "resp-1", "gemini", vec![user]),
        deliver_to(&sink),
    )
    .await
    .unwrap());
    // The tool-less retry is in flight, its response held open by the mock.
    wait_for("the tool-less retry to start", || {
        provider.requests.lock().unwrap().len() >= 2
    })
    .await;
    cancel("s-gm-cancel");
    // The abort's observable proof: the mock's held body drops only when the
    // client's connection closes, which means the call task died with it.
    wait_for("the cancelled call's connection to drop", || {
        mock.disconnected.load(Ordering::SeqCst)
    })
    .await;

    assert!(sink.responses.lock().unwrap().is_empty());
    assert!(sink.llm_errors.lock().unwrap().is_empty());
}
