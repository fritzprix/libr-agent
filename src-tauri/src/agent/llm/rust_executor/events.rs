use super::settings::{FallbackModel, HTTP_STATUS_CONTEXT_KEY};
use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{AgentRuntimeError, CompactRequest, CompletionRequest};
use crate::agent::session_manager::CompactStreamMeta;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::agent::AgentSessionManager;
use crate::models::chat::Message;
use llm_client::StreamDelta;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// The event that carries `delta` of the call answering `request`.
pub(super) fn stream_delta_event(request: &CompletionRequest, delta: StreamDelta) -> AgentEvent {
    AgentEvent::LlmStreamDelta {
        session_id: request.session_id.clone(),
        response_message_id: request.response_message_id.clone(),
        provider: request.provider.clone(),
        max_tokens: request.max_tokens,
        delta,
    }
}

/// The event that withdraws the live message of `request` (#3089).
///
/// Emitted when the executor throws away an attempt it already streamed
/// (the Gemini malformed-function-call retry) and will answer again under
/// the same response message id.
pub(super) fn stream_reset_event(request: &CompletionRequest) -> AgentEvent {
    AgentEvent::LlmStreamReset {
        session_id: request.session_id.clone(),
        response_message_id: request.response_message_id.clone(),
    }
}

/// The notice that the primary model failed and the fallback is being tried (#2961).
///
/// `status_code` is the HTTP status `runtime_error` stashed in the primary
/// error's context; `None` for failures that never reached an HTTP response.
/// `error_message` is the raw server error the user can expand in the chat.
pub(super) fn fallback_notice_event(
    request: &CompletionRequest,
    primary_error: &AgentRuntimeError,
    fallback: &FallbackModel,
) -> AgentEvent {
    let status_code = primary_error
        .details
        .as_ref()
        .and_then(|details| details.context.as_ref())
        .and_then(|context| context.get(HTTP_STATUS_CONTEXT_KEY))
        .and_then(|status| status.as_u64().and_then(|value| u16::try_from(value).ok()));
    AgentEvent::LlmFallbackNotice {
        session_id: request.session_id.clone(),
        status_code,
        fallback_provider: fallback.provider.clone(),
        fallback_model: fallback.model.clone(),
        error_message: primary_error.display_message.clone(),
    }
}

/// Where an executor result is delivered: the session manager in the app.
///
/// A trait so a test can deliver through the real session handlers without a
/// full Tauri runtime, which cannot be cloned for a mock window in tests.
#[async_trait::async_trait]
pub trait ResponseSink: Send + Sync {
    /// One live piece of the running call, as an [`AgentEvent::LlmStreamDelta`].
    ///
    /// Sync and best effort: a lost delta only makes the live view lag, because
    /// the final message still arrives through [`Self::llm_response`]. The
    /// default drops it, for a target that has no live view.
    fn llm_stream_delta(&self, _event: AgentEvent) {}

    /// The streamed attempt was thrown away and will be answered again
    /// under the same response message id (#3089).
    ///
    /// Sync and best effort like [`Self::llm_stream_delta`]: a missed reset
    /// only leaves the discarded text on screen until the final message
    /// replaces it.
    fn llm_stream_reset(&self, _event: AgentEvent) {}

    /// The primary model failed and the fallback model is being tried (#2961).
    ///
    /// Sync and best effort like [`Self::llm_stream_delta`]: a lost notice only
    /// hides the explanation; the fallback answer still arrives.
    fn llm_fallback_notice(&self, _event: AgentEvent) {}

    async fn llm_response(&self, session_id: String, message: Message) -> Result<(), String>;
    async fn llm_error(&self, session_id: String, error: AgentRuntimeError) -> Result<(), String>;
    async fn compact_response(
        &self,
        session_id: &str,
        request: &CompactRequest,
        summary: String,
        meta: CompactStreamMeta,
    ) -> Result<(), String>;
    async fn compact_error(
        &self,
        session_id: String,
        error: AgentRuntimeError,
    ) -> Result<(), String>;
}

#[async_trait::async_trait]
impl ResponseSink for AgentSessionManager {
    fn llm_stream_delta(&self, event: AgentEvent) {
        let dispatcher = TauriEventDispatcher::from_handle(self.app_handle());
        if let Err(error) = dispatcher.emit_agent_event(event) {
            log::debug!("Rust LLM executor: could not emit a stream delta: {error}");
        }
    }

    fn llm_stream_reset(&self, event: AgentEvent) {
        let dispatcher = TauriEventDispatcher::from_handle(self.app_handle());
        if let Err(error) = dispatcher.emit_agent_event(event) {
            log::debug!("Rust LLM executor: could not emit a stream reset: {error}");
        }
    }

    fn llm_fallback_notice(&self, event: AgentEvent) {
        let dispatcher = TauriEventDispatcher::from_handle(self.app_handle());
        if let Err(error) = dispatcher.emit_agent_event(event) {
            log::debug!("Rust LLM executor: could not emit the fallback notice: {error}");
        }
    }

    async fn llm_response(&self, session_id: String, message: Message) -> Result<(), String> {
        self.handle_llm_response(session_id, message).await
    }

    async fn llm_error(&self, session_id: String, error: AgentRuntimeError) -> Result<(), String> {
        self.handle_llm_error(session_id, error).await
    }

    async fn compact_response(
        &self,
        session_id: &str,
        request: &CompactRequest,
        summary: String,
        meta: CompactStreamMeta,
    ) -> Result<(), String> {
        self.handle_compact_response(
            session_id,
            request.to_id.clone(),
            request.compacted_delta_count,
            summary,
            Some(meta),
        )
        .await
        .map(|_| ())
    }

    async fn compact_error(
        &self,
        session_id: String,
        error: AgentRuntimeError,
    ) -> Result<(), String> {
        self.handle_compact_error(session_id, error, None).await
    }
}

/// The session manager kept in the app's managed state, as a delivery target.
pub(super) fn manager_sink(app_handle: &AppHandle) -> Option<Arc<dyn ResponseSink>> {
    app_handle
        .try_state::<AgentSessionManager>()
        .map(|manager| Arc::new(manager.inner().clone_for_task()) as Arc<dyn ResponseSink>)
}
