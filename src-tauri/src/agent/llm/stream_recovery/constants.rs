pub const REPEATED_THINKING_MAX_RETRIES: u32 = 2;
pub const REPEATED_TEXT_LOOP_MAX_RETRIES: u32 = 2;
pub const BAD_TOOL_ARGS_MAX_RETRIES: u32 = 2;
/// Final turn that is only text-formatted tool-call markup (`<function=NAME>…`).
/// Does not use `experimentalSettings.toolLoopMaxResampleRetries`.
pub const MALFORMED_TOOL_MARKUP_MAX_RETRIES: u32 = 2;
/// Corrective note appended before retrying; neutral on purpose (no tool names).
pub const MALFORMED_TOOL_MARKUP_NOTE: &str = "Your previous reply contained text formatted like a tool call, but no structured tool call was made, so nothing was executed and that reply was discarded. If you meant to call a tool, send it as a structured tool call. Otherwise, continue your answer.";

/// Client-side reasoning-budget abort: one clean resample, no prompt nudge.
pub const REASONING_BUDGET_MAX_RETRIES: u32 = 1;
/// Max FallThrough incidents per workflow before hard-stopping. Bounds
/// truncated-args loops where raw args (and thus loop-prevention signatures)
/// change every attempt.
pub const BAD_TOOL_ARGS_MAX_INCIDENTS: u32 = 3;
