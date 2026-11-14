//! LLM client with no Tauri or WebView dependency (#2754).
//!
//! The GUI, `libragent --tui`, and `liblet` share this one client. A provider
//! implements [`ChatProvider`]; the trait's default methods are the
//! OpenAI-compatible behaviour (request body, streaming parse, tool-call
//! mapping, error classification), so an OpenAI-compatible provider overrides
//! only the hooks that differ. [`complete`] runs one streaming call with
//! retries.
//!
//! Covered: OpenAI, Fireworks, OpenRouter, and custom OpenAI-compatible
//! providers, with the GUI's generation options (temperature, max output
//! tokens, thinking effort, retries) and OpenAI's automatic prompt-cache key.
//! Anthropic and Gemini (#2755) override the message and tool format, the
//! auth header, and the stream parser, and reuse the rest.

mod anthropic;
mod client;
mod error;
mod gemini;
mod openai;
mod provider;
mod stream;
mod types;

pub use anthropic::{Anthropic, AnthropicStream};
pub use client::{complete, RetryPolicy};
pub use error::{ErrorKind, LlmError};
pub use gemini::{Gemini, GeminiStream};
pub use provider::{
    is_openai_compatible, is_supported, provider_with_endpoint, resolve_provider, ChatProvider,
    CustomOpenAi, Endpoint, Fireworks, OpenAi, OpenRouter,
};
pub use stream::{OpenAiStream, StreamParser};
pub use types::{
    ChatMessage, ChatRequest, CompletedChat, ContentPart, Role, ThinkingEffort, ToolCall,
    ToolCallFunction, ToolSpec, Usage, UsageDetails,
};
