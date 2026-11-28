use super::completion::assistant_message;
use super::config::{no_compaction_parent, no_session_manager, route, unavailable, Route};
use super::convert;
use super::events::{fallback_notice_event, stream_delta_event};
use super::settings::{runtime_error, FallbackModel};
use super::tasks::{cancel, spawn_tracked, IN_FLIGHT};
use crate::agent::llm::types::{AgentRuntimeErrorType, CompletionRequest};
use llm_client::{CompletedChat, ErrorKind, LlmError, StreamDelta};
use serde_json::json;
use tokio::sync::oneshot;

#[test]
fn the_removed_executor_env_var_changes_nothing() {
    // #3094 removed LIBRAGENT_LLM_EXECUTOR. A stale value left in a user's
    // environment must not resurrect the WebView route.
    std::env::set_var("LIBRAGENT_LLM_EXECUTOR", "webview");
    assert_eq!(route("openai"), Route::Rust);
    assert_eq!(route("mistral"), Route::Unavailable);
    std::env::remove_var("LIBRAGENT_LLM_EXECUTOR");
}

#[test]
fn client_errors_keep_their_kind_and_code() {
    let cases = [
        (ErrorKind::Network, AgentRuntimeErrorType::NetworkError),
        (
            ErrorKind::Authentication,
            AgentRuntimeErrorType::AuthenticationError,
        ),
        (ErrorKind::RateLimit, AgentRuntimeErrorType::RateLimitError),
        (ErrorKind::Service, AgentRuntimeErrorType::AiServiceError),
    ];
    for (kind, expected) in cases {
        let error = runtime_error(LlmError {
            kind,
            code: "RUST_EXECUTOR_HTTP",
            message: "boom".to_string(),
            status: None,
        });
        assert_eq!(
            std::mem::discriminant(&error.error_type),
            std::mem::discriminant(&expected)
        );
        let code = error
            .details
            .as_ref()
            .and_then(|details| details.error_code.as_deref());
        assert_eq!(code, Some("RUST_EXECUTOR_HTTP"));
    }
}

#[test]
fn the_assistant_message_answers_the_request_it_was_made_for() {
    let request = CompletionRequest {
        session_id: "s1".to_string(),
        response_message_id: "resp-1".to_string(),
        messages: Vec::new(),
        model: "m".to_string(),
        provider: "openai".to_string(),
        system_prompt: None,
        max_tokens: None,
        available_tools: None,
    };
    let chat = CompletedChat {
        content: "done".to_string(),
        thinking: String::new(),
        thinking_signature: Some("sig".to_string()),
        tool_calls: Vec::new(),
        usage: Some(llm_client::Usage {
            prompt_tokens: 7,
            completion_tokens: 1,
            total_tokens: 8,
            cached_prompt_tokens: None,
            details: None,
        }),
        finish_reason: Some("stop".to_string()),
    };

    let message = assistant_message(&request, chat);

    // The agent loop matches the reply to the turn by this id.
    assert_eq!(message.id, "resp-1");
    assert_eq!(message.session_id, "s1");
    assert_eq!(message.role, "assistant");
    assert_eq!(convert::text_of(&message.content), "done");
    assert!(message.tool_calls.is_none());
    assert!(message.thinking.is_none());
    // Anthropic and Gemini need it back with the next request.
    assert_eq!(message.thinking_signature.as_deref(), Some("sig"));
    assert_eq!(message.prompt_tokens, Some(7));
    assert_eq!(
        message.usage,
        Some(json!({ "promptTokens": 7, "completionTokens": 1, "totalTokens": 8 }))
    );
}

#[test]
fn a_fallback_notice_names_the_status_the_model_and_the_raw_error() {
    let request = CompletionRequest {
        session_id: "s1".to_string(),
        response_message_id: "resp-1".to_string(),
        messages: Vec::new(),
        model: "big".to_string(),
        provider: "custom:llama".to_string(),
        system_prompt: None,
        max_tokens: None,
        available_tools: None,
    };
    let primary_error = runtime_error(LlmError::from_http_status(
        400,
        "context length exceeded: request is too large",
    ));
    let fallback = FallbackModel {
        provider: "openai".to_string(),
        model: "gpt-4o-mini".to_string(),
    };

    let event =
        serde_json::to_value(fallback_notice_event(&request, &primary_error, &fallback)).unwrap();

    assert_eq!(
        event,
        json!({
            "type": "llmFallbackNotice",
            "sessionId": "s1",
            "statusCode": 400,
            "fallbackProvider": "openai",
            "fallbackModel": "gpt-4o-mini",
            "errorMessage": "LLM provider returned HTTP 400: context length exceeded: request is too large",
        })
    );
}

