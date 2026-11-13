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

use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use warp::Filter;

use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::llm::rust_executor::{
    configure, try_dispatch_completion, Endpoint, ExecutorConfig, ResponseSink,
};
use tauri_mcp_agent_lib::agent::llm::types::{
    AgentRuntimeError, CompactRequest, CompletionRequest,
};
use tauri_mcp_agent_lib::agent::session_manager::CompactStreamMeta;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
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
}

#[async_trait::async_trait]
impl ResponseSink for RecordingSink {
    async fn llm_response(&self, _session_id: String, message: Message) -> Result<(), String> {
        self.responses.lock().unwrap().push(message);
        Ok(())
    }
    async fn llm_error(&self, _session_id: String, error: AgentRuntimeError) -> Result<(), String> {
        Err(error.display_message)
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
        error: AgentRuntimeError,
    ) -> Result<(), String> {
        Err(error.display_message)
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

fn config(endpoint: Endpoint, sink: Arc<RecordingSink>) -> ExecutorConfig {
    ExecutorConfig {
        enabled: true,
        endpoint_override: Some(endpoint),
        assume_no_webview: true,
        sink_override: Some(sink),
    }
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
    let _config = configure(config(provider.endpoint.clone(), sink.clone()));
    let handle = app();
    let user = message(session_id, "u1", "user", "list the playbooks", 1_000);

    assert!(try_dispatch_completion(
        handle,
        &request(session_id, "resp-1", PROVIDER, vec![user.clone()])
    )
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
    assert!(try_dispatch_completion(handle, &follow_up).unwrap());
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

#[tokio::test]
async fn a_provider_the_executor_cannot_run_fails_fast_without_a_webview() {
    let _guard = TEST_GUARD.lock().await;
    let provider = mock_provider(vec![text_reply("unused")]).await;
    let sink = Arc::new(RecordingSink::default());
    let _config = configure(config(provider.endpoint.clone(), sink.clone()));
    let user = message("s", "u1", "user", "hello", 1_000);

    let error = try_dispatch_completion(app(), &request("s", "resp-1", "anthropic", vec![user]))
        .expect_err("nothing can answer an Anthropic request without a WebView");

    assert!(error.display_message.contains("anthropic"));
    assert!(provider.requests.lock().unwrap().is_empty());
    assert!(sink.responses.lock().unwrap().is_empty());
}
