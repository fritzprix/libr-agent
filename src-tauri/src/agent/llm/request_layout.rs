use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

/// XML tag wrapper to isolate volatile session context (planning, workspace state, etc.)
/// within a synthetic user message for LLMs. This is cleaner than HTML comments for parsing,
/// prevents cache churn in system prompts, and clearly scopes background reference context.
const SESSION_CONTEXT_BACKGROUND_HEADER: &str = "<session-context>";
const SESSION_CONTEXT_BACKGROUND_FOOTER: &str = "</session-context>";

/// Short non-intent framing inside the injected block (keep to one line — long
/// sermons cause per-turn re-anchoring without improving role separation).
const SESSION_CONTEXT_DISCLAIMER: &str = "Session environment state (not a user request).";

/// Re-inject current SC (instead of lagged tn−1 snapshot) at least this often.
pub const SESSION_CONTEXT_HEARTBEAT_TURNS: u32 = 8;

#[derive(Debug, Clone)]
pub struct RequestLayout {
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
}

/// Inputs for ephemeral synthetic-user session-context injection (Phase 2).
#[derive(Debug, Clone, Default)]
pub struct SessionContextInjectState {
    /// Volatile SC built for this request (state @ tn).
    pub current: Option<String>,
    /// Snapshot stored after the previous request (state @ tn−1), when available.
    pub previous: Option<String>,
    /// Completions since the last force-fresh inject (heartbeat).
    pub turns_since_force_fresh: u32,
}

/// Result of resolving what to inject and what to persist for the next turn.
#[derive(Debug, Clone)]
pub struct SessionContextInjectOutcome {
    /// Text body to place in the synthetic user message (None → do not inject).
    pub inject_text: Option<String>,
    /// Snapshot to store as `previous` for the next request (usually `current`).
    pub next_previous: Option<String>,
    /// Reset heartbeat counter when true; otherwise increment.
    pub force_fresh: bool,
}

/// OpenAI-compatible custom providers use the `custom:<id>` session provider id.
pub fn is_custom_openai_compatible_provider(provider: &str) -> bool {
    provider
        .strip_prefix("custom:")
        .is_some_and(|id| !id.is_empty())
}

pub fn provider_uses_synthetic_session_context(provider: &str) -> bool {
    is_custom_openai_compatible_provider(provider)
        || matches!(
            provider,
            "openai" | "openrouter" | "fireworks" | "anthropic" | "gemini" | "ollama"
        )
}

fn provider_uses_background_reference_wrapper(provider: &str) -> bool {
    !matches!(provider, "anthropic")
}

fn synthetic_session_context_id_prefix(provider: &str) -> &str {
    if is_custom_openai_compatible_provider(provider) {
        return "custom-openai-session-context";
    }
    match provider {
        "openai" => "openai-session-context",
        "openrouter" => "openrouter-session-context",
        "fireworks" => "fireworks-session-context",
        "anthropic" => "anthropic-session-context",
        "gemini" => "gemini-session-context",
        "ollama" => "ollama-session-context",
        _ => "session-context",
    }
}

fn format_session_context_text(provider: &str, session_context: &str) -> String {
    let body = format!("{}\n\n{}", SESSION_CONTEXT_DISCLAIMER, session_context);
    if provider_uses_background_reference_wrapper(provider) {
        format!(
            "{}\n\n{}\n\n{}",
            SESSION_CONTEXT_BACKGROUND_HEADER, body, SESSION_CONTEXT_BACKGROUND_FOOTER
        )
    } else {
        // Anthropic historically omitted XML wrappers; keep the disclaimer either way.
        body
    }
}

/// End index of the durable conversation window, excluding trailing compaction overlays.
fn conversation_end_excluding_compaction_overlays(messages: &[Message]) -> usize {
    let mut end = messages.len();
    while end > 0 && messages[end - 1].is_compaction_overlay_message() {
        end -= 1;
    }
    end
}

/// Choose where to inject synthetic session context (Phase 2 / Option B).
///
/// Absolute tail is ruled out (TB2.1 BM: SC reads as user intent). Tracking the
/// *latest* assistant is also wrong: each new `aN` in a tool loop moves SC and
/// collapses the prompt-cache prefix.
///
/// Priority:
/// 1. Keep compaction overlays last (instruction attention).
/// 2. Anchor to the latest **external** user request `u`:
///    - If a response chain already exists after `u`, insert immediately before
///      the **first** assistant after that `u` (`[u, SC, a1, t1, a2, …]`).
///      The slot stays fixed for the whole tool loop.
///    - If no assistant follows `u` yet, insert immediately before `u` so the
///      real user request stays last (`[…, SC, u]` — not absolute tail when
///      prior history exists).
/// 3. No external user: before latest assistant, else pre-compaction tail.
pub fn synthetic_session_context_insert_index(messages: &[Message]) -> usize {
    let end = conversation_end_excluding_compaction_overlays(messages);
    let window = &messages[..end];

    if let Some(user_idx) = window
        .iter()
        .rposition(|message| message.is_external_request_message())
    {
        if let Some(offset) = window[user_idx + 1..]
            .iter()
            .position(|message| message.role == "assistant")
        {
            // Before first assistant of this user turn's response chain.
            return user_idx + 1 + offset;
        }
        // Awaiting first completion for this user: keep real request last.
        return user_idx;
    }

    if let Some(assistant_idx) = window
        .iter()
        .rposition(|message| message.role == "assistant")
    {
        return assistant_idx;
    }

    end
}

