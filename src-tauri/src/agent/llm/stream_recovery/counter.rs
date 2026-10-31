use super::constants::{
    BAD_TOOL_ARGS_MAX_RETRIES, MALFORMED_TOOL_MARKUP_MAX_RETRIES, REASONING_BUDGET_MAX_RETRIES,
    REPEATED_TEXT_LOOP_MAX_RETRIES, REPEATED_THINKING_MAX_RETRIES,
};
use crate::agent::state::AgentSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StreamingRecoveryCounter {
    Thinking,
    Text,
    BadToolArgs,
    ReasoningBudget,
    MalformedToolMarkup,
}

impl StreamingRecoveryCounter {
    pub(super) fn max_retries(self) -> u32 {
        match self {
            Self::Thinking => REPEATED_THINKING_MAX_RETRIES,
            Self::Text => REPEATED_TEXT_LOOP_MAX_RETRIES,
            Self::BadToolArgs => BAD_TOOL_ARGS_MAX_RETRIES,
            Self::ReasoningBudget => REASONING_BUDGET_MAX_RETRIES,
            Self::MalformedToolMarkup => MALFORMED_TOOL_MARKUP_MAX_RETRIES,
        }
    }

    pub(super) async fn read_count(self, session: &AgentSession) -> u32 {
        match self {
            Self::Thinking => *session.repeated_thinking_retry_count.read().await,
            Self::Text => *session.repeated_text_loop_retry_count.read().await,
            Self::BadToolArgs => *session.bad_tool_args_retry_count.read().await,
            Self::ReasoningBudget => *session.reasoning_budget_retry_count.read().await,
            Self::MalformedToolMarkup => *session.malformed_tool_markup_retry_count.read().await,
        }
    }

    pub(super) async fn write_count(self, session: &AgentSession, value: u32) {
        match self {
            Self::Thinking => {
                *session.repeated_thinking_retry_count.write().await = value;
            }
            Self::Text => {
                *session.repeated_text_loop_retry_count.write().await = value;
            }
            Self::BadToolArgs => {
                *session.bad_tool_args_retry_count.write().await = value;
            }
            Self::ReasoningBudget => {
                *session.reasoning_budget_retry_count.write().await = value;
            }
            Self::MalformedToolMarkup => {
                *session.malformed_tool_markup_retry_count.write().await = value;
            }
        }
    }
}
