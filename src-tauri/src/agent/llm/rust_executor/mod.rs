//! LLM execution in Rust for the providers `llm-client` implements (#2704,
//! #2755, #2760).
//!
//! By default, a session whose provider (and Base URL) `llm-client` supports
//! is answered here. Providers the client does not support yet (mistral, for
//! example) still go to the WebView via `llm:completion-request` /
//! `llm:compact-request`. Set `LIBRAGENT_LLM_EXECUTOR=webview` to force every
//! call onto the frontend. [`configure`] replaces the variable, the endpoint
//! lookup, the WebView check and the delivery target for the life of its
//! guard, so a test can run a whole turn without touching the process
//! environment or needing a window.
//!
//! A request that Rust cannot answer still goes to the WebView when there is
//! one, and fails fast when there is none, because nothing would ever answer it.
//!
//! The HTTP calls go through the Tauri-free `llm-client` crate, which the GUI,
//! `libragent --tui`, and `liblet` share. This module decides who answers a
//! request, reads the provider and generation settings, and delivers results.
//!
//! Covered: the OpenAI-compatible providers (`openai`, `fireworks`,
//! `openrouter`, and `custom:<id>`, for example vLLM or LM Studio),
//! `anthropic`, and `gemini`; streaming text, reasoning and its signature,
//! tool calls, images and audio in the request, compaction, and the GUI's
//! temperature, thinking-effort, retry, and prompt-cache behaviour.
//!
//! While a call runs, each streamed piece goes out as an
//! [`crate::agent::events::AgentEvent::LlmStreamDelta`] (#2786), so the GUI
//! shows the turn live. Mid-stream loop and reasoning-budget detection is not
//! done here: the GUI feeds those events into its `StreamAccumulator`, which
//! reports an issue back and cancels the call. Not covered here (the frontend
//! has them): retry with a fallback model, and Gemini's retry without tools
//! after a malformed function call.

mod completion;
mod config;
mod convert;
mod events;
mod settings;
mod tasks;

#[cfg(test)]
mod tests;

use crate::agent::llm::types::{AgentRuntimeError, CompactRequest, CompletionRequest};
use completion::{run_compaction, run_completion};
use config::{no_session_manager, route_for, unavailable, Route};
use events::manager_sink;
use std::sync::Arc;
use tasks::spawn_tracked;
use tauri::AppHandle;

pub use config::{
    configure, is_enabled, runs_without_webview, ConfigGuard, ExecutorConfig, EXECUTOR_ENV,
};
pub use events::ResponseSink;
pub use llm_client::Endpoint;
pub use tasks::cancel;

/// Answer `request` in Rust when the executor is enabled and the provider is
/// one `llm-client` supports. `Ok(false)` means the frontend must handle it; an error
/// means nothing can.
pub async fn try_dispatch_completion(
    app_handle: &AppHandle,
    request: &CompletionRequest,
) -> Result<bool, Box<AgentRuntimeError>> {
    dispatch_completion(app_handle, request, || manager_sink(app_handle)).await
}

/// [`try_dispatch_completion`] with the session-manager lookup passed in, for
/// a caller (a test) that cannot reach app state through its handle.
pub async fn dispatch_completion(
    app_handle: &AppHandle,
    request: &CompletionRequest,
    manager: impl FnOnce() -> Option<Arc<dyn ResponseSink>>,
) -> Result<bool, Box<AgentRuntimeError>> {
    let (route, config) = route_for(app_handle, &request.provider).await;
    match route {
        Route::Frontend => return Ok(false),
        Route::Unavailable => return Err(Box::new(unavailable(&request.provider))),
        Route::Rust => {}
    }
    // The route said Rust can answer, so falling back to the WebView now would
    // silently lose the request when there is none: fail instead.
    let Some(sink) = manager() else {
        return Err(Box::new(no_session_manager()));
    };

    let request = request.clone();
    let session_id = request.session_id.clone();
    spawn_tracked(&session_id, async move {
        let session_id = request.session_id.clone();
        let delivered = match run_completion(&request, &config, sink.as_ref()).await {
            Ok(message) => sink.llm_response(session_id.clone(), message).await,
            Err(error) => {
                log::warn!(
                    "Rust LLM executor: completion failed for session {session_id}: {}",
                    error.display_message
                );
                sink.llm_error(session_id.clone(), *error).await
            }
        };
        if let Err(error) = delivered {
            log::error!(
                "Rust LLM executor: could not deliver the result for session {session_id}: {error}"
            );
        }
    });
    log::info!("Rust LLM executor: running completion for session {session_id}");
    Ok(true)
}

/// Compaction counterpart of [`try_dispatch_completion`].
pub async fn try_dispatch_compaction(
    app_handle: &AppHandle,
    request: &CompactRequest,
) -> Result<bool, Box<AgentRuntimeError>> {
    dispatch_compaction(app_handle, request, || manager_sink(app_handle)).await
}

/// [`try_dispatch_compaction`] with the session-manager lookup passed in.
pub async fn dispatch_compaction(
    app_handle: &AppHandle,
    request: &CompactRequest,
    manager: impl FnOnce() -> Option<Arc<dyn ResponseSink>>,
) -> Result<bool, Box<AgentRuntimeError>> {
    // Without the parent request there is no model to call, so no provider to
    // route by: the frontend gets it, as before.
    let provider = request
        .parent_request
        .as_ref()
        .map(|parent| parent.provider.as_str())
        .unwrap_or_default();
    let (route, config) = route_for(app_handle, provider).await;
    match route {
        Route::Frontend => return Ok(false),
        Route::Unavailable => return Err(Box::new(unavailable(provider))),
        Route::Rust => {}
    }
    let Some(parent) = request.parent_request.as_ref() else {
        return Ok(false);
    };
    // The route said Rust can answer, so falling back to the WebView now would
    // silently lose the request when there is none: fail instead.
    let Some(sink) = manager() else {
        return Err(Box::new(no_session_manager()));
    };

    let request = request.clone();
    let model = parent.model.clone();
    let provider = parent.provider.clone();
    let system_prompt = parent.system_prompt.clone();
    let session_id = request.session_id.clone();
    spawn_tracked(&session_id, async move {
        let session_id = request.session_id.clone();
        let outcome = run_compaction(
            &request,
            &model,
            &provider,
            system_prompt.as_deref(),
            &config,
        )
        .await;
        let delivered = match outcome {
            Ok((summary, meta)) => {
                sink.compact_response(&session_id, &request, summary, meta)
                    .await
            }
            Err(error) => {
                log::warn!(
                    "Rust LLM executor: compaction failed for session {session_id}: {}",
                    error.display_message
                );
                sink.compact_error(session_id.clone(), *error).await
            }
        };
        if let Err(error) = delivered {
            log::error!(
                "Rust LLM executor: could not deliver the compaction result for session {session_id}: {error}"
            );
        }
    });
    log::info!("Rust LLM executor: running compaction for session {session_id}");
    Ok(true)
}
