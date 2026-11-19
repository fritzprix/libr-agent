//! One streaming chat call, with the GUI's retry policy.

use crate::error::{ErrorKind, LlmError};
use crate::provider::ChatProvider;
use crate::stream::LineBuffer;
use crate::types::{ChatRequest, CompletedChat, StreamDelta, UsageDetails};
use futures::StreamExt;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// Long generations are normal; this only bounds a stream that went silent.
const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// One client for every call, so connections to a provider are reused.
static HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    let builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_IDLE_TIMEOUT);
    let builder = http_proxy::configure_client_builder(builder).map_err(|e| e.to_string())?;
    builder.build().map_err(|e| e.to_string())
});

/// How often a failed call is tried again before the response starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries after the first attempt.
    pub max_retries: u32,
    /// Wait before the first retry; it doubles for each one after.
    pub base_delay: Duration,
}

impl RetryPolicy {
    pub const NONE: Self = Self {
        max_retries: 0,
        base_delay: Duration::ZERO,
    };

    fn delay_before_retry(&self, retry_index: u32) -> Duration {
        let factor = 1u32.checked_shl(retry_index).unwrap_or(u32::MAX);
        self.base_delay.saturating_mul(factor)
    }
}

/// True when `base_url` is a loopback, private-network, or `.local` host.
///
/// Such servers get no retries: a retry repeats a slow local prefill and
/// competes for the same GPU. Same rule as the GUI (`llm-host-policy.ts`).
fn is_self_hosted(base_url: &str) -> bool {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return false;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let Ok(url) = reqwest::Url::parse(&with_scheme) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_lowercase();
    if matches!(
        host.as_str(),
        "localhost" | "127.0.0.1" | "::1" | "0.0.0.0" | "host.docker.internal"
    ) || host.ends_with(".local")
    {
        return true;
    }
    let Ok(ip) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    ip.is_private()
}

fn network_error(message: String) -> LlmError {
    LlmError::new(ErrorKind::Network, "RUST_EXECUTOR_NETWORK", message)
}

/// Sends `body` to `url` once and returns the response if it is a success.
async fn send(
    client: &reqwest::Client,
    provider: &dyn ChatProvider,
    url: &str,
    body: &Value,
) -> Result<reqwest::Response, LlmError> {
    let mut request = client
        .post(url)
        .header("accept", "text/event-stream")
        .json(body);
    for (name, value) in provider.extra_headers() {
        request = request.header(*name, *value);
    }
    if let Some(api_key) = provider
        .endpoint()
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
    {
        let (name, value) = provider.auth_header(api_key);
        request = request.header(name, value);
    }

    let response = request.send().await.map_err(|e| {
        // `without_url`: a base URL can carry credentials in its query.
        network_error(format!(
            "Could not reach the LLM provider: {}",
            e.without_url()
        ))
    })?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let text = response.text().await.unwrap_or_default();
    Err(provider.http_error(status.as_u16(), &text))
}

