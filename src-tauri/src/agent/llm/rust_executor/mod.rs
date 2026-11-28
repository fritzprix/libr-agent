//! LLM execution in Rust for the providers `llm-client` implements (#2704,
//! #2755, #2760, #3094).
//!
//! Every LLM request is answered here: a session whose provider (and Base
//! URL) `llm-client` supports runs in this executor, and a provider it does
//! not support fails with a clear error. There is no WebView fallback and no
//! env override — every GUI provider is supported, so a request that went to
//! the WebView could never be answered anyway. [`configure`] replaces the
//! endpoint lookup and the delivery target for the life of its guard, so a
//! test can run a whole turn without touching the process environment or
//! needing a window.
//!
//! The HTTP calls go through the Tauri-free `llm-client` crate, which the GUI,
//! `libragent --tui`, and `liblet` share. This module decides who answers a
//! request, reads the provider and generation settings, and delivers results.
//!
//! Covered: the OpenAI-compatible providers (`openai`, `fireworks`,
//! `openrouter`, `groq`, `cerebras`, `ollama`, and `custom:<id>`, for example
//! vLLM or LM Studio), `anthropic`, and `gemini`; streaming text, reasoning and
//! its signature, tool calls, images and audio in the request, compaction, and
//! the GUI's temperature, thinking-effort, retry, and prompt-cache behaviour.
//!
//! While a call runs, each streamed piece goes out as an
//! [`crate::agent::events::AgentEvent::LlmStreamDelta`] (#2786), so the GUI
//! shows the turn live. Mid-stream loop and reasoning-budget detection is not
//! done here: the GUI feeds those events into its `StreamAccumulator`, which
//! reports an issue back and cancels the call. Covered here too, like the
//! frontend: Gemini's one-time retry without tools after a malformed function
//! call (#3089), in `completion.rs`.

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
use config::{no_compaction_parent, no_session_manager, route_for, unavailable, Route};
use events::manager_sink;
use std::sync::Arc;
use tasks::spawn_tracked;
use tauri::AppHandle;

pub use config::{configure, ConfigGuard, ExecutorConfig};
pub use events::ResponseSink;
pub use llm_client::Endpoint;
pub use tasks::cancel;

/// Answer `request` in Rust. An unsupported provider (or `empty`) returns an
/// error that names it; nothing waits for a WebView.
pub async fn try_dispatch_completion(
    app_handle: &AppHandle,
    request: &CompletionRequest,
) -> Result<(), Box<AgentRuntimeError>> {
    dispatch_completion(request, || manager_sink(app_handle)).await
}

/// [`try_dispatch_completion`] with the session-manager lookup passed in, for
/// a caller (a test) that cannot reach app state.
pub async fn dispatch_completion(
    request: &CompletionRequest,
    manager: impl FnOnce() -> Option<Arc<dyn ResponseSink>>,
) -> Result<(), Box<AgentRuntimeError>> {
    let (route, config) = route_for(&request.provider);
    if route == Route::Unavailable {
        return Err(Box::new(unavailable(&request.provider)));
    }
    // The request was routed to Rust, so dropping it here would silently lose
    // the turn: fail instead.
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
    Ok(())
}

/// Compaction counterpart of [`try_dispatch_completion`].
pub async fn try_dispatch_compaction(
    app_handle: &AppHandle,
    request: &CompactRequest,
) -> Result<(), Box<AgentRuntimeError>> {
    dispatch_compaction(request, || manager_sink(app_handle)).await
}

/// [`try_dispatch_compaction`] with the session-manager lookup passed in.
pub async fn dispatch_compaction(
    request: &CompactRequest,
    manager: impl FnOnce() -> Option<Arc<dyn ResponseSink>>,
) -> Result<(), Box<AgentRuntimeError>> {
    // Without the parent request there is no model to call, so no provider to
    // route by: fail with a clear error instead of going to the WebView
    // (#3094).
    let Some(parent) = request.parent_request.as_ref() else {
        return Err(Box::new(no_compaction_parent()));
    };
    let (route, config) = route_for(&parent.provider);
    if route == Route::Unavailable {
        return Err(Box::new(unavailable(&parent.provider)));
    }
    // The request was routed to Rust, so dropping it here would silently lose
    // the summary: fail instead.
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
    Ok(())
}