#[test]
fn a_fallback_notice_without_an_http_status_omits_the_status() {
    let request = CompletionRequest {
        session_id: "s1".to_string(),
        response_message_id: "resp-1".to_string(),
        messages: Vec::new(),
        model: "big".to_string(),
        provider: "openai".to_string(),
        system_prompt: None,
        max_tokens: None,
        available_tools: None,
    };
    let primary_error = runtime_error(LlmError::new(
        ErrorKind::Network,
        "RUST_EXECUTOR_NETWORK",
        "down",
    ));
    let fallback = FallbackModel {
        provider: "anthropic".to_string(),
        model: "claude-sonnet".to_string(),
    };

    let event =
        serde_json::to_value(fallback_notice_event(&request, &primary_error, &fallback)).unwrap();

    // The notice still carries the model name and the raw error.
    assert_eq!(event["type"], "llmFallbackNotice");
    assert!(event.get("statusCode").is_none());
    assert_eq!(event["fallbackModel"], "claude-sonnet");
    assert_eq!(event["errorMessage"], "down");
}

#[test]
fn a_stream_delta_event_names_its_turn_and_serializes_for_the_frontend() {
    let request = CompletionRequest {
        session_id: "s1".to_string(),
        response_message_id: "resp-1".to_string(),
        messages: Vec::new(),
        model: "m".to_string(),
        provider: "anthropic".to_string(),
        system_prompt: None,
        max_tokens: Some(4096),
        available_tools: None,
    };
    let delta = StreamDelta {
        content: Some("hi".to_string()),
        ..StreamDelta::default()
    };

    let event = serde_json::to_value(stream_delta_event(&request, delta)).unwrap();

    assert_eq!(
        event,
        json!({
            "type": "llmStreamDelta",
            "sessionId": "s1",
            "responseMessageId": "resp-1",
            "provider": "anthropic",
            "maxTokens": 4096,
            "delta": { "content": "hi" },
        })
    );
}

#[test]
fn every_supported_provider_routes_to_rust_and_nothing_else() {
    // There is no WebView route anymore (#3094): llm-client-supported
    // providers run here, and a configured Base URL is honored for every
    // built-in provider (#2797), so it never changes the route; see
    // `every_built_in_provider_honors_its_configured_base_url` in
    // llm-client and `configured_anthropic_base_url_routes_to_rust` in
    // the integration tests.
    assert_eq!(route("openai"), Route::Rust);
    assert_eq!(route("custom:lmstudio"), Route::Rust);
    assert_eq!(route("fireworks"), Route::Rust);
    assert_eq!(route("openrouter"), Route::Rust);
    assert_eq!(route("anthropic"), Route::Rust);
    assert_eq!(route("gemini"), Route::Rust);
    assert_eq!(route("groq"), Route::Rust);
    assert_eq!(route("cerebras"), Route::Rust);
    assert_eq!(route("ollama"), Route::Rust);
    // Anything else fails with a clear error instead of waiting for a
    // frontend that could not answer it.
    assert_eq!(route("mistral"), Route::Unavailable);
    assert_eq!(route(""), Route::Unavailable);
}

#[test]
fn an_unavailable_route_says_which_provider_cannot_run() {
    let error = unavailable("mistral");
    let code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref());
    assert_eq!(code, Some("RUST_EXECUTOR_UNSUPPORTED_PROVIDER"));
    assert!(error.display_message.contains("mistral"));
}

#[test]
fn a_compaction_without_a_parent_says_there_is_no_model_to_call() {
    let error = no_compaction_parent();
    let code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref());
    assert_eq!(code, Some("RUST_EXECUTOR_NO_COMPACTION_PARENT"));
    assert!(error.display_message.contains("parent"));
}

#[test]
fn a_missing_session_manager_names_the_missing_receiver() {
    let error = no_session_manager();
    let code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref());
    assert_eq!(code, Some("RUST_EXECUTOR_NO_SESSION_MANAGER"));
    assert!(error.display_message.contains("session manager"));
}

#[tokio::test]
async fn a_call_that_ends_at_once_leaves_no_entry_behind() {
    let (done, finished) = oneshot::channel::<()>();
    spawn_tracked("early-finish", async move {
        let _ = done.send(());
    });
    finished.await.unwrap();
    // The entry is removed right after the work, on the task itself.
    for _ in 0..100 {
        if !IN_FLIGHT.contains_key("early-finish") {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("a finished call left its in-flight entry");
}

#[tokio::test]
async fn a_replacing_call_aborts_the_old_one_and_keeps_its_own_entry() {
    let (old_started, old_running) = oneshot::channel::<()>();
    let (old_dropped, old_was_aborted) = oneshot::channel::<()>();
    spawn_tracked("replaced", async move {
        // Dropped when the task is aborted, which signals the test.
        let _signal = old_dropped;
        let _ = old_started.send(());
        std::future::pending::<()>().await;
    });
    old_running.await.unwrap();

    let (release, wait_release) = oneshot::channel::<()>();
    spawn_tracked("replaced", async move {
        let _ = wait_release.await;
    });

    // The old call was aborted: its sender is dropped without a send.
    assert!(old_was_aborted.await.is_err());
    // The old call's end did not remove the new call's entry.
    assert!(IN_FLIGHT.contains_key("replaced"));
    let _ = release.send(());
    cancel("replaced");
    assert!(!IN_FLIGHT.contains_key("replaced"));
}