/// Whether this request should inject current SC instead of the lagged snapshot.
pub fn should_force_fresh_session_context(
    messages: &[Message],
    previous: Option<&str>,
    turns_since_force_fresh: u32,
) -> bool {
    if previous.is_none() {
        return true;
    }
    if !messages.iter().any(|message| message.role == "assistant") {
        return true;
    }
    if messages
        .last()
        .is_some_and(|message| message.is_compaction_overlay_message())
    {
        return true;
    }
    turns_since_force_fresh >= SESSION_CONTEXT_HEARTBEAT_TURNS
}

/// Resolve inject text + next snapshot from current/previous SC and message shape.
pub fn resolve_session_context_inject(
    messages: &[Message],
    state: &SessionContextInjectState,
) -> SessionContextInjectOutcome {
    let Some(current) = state
        .current
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .map(|text| text.to_string())
    else {
        return SessionContextInjectOutcome {
            inject_text: None,
            next_previous: None,
            force_fresh: true,
        };
    };

    let force_fresh = should_force_fresh_session_context(
        messages,
        state.previous.as_deref(),
        state.turns_since_force_fresh,
    );

    let inject_text = if force_fresh {
        current.clone()
    } else if let Some(previous) = state
        .previous
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
    {
        previous.to_string()
    } else {
        current.clone()
    };

    SessionContextInjectOutcome {
        inject_text: Some(inject_text),
        next_previous: Some(current),
        force_fresh,
    }
}

fn build_synthetic_session_context_message(
    provider: &str,
    session_id: &str,
    session_context: &str,
    reference_message_id: &str,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    Message {
        id: format!(
            "{}-{}",
            synthetic_session_context_id_prefix(provider),
            reference_message_id
        ),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: format_session_context_text(provider, session_context),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::SessionContext),
        error: None,
        metadata: None,
    }
}

/// Build the provider request layout. Uses **current** SC only (no lag cache).
/// Prefer [`build_request_layout_with_inject_state`] from the live completion path.
pub fn build_request_layout(
    provider: &str,
    session_id: &str,
    system_prompt: Option<String>,
    session_context: Option<String>,
    messages: Vec<Message>,
) -> RequestLayout {
    build_request_layout_with_inject_state(
        provider,
        session_id,
        system_prompt,
        SessionContextInjectState {
            current: session_context,
            previous: None,
            turns_since_force_fresh: 0,
        },
        messages,
    )
    .0
}

/// Build request layout and return inject outcome for session persistence.
pub fn build_request_layout_with_inject_state(
    provider: &str,
    session_id: &str,
    system_prompt: Option<String>,
    inject_state: SessionContextInjectState,
    mut messages: Vec<Message>,
) -> (RequestLayout, SessionContextInjectOutcome) {
    let outcome = resolve_session_context_inject(&messages, &inject_state);
    let Some(session_context) = outcome.inject_text.clone() else {
        return (
            RequestLayout {
                system_prompt,
                messages,
            },
            outcome,
        );
    };

    if provider_uses_synthetic_session_context(provider) {
        let insert_idx = synthetic_session_context_insert_index(&messages);

        let reference_message_id = if insert_idx < messages.len() {
            messages[insert_idx].id.as_str()
        } else if insert_idx > 0 {
            messages[insert_idx - 1].id.as_str()
        } else {
            "system"
        };

        let session_context_msg = build_synthetic_session_context_message(
            provider,
            session_id,
            &session_context,
            reference_message_id,
        );

        messages.insert(insert_idx, session_context_msg);

        (
            RequestLayout {
                system_prompt,
                messages,
            },
            outcome,
        )
    } else {
        let framed_context = format_session_context_text(provider, &session_context);
        let mut merged = false;
        for message in messages.iter_mut().rev() {
            if message.role == "user" && !message.is_internal_synthetic_user_message() {
                let mut new_content = vec![MCPContent::Text {
                    text: framed_context.clone(),
                }];
                new_content.append(&mut message.content);
                message.content = new_content;
                merged = true;
                break;
            }
        }

        let system_prompt = if !merged {
            Some(
                [system_prompt, Some(framed_context)]
                    .into_iter()
                    .flatten()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
            .filter(|prompt| !prompt.is_empty())
        } else {
            system_prompt
        };

        (
            RequestLayout {
                system_prompt,
                messages,
            },
            outcome,
        )
    }
}

pub fn select_last_submitted_input_message_id(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| !message.is_internal_synthetic_user_message())
        .or_else(|| messages.last())
        .map(|message| message.id.clone())
}