/// [`send`], tried again under `retry` while the error is retryable.
async fn send_with_retries(
    client: &reqwest::Client,
    provider: &dyn ChatProvider,
    url: &str,
    body: &Value,
    retry: RetryPolicy,
) -> Result<reqwest::Response, LlmError> {
    let mut retries_done = 0;
    loop {
        match send(client, provider, url, body).await {
            Ok(response) => return Ok(response),
            Err(error) if error.is_retryable() && retries_done < retry.max_retries => {
                let delay = retry.delay_before_retry(retries_done);
                retries_done += 1;
                log::warn!(
                    "LLM call failed ({}); retry {retries_done}/{} after {delay:?}",
                    error.message,
                    retry.max_retries
                );
                tokio::time::sleep(delay).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Run `request` on `provider` and read the streamed answer to its end.
///
/// A call that fails before the response starts is tried again under `retry`
/// when [`LlmError::is_retryable`] says it may help; self-hosted endpoints are
/// never retried. A stream that breaks after it started is not retried.
pub async fn complete(
    provider: &dyn ChatProvider,
    request: &ChatRequest,
    retry: RetryPolicy,
) -> Result<CompletedChat, LlmError> {
    complete_with_deltas(provider, request, retry, |_| {}).await
}

/// [`complete`], calling `on_delta` for each increment as the stream arrives.
///
/// Deltas come in stream order, each as soon as its line is read; the returned
/// [`CompletedChat`] is the same one [`complete`] gives. A delta is only ever
/// sent for a response that started, so a call that is retried reports none
/// of the failed attempts. `on_delta` runs on the reading task and must not
/// block.
pub async fn complete_with_deltas(
    provider: &dyn ChatProvider,
    request: &ChatRequest,
    retry: RetryPolicy,
    mut on_delta: impl FnMut(StreamDelta) + Send,
) -> Result<CompletedChat, LlmError> {
    let client = HTTP_CLIENT.as_ref().map_err(|e| {
        LlmError::new(
            ErrorKind::Network,
            "RUST_EXECUTOR_CLIENT",
            format!("Failed to build the HTTP client: {e}"),
        )
    })?;
    let retry = if is_self_hosted(&provider.endpoint().base_url) {
        RetryPolicy::NONE
    } else {
        retry
    };
    let url = provider.request_url(&provider.model_id(&request.model));
    if let Err(e) = http_proxy::ensure_proxy_resolved(&url).await {
        log::warn!("system proxy pre-resolve failed for LLM URL: {e}");
    }
    let body = provider.build_body(request);
    let mut request_start = Instant::now();
    let response = match send_with_retries(client, provider, &url, &body, retry).await {
        Ok(response) => response,
        // A provider may know a change to the body that fixes the error; it is
        // sent once, whatever `retry` says.
        Err(error) => match provider.body_after_error(&body, &error) {
            Some(fixed) => {
                request_start = Instant::now();
                send_with_retries(client, provider, &url, &fixed, retry).await?
            }
            None => return Err(error),
        },
    };

    let stream_error =
        |message: String| LlmError::new(ErrorKind::Service, "RUST_EXECUTOR_STREAM", message);
    let mut lines = LineBuffer::default();
    let mut parser = provider.stream_parser();
    let mut bytes = response.bytes_stream();
    let mut first_token_at: Option<Instant> = None;
    let mut observe_and_emit = |delta: StreamDelta| {
        if first_token_at.is_none() && has_token_output(&delta) {
            first_token_at = Some(Instant::now());
        }
        on_delta(delta);
    };
    'read: while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(|e| {
            network_error(format!(
                "The LLM stream was interrupted: {}",
                e.without_url()
            ))
        })?;
        for line in lines.push(&chunk) {
            let step = parser.push_line(&line).map_err(stream_error)?;
            step.deltas.into_iter().for_each(&mut observe_and_emit);
            if step.done {
                break 'read;
            }
        }
    }
    if let Some(rest) = lines.finish() {
        let step = parser.push_line(&rest).map_err(stream_error)?;
        step.deltas.into_iter().for_each(&mut observe_and_emit);
    }

    let finished_at = Instant::now();
    let mut chat = parser.finish();
    attach_timing(&mut chat, request_start, first_token_at, finished_at);
    Ok(chat)
}

fn has_token_output(delta: &StreamDelta) -> bool {
    delta.content.is_some() || delta.thinking.is_some() || !delta.tool_calls.is_empty()
}

fn attach_timing(
    chat: &mut CompletedChat,
    request_start: Instant,
    first_token_at: Option<Instant>,
    finished_at: Instant,
) {
    let Some(usage) = chat.usage.as_mut().filter(|u| u.completion_tokens > 0) else {
        return;
    };
    let details = usage.details.get_or_insert_with(UsageDetails::default);
    let total_ms = finished_at
        .saturating_duration_since(request_start)
        .as_secs_f64()
        * 1000.0;
    if let Some(first_token_at) = first_token_at {
        let ttft_ms = first_token_at
            .saturating_duration_since(request_start)
            .as_secs_f64()
            * 1000.0;
        let eval_ms = finished_at
            .saturating_duration_since(first_token_at)
            .as_secs_f64()
            * 1000.0;
        if details.eval_duration.is_none() {
            details.prompt_eval_duration.get_or_insert(ttft_ms);
            details.eval_duration = Some(if eval_ms > 0.0 { eval_ms } else { total_ms });
        }
        details.time_to_first_token.get_or_insert(ttft_ms);
    } else {
        details.eval_duration.get_or_insert(total_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{CustomOpenAi, Endpoint, Ollama, OpenRouter};
    use crate::types::ThinkingEffort;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use warp::Filter;

    /// What the mock server saw: request bodies and selected headers.
    #[derive(Default)]
    struct Seen {
        bodies: Mutex<Vec<Value>>,
        headers: Mutex<Vec<(Option<String>, Option<String>)>>,
        calls: AtomicUsize,
    }

    /// Serves POST /v1/chat/completions on a free loopback port. `reply` gets
    /// the 0-based call number.
    async fn serve(
        reply: impl Fn(usize) -> warp::http::Response<String> + Clone + Send + Sync + 'static,
    ) -> (Endpoint, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        let record = seen.clone();
        let route = warp::post()
            .and(warp::path!("v1" / "chat" / "completions"))
            .and(warp::body::json())
            .and(warp::header::optional::<String>("authorization"))
            .and(warp::header::optional::<String>("x-title"))
            .map(
                move |body: Value, auth: Option<String>, title: Option<String>| {
                    record.bodies.lock().unwrap().push(body);
                    record.headers.lock().unwrap().push((auth, title));
                    reply(record.calls.fetch_add(1, Ordering::SeqCst))
                },
            );
        let (addr, server) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::spawn(server);
        let endpoint = Endpoint {
            base_url: format!("http://{addr}/v1/"),
            api_key: Some("dummy-token-abcd".to_string()),
        };
        (endpoint, seen)
    }

    fn recorded(body: &'static str) -> warp::http::Response<String> {
        warp::http::Response::builder()
            .header("content-type", "text/event-stream")
            .body(body.to_string())
            .unwrap()
    }

    fn status(code: u16, body: &str) -> warp::http::Response<String> {
        warp::http::Response::builder()
            .status(code)
            .body(body.to_string())
            .unwrap()
    }

    fn request() -> ChatRequest {
        ChatRequest {
            model: "m".to_string(),
            ..ChatRequest::default()
        }
    }

    const QUICK_RETRY: RetryPolicy = RetryPolicy {
        max_retries: 2,
        base_delay: Duration::from_millis(1),
    };

    #[tokio::test]
    async fn a_recorded_tool_call_stream_is_read_to_the_end_with_the_bearer_token() {
        let (endpoint, seen) =
            serve(|_| recorded(include_str!("../tests/fixtures/openai_tool_calls.sse"))).await;
        let provider = CustomOpenAi { endpoint };

        let chat = complete(&provider, &request(), RetryPolicy::NONE)
            .await
            .expect("completion succeeds");

        assert_eq!(chat.tool_calls.len(), 2);
        assert_eq!(chat.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chat.usage.unwrap().prompt_tokens, 2_048);
        assert_eq!(seen.bodies.lock().unwrap()[0]["stream"], true);
        assert_eq!(
            seen.headers.lock().unwrap()[0].0.as_deref(),
            Some("Bearer dummy-token-abcd")
        );
    }

    /// Runs both calls against one recording; returns what each produced.
    async fn streamed_and_plain(
        recording: &'static str,
    ) -> (CompletedChat, Vec<StreamDelta>, CompletedChat) {
        let (endpoint, _) = serve(move |_| recorded(recording)).await;
        let provider = CustomOpenAi { endpoint };
        let mut deltas = Vec::new();
        let streamed =
            complete_with_deltas(&provider, &request(), RetryPolicy::NONE, |d| deltas.push(d))
                .await
                .expect("streamed completion succeeds");
        let plain = complete(&provider, &request(), RetryPolicy::NONE)
            .await
            .expect("plain completion succeeds");
        (streamed, deltas, plain)
    }

    fn strip_timings(mut chat: CompletedChat) -> CompletedChat {
        if let Some(usage) = chat.usage.as_mut() {
            if let Some(details) = usage.details.as_mut() {
                details.eval_duration = None;
                details.prompt_eval_duration = None;
                details.time_to_first_token = None;
                if *details == UsageDetails::default() {
                    usage.details = None;
                }
            }
        }
        chat
    }

    #[tokio::test]
    async fn streamed_text_arrives_in_order_and_the_final_answer_equals_the_plain_call() {
        let (streamed, deltas, plain) =
            streamed_and_plain(include_str!("../tests/fixtures/vllm_reasoning.sse")).await;

        let streamed_details = streamed
            .usage
            .as_ref()
            .and_then(|u| u.details.as_ref())
            .unwrap();
        assert!(streamed_details.eval_duration.is_some_and(|ms| ms > 0.0));
        assert!(streamed_details.prompt_eval_duration.is_some());
        assert!(streamed_details.time_to_first_token.is_some());

        let plain = strip_timings(plain);
        let streamed = strip_timings(streamed);
        assert_eq!(streamed, plain);
        let pieces: Vec<&str> = deltas.iter().filter_map(|d| d.content.as_deref()).collect();
        assert_eq!(pieces.concat(), plain.content);
        let thinking: String = deltas
            .iter()
            .filter_map(|d| d.thinking.as_deref())
            .collect();
        assert_eq!(thinking, plain.thinking);
        // Reasoning comes before the answer, and the finish comes last.
        let last_thinking = deltas.iter().rposition(|d| d.thinking.is_some()).unwrap();
        let first_content = deltas.iter().position(|d| d.content.is_some()).unwrap();
        assert!(last_thinking < first_content);
        let finish = deltas
            .iter()
            .position(|d| d.finish_reason.is_some())
            .unwrap();
        assert!(finish >= deltas.iter().rposition(|d| d.content.is_some()).unwrap());
        assert_eq!(deltas[finish].finish_reason, plain.finish_reason);
    }

    #[tokio::test]
    async fn streamed_tool_calls_add_up_to_the_plain_calls() {
        let (streamed, deltas, plain) =
            streamed_and_plain(include_str!("../tests/fixtures/openai_tool_calls.sse")).await;

        let streamed_details = streamed
            .usage
            .as_ref()
            .and_then(|u| u.details.as_ref())
            .unwrap();
        assert!(streamed_details.eval_duration.is_some_and(|ms| ms > 0.0));

        let plain = strip_timings(plain);
        let streamed = strip_timings(streamed);
        assert_eq!(streamed, plain);
        assert_eq!(plain.tool_calls.len(), 2);
        for (index, call) in plain.tool_calls.iter().enumerate() {
            let arguments: String = deltas
                .iter()
                .flat_map(|d| &d.tool_calls)
                .filter(|fragment| fragment.index == index as u64)
                .map(|fragment| fragment.function.arguments.as_str())
                .collect();
            assert_eq!(arguments, call.function.arguments);
        }
    }

    #[test]
    fn attach_timing_skips_zero_completion_tokens_and_preserves_provider_eval_duration() {
        let start = Instant::now();
        let first = start + Duration::from_millis(50);
        let end = first + Duration::from_millis(200);

        let mut zero_completion = CompletedChat {
            usage: Some(crate::types::Usage {
                prompt_tokens: 10,
                completion_tokens: 0,
                total_tokens: 10,
                ..Default::default()
            }),
            ..Default::default()
        };
        attach_timing(&mut zero_completion, start, Some(first), end);
        assert!(zero_completion.usage.unwrap().details.is_none());

        let mut provider_timed = CompletedChat {
            usage: Some(crate::types::Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                details: Some(UsageDetails {
                    eval_duration: Some(999.0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        attach_timing(&mut provider_timed, start, Some(first), end);
        let details = provider_timed.usage.unwrap().details.unwrap();
        assert_eq!(details.eval_duration, Some(999.0));
        assert_eq!(details.time_to_first_token, Some(50.0));
    }

    #[tokio::test]
    async fn provider_headers_are_sent() {
        let (endpoint, seen) =
            serve(|_| recorded(include_str!("../tests/fixtures/openai_text.sse"))).await;
        let provider = OpenRouter { endpoint };

        complete(&provider, &request(), RetryPolicy::NONE)
            .await
            .expect("completion succeeds");

        assert_eq!(
            seen.headers.lock().unwrap()[0].1.as_deref(),
            Some("LibrAgent")
        );
    }

    #[tokio::test]
    async fn http_errors_are_classified_and_keep_the_provider_message() {
        let (endpoint, _) =
            serve(|_| status(401, r#"{"error":{"message":"Incorrect API key provided"}}"#)).await;
        let provider = CustomOpenAi { endpoint };

        let error = complete(&provider, &request(), RetryPolicy::NONE)
            .await
            .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Authentication);
        assert!(error.message.contains("HTTP 401"));
        assert!(error.message.contains("Incorrect API key"));
    }

    /// The retry loop alone: `complete` turns retries off for the loopback mock.
    async fn complete_with_retries(
        provider: &dyn ChatProvider,
        retry: RetryPolicy,
    ) -> Result<reqwest::Response, LlmError> {
        let client = HTTP_CLIENT.as_ref().unwrap();
        let url = provider.request_url("m");
        send_with_retries(
            client,
            provider,
            &url,
            &provider.build_body(&request()),
            retry,
        )
        .await
    }

    #[tokio::test]
    async fn a_server_error_is_retried_until_it_succeeds() {
        let (endpoint, seen) = serve(|call| match call {
            0 => status(503, "overloaded"),
            _ => recorded(include_str!("../tests/fixtures/openai_text.sse")),
        })
        .await;
        let provider = CustomOpenAi { endpoint };

        let response = complete_with_retries(&provider, QUICK_RETRY).await;

        assert!(response.unwrap().status().is_success());
        assert_eq!(seen.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn ollama_sends_again_without_the_effort_level_a_model_rejects() {
        let (mock, seen) = serve(|call| match call {
            0 => status(
                400,
                r#"{"error":{"message":"think value \"low\" is not supported for this model"}}"#,
            ),
            _ => recorded(include_str!("../tests/fixtures/ollama_v1_reasoning.sse")),
        })
        .await;
        // Ollama's Base URL is the server root; the provider adds `/v1`.
        let provider = Ollama {
            endpoint: Endpoint {
                base_url: mock.base_url.trim_end_matches("/v1/").to_string(),
                api_key: None,
            },
        };
        let request = ChatRequest {
            thinking_effort: ThinkingEffort::Low,
            ..request()
        };

        let chat = complete(&provider, &request, RetryPolicy::NONE)
            .await
            .expect("the second call succeeds");

        assert_eq!(chat.content, "2 + 2 = 4.");
        let bodies = seen.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0]["reasoning_effort"], "low");
        assert!(bodies[1].get("reasoning_effort").is_none());
        assert_eq!(bodies[1]["max_tokens"], 4096);
    }

    #[tokio::test]
    async fn a_rejected_effort_level_is_fixed_once_only() {
        let rejected =
            r#"{"error":{"message":"think value \"low\" is not supported for this model"}}"#;
        let (mock, seen) = serve(move |_| status(400, rejected)).await;
        let provider = Ollama {
            endpoint: Endpoint {
                base_url: mock.base_url.trim_end_matches("/v1/").to_string(),
                api_key: None,
            },
        };
        let request = ChatRequest {
            thinking_effort: ThinkingEffort::Low,
            ..request()
        };

        let error = complete(&provider, &request, RetryPolicy::NONE)
            .await
            .unwrap_err();

        assert!(error.message.contains("HTTP 400"));
        assert_eq!(seen.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_client_error_is_not_retried_and_retries_stop_at_the_limit() {
        let (endpoint, seen) = serve(|_| status(400, "bad request")).await;
        let provider = CustomOpenAi { endpoint };
        assert!(complete_with_retries(&provider, QUICK_RETRY).await.is_err());
        assert_eq!(seen.calls.load(Ordering::SeqCst), 1);

        let (endpoint, seen) = serve(|_| status(500, "down")).await;
        let provider = CustomOpenAi { endpoint };
        let error = complete_with_retries(&provider, QUICK_RETRY)
            .await
            .unwrap_err();
        assert_eq!(error.status, Some(500));
        assert_eq!(seen.calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_self_hosted_endpoint_is_never_retried() {
        let (endpoint, seen) = serve(|_| status(502, "provider exploded")).await;
        let provider = CustomOpenAi { endpoint };

        let error = complete(&provider, &request(), QUICK_RETRY)
            .await
            .unwrap_err();

        assert!(error.message.contains("HTTP 502"));
        assert_eq!(seen.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_unreachable_endpoint_is_a_network_error() {
        // Bind then drop, so the port is closed.
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let provider = CustomOpenAi {
            endpoint: Endpoint {
                base_url: format!("http://127.0.0.1:{port}/v1"),
                api_key: None,
            },
        };

        let error = complete(&provider, &request(), QUICK_RETRY)
            .await
            .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Network);
        assert!(error.is_retryable());
    }

    #[test]
    fn retry_delays_double() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
        };
        assert_eq!(policy.delay_before_retry(0), Duration::from_secs(1));
        assert_eq!(policy.delay_before_retry(3), Duration::from_secs(8));
        // No overflow, however many retries.
        assert!(policy.delay_before_retry(40) >= Duration::from_secs(1 << 20));
    }

    #[test]
    fn self_hosted_hosts_are_recognised() {
        for url in [
            "http://localhost:11434",
            "http://127.0.0.1:8000/v1",
            "http://[::1]:8000",
            "http://host.docker.internal:1234/v1",
            "http://gpu-box.local/v1",
            "http://10.0.0.5:8000",
            "http://192.168.1.20/v1",
            "http://172.20.3.4/v1",
            "127.0.0.1:11434",
        ] {
            assert!(is_self_hosted(url), "{url}");
        }
        for url in [
            "https://api.openai.com/v1",
            "https://openrouter.ai/api/v1",
            "http://172.32.0.1/v1",
            "",
            "not a url at all",
        ] {
            assert!(!is_self_hosted(url), "{url}");
        }
    }
}
