//! One streaming chat-completions call against an OpenAI-compatible endpoint.

use super::stream::{ChatStreamAccumulator, CompletedChat, LineBuffer};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use futures::StreamExt;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

/// Where and how to reach the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Base URL up to and including the API version, for example
    /// `https://api.openai.com/v1` or `http://127.0.0.1:8000/v1`.
    pub base_url: String,
    /// Sent as a bearer token when present. Local servers often need none.
    pub api_key: Option<String>,
}

impl Endpoint {
    fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

/// Long generations are normal; this only bounds a stream that went silent.
const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How much of an error body is kept for the user-facing message.
const ERROR_BODY_CHARS: usize = 600;

fn error(kind: AgentRuntimeErrorType, code: &str, message: String) -> Box<AgentRuntimeError> {
    Box::new(AgentRuntimeError::new(kind, message).with_code(code))
}

/// One client for every call, so connections to a provider are reused.
static HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_IDLE_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())
});

fn http_status_error(status: reqwest::StatusCode, body: &str) -> Box<AgentRuntimeError> {
    let snippet: String = body.chars().take(ERROR_BODY_CHARS).collect();
    let message = format!(
        "LLM provider returned HTTP {}: {}",
        status.as_u16(),
        snippet
    );
    match status.as_u16() {
        401 | 403 => error(
            AgentRuntimeErrorType::AuthenticationError,
            "RUST_EXECUTOR_AUTH",
            message,
        ),
        429 => error(
            AgentRuntimeErrorType::RateLimitError,
            "RUST_EXECUTOR_RATE_LIMIT",
            message,
        ),
        _ => error(
            AgentRuntimeErrorType::AiServiceError,
            "RUST_EXECUTOR_HTTP",
            message,
        ),
    }
}

/// POST `body` and read the stream to its end.
pub(super) async fn complete(
    endpoint: &Endpoint,
    body: &Value,
) -> Result<CompletedChat, Box<AgentRuntimeError>> {
    let client = HTTP_CLIENT.as_ref().map_err(|e| {
        error(
            AgentRuntimeErrorType::NetworkError,
            "RUST_EXECUTOR_CLIENT",
            format!("Failed to build the HTTP client: {e}"),
        )
    })?;

    let mut request = client
        .post(endpoint.chat_completions_url())
        .header("accept", "text/event-stream")
        .json(body);
    if let Some(api_key) = endpoint.api_key.as_deref().filter(|key| !key.is_empty()) {
        request = request.bearer_auth(api_key);
    }

    let response = request.send().await.map_err(|e| {
        error(
            AgentRuntimeErrorType::NetworkError,
            "RUST_EXECUTOR_NETWORK",
            // `without_url`: a base URL can carry credentials in its query.
            format!("Could not reach the LLM provider: {}", e.without_url()),
        )
    })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_status_error(status, &body));
    }

    let mut lines = LineBuffer::default();
    let mut accumulator = ChatStreamAccumulator::default();
    let mut bytes = response.bytes_stream();
    let stream_error = |message: String| {
        error(
            AgentRuntimeErrorType::AiServiceError,
            "RUST_EXECUTOR_STREAM",
            message,
        )
    };

    'read: while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(|e| {
            error(
                AgentRuntimeErrorType::NetworkError,
                "RUST_EXECUTOR_NETWORK",
                format!("The LLM stream was interrupted: {}", e.without_url()),
            )
        })?;
        for line in lines.push(&chunk) {
            if accumulator.push_line(&line).map_err(stream_error)? {
                break 'read;
            }
        }
    }
    if let Some(rest) = lines.finish() {
        accumulator.push_line(&rest).map_err(stream_error)?;
    }

    Ok(accumulator.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use warp::Filter;

    /// Serves `reply` for POST /v1/chat/completions on a free loopback port.
    async fn serve(
        reply: impl Fn(Value, Option<String>) -> warp::http::Response<String>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Endpoint {
        let route = warp::post()
            .and(warp::path!("v1" / "chat" / "completions"))
            .and(warp::body::json())
            .and(warp::header::optional::<String>("authorization"))
            .map(move |body: Value, auth: Option<String>| reply(body, auth));
        let (addr, server) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::spawn(server);
        Endpoint {
            base_url: format!("http://{addr}/v1/"),
            api_key: Some("dummy-token-abcd".to_string()),
        }
    }

    fn sse(lines: &[&str]) -> warp::http::Response<String> {
        let body: String = lines
            .iter()
            .map(|line| format!("data: {line}\n\n"))
            .collect();
        warp::http::Response::builder()
            .header("content-type", "text/event-stream")
            .body(body)
            .unwrap()
    }

    #[tokio::test]
    async fn a_streamed_answer_with_a_tool_call_is_read_to_the_end() {
        let endpoint = serve(|body, auth| {
            // The request must be a streaming one, with the bearer token.
            assert_eq!(body["stream"], true);
            assert_eq!(auth.as_deref(), Some("Bearer dummy-token-abcd"));
            sse(&[
                r#"{"choices":[{"delta":{"content":"Reading the file."}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"workspace__readFile","arguments":"{\"path\":\"a.txt\"}"}}]},"finish_reason":"tool_calls"}]}"#,
                r#"{"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":9,"total_tokens":29}}"#,
                "[DONE]",
            ])
        })
        .await;

        let chat = complete(&endpoint, &json!({ "model": "m", "stream": true }))
            .await
            .expect("completion succeeds");

        assert_eq!(chat.content, "Reading the file.");
        assert_eq!(chat.tool_calls.len(), 1);
        assert_eq!(chat.tool_calls[0].function.name, "workspace__readFile");
        assert_eq!(chat.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chat.usage.unwrap()["promptTokens"], 20);
    }

    #[tokio::test]
    async fn http_errors_are_classified_and_keep_the_provider_message() {
        let endpoint = serve(|_, _| {
            warp::http::Response::builder()
                .status(401)
                .body(r#"{"error":{"message":"Incorrect API key provided"}}"#.to_string())
                .unwrap()
        })
        .await;

        let error = complete(&endpoint, &json!({})).await.unwrap_err();

        assert!(matches!(
            error.error_type,
            AgentRuntimeErrorType::AuthenticationError
        ));
        assert!(error.display_message.contains("HTTP 401"));
        assert!(error.display_message.contains("Incorrect API key"));
    }

    #[tokio::test]
    async fn an_unreachable_endpoint_is_a_network_error() {
        // Bind then drop, so the port is closed.
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let endpoint = Endpoint {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            api_key: None,
        };

        let error = complete(&endpoint, &json!({})).await.unwrap_err();

        assert!(matches!(
            error.error_type,
            AgentRuntimeErrorType::NetworkError
        ));
    }
}
