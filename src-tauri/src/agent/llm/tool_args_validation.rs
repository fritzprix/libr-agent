//! Shared validation / classification for tool-call `function.arguments` JSON.
//!
//! Used both for early bad-response retry (before cache/execute) and for the
//! guided tool-result fallback when parse still fails at execution time.

use crate::agent::types::ToolCall;

/// Why `function.arguments` failed to parse as a usable tool-args object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgsParseFailureKind {
    /// JSON cut off inside a `"..."` string (typical oversized field + provider truncate).
    TruncatedString,
    /// JSON cut off elsewhere (object/array/number).
    TruncatedJson,
    /// Complete but invalid JSON, or JSON that is not an object.
    MalformedJson,
}

impl ArgsParseFailureKind {
    pub fn as_error_kind(self) -> &'static str {
        match self {
            Self::TruncatedString => "truncated_tool_args_string",
            Self::TruncatedJson => "truncated_tool_args",
            Self::MalformedJson => "malformed_tool_args",
        }
    }

    pub fn is_truncated(self) -> bool {
        matches!(self, Self::TruncatedString | Self::TruncatedJson)
    }
}

/// Classify a `serde_json` parse error into truncated vs malformed.
pub fn classify_args_parse_failure(error: &serde_json::Error) -> ArgsParseFailureKind {
    use serde_json::error::Category;
    let message = error.to_string();
    match error.classify() {
        Category::Eof if message.contains("string") => ArgsParseFailureKind::TruncatedString,
        Category::Eof => ArgsParseFailureKind::TruncatedJson,
        _ => ArgsParseFailureKind::MalformedJson,
    }
}

/// Redact truncated arguments in a slice of tool calls in-place.
///
/// Malformed (non-truncated) arguments are left intact because they are not oversized
/// and retaining the raw payload allows the model to see its syntax error.
pub fn redact_invalid_tool_call_args(tool_calls: &mut [ToolCall]) -> usize {
    let mut count = 0;
    for tool_call in tool_calls.iter_mut() {
        if let Err((kind, _)) = inspect_tool_call_arguments(&tool_call.function.arguments) {
            if kind.is_truncated() && !is_redacted_marker(&tool_call.function.arguments) {
                let raw_len = tool_call.function.arguments.len();
                tool_call.function.arguments = redact_tool_call_arguments(raw_len);
                count += 1;
            }
        }
    }
    count
}

pub fn redact_tool_call_arguments(raw_len: usize) -> String {
    format!("<redacted: {} bytes, truncated>", raw_len)
}

pub fn is_redacted_marker(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with("<redacted:") && trimmed.ends_with(">")
}

pub fn parse_redacted_byte_length(raw: &str) -> Option<usize> {
    let trimmed = raw.trim();
    if trimmed.starts_with("<redacted:") && trimmed.ends_with(">") {
        let inner = &trimmed[10..trimmed.len() - 1];
        if let Some(bytes_pos) = inner.find("bytes") {
            let num_str = inner[..bytes_pos].trim();
            return num_str.parse::<usize>().ok();
        }
    }
    None
}

/// Inspect a single arguments blob. Accepts only a JSON object (matches
/// `inspectToolCallArguments` in `message-normalizer.ts`).
///
/// Empty / whitespace-only arguments are treated as `{}` — providers often
/// emit `""` for zero-parameter tool calls, which is not truncation.
pub fn inspect_tool_call_arguments(
    raw_arguments: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, (ArgsParseFailureKind, String)> {
    if raw_arguments.trim().is_empty() {
        return Ok(serde_json::Map::new());
    }

    if is_redacted_marker(raw_arguments) {
        return Err((
            ArgsParseFailureKind::TruncatedJson,
            "tool-call arguments were truncated (output budget exceeded)".to_string(),
        ));
    }

    match serde_json::from_str::<serde_json::Value>(raw_arguments) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(_) => Err((
            ArgsParseFailureKind::MalformedJson,
            "arguments must decode to a JSON object".to_string(),
        )),
        Err(error) => Err((classify_args_parse_failure(&error), error.to_string())),
    }
}

/// Short preview of a raw args string for error payloads / logs.
pub fn args_preview(args_str: &str, max_chars: usize) -> String {
    let mut preview: String = args_str.chars().take(max_chars).collect();
    if args_str.chars().count() > max_chars {
        preview.push('…');
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::ToolCallFunction;

    fn tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }

    #[test]
    fn empty_arguments_are_empty_object() {
        assert!(inspect_tool_call_arguments("").is_ok());
        assert!(inspect_tool_call_arguments("   ").is_ok());
    }

    #[test]
    fn valid_object_args_pass() {
        assert!(inspect_tool_call_arguments(r#"{"path":"a.txt"}"#).is_ok());
    }

    #[test]
    fn truncated_string_is_classified() {
        let err = serde_json::from_str::<serde_json::Value>(r#"{"thought":"hello"#).unwrap_err();
        assert_eq!(
            classify_args_parse_failure(&err),
            ArgsParseFailureKind::TruncatedString
        );
    }

    #[test]
    fn truncated_object_is_classified() {
        let err = serde_json::from_str::<serde_json::Value>(r#"{"a":1"#).unwrap_err();
        assert_eq!(
            classify_args_parse_failure(&err),
            ArgsParseFailureKind::TruncatedJson
        );
    }

    #[test]
    fn malformed_complete_json_is_classified() {
        let err = serde_json::from_str::<serde_json::Value>(r#"{path:"broken"}"#).unwrap_err();
        assert_eq!(
            classify_args_parse_failure(&err),
            ArgsParseFailureKind::MalformedJson
        );
    }

    #[test]
    fn non_object_json_is_malformed() {
        let err = inspect_tool_call_arguments(r#""just a string""#).unwrap_err();
        assert_eq!(err.0, ArgsParseFailureKind::MalformedJson);
        assert!(!err.0.is_truncated());
        let err = inspect_tool_call_arguments("[1,2]").unwrap_err();
        assert_eq!(err.0, ArgsParseFailureKind::MalformedJson);
        assert!(!err.0.is_truncated());
    }

    #[test]
    fn redact_only_redacts_truncated_args_leaving_malformed_intact() {
        let mut calls = vec![
            tool_call("good", "listFiles", r#"{"path":"src"}"#),
            tool_call("trunc", "readFile", r#"{"path":"foo"#),
            tool_call("malformed", "query", r#"{"a": 1,}"#),
            tool_call("array", "batch", "[1, 2]"),
        ];

        let count = redact_invalid_tool_call_args(&mut calls);
        assert_eq!(count, 1, "only truncated call should be redacted");

        // Truncated call is redacted
        assert_eq!(
            calls[1].function.arguments,
            "<redacted: 12 bytes, truncated>"
        );
        assert!(is_redacted_marker(&calls[1].function.arguments));

        // Good and malformed calls retain raw arguments
        assert_eq!(calls[0].function.arguments, r#"{"path":"src"}"#);
        assert_eq!(calls[2].function.arguments, r#"{"a": 1,}"#);
        assert_eq!(calls[3].function.arguments, "[1, 2]");

        // Inspecting malformed calls still yields MalformedJson with is_truncated == false
        let malformed_err = inspect_tool_call_arguments(&calls[2].function.arguments).unwrap_err();
        assert_eq!(malformed_err.0, ArgsParseFailureKind::MalformedJson);
        assert!(!malformed_err.0.is_truncated());

        let array_err = inspect_tool_call_arguments(&calls[3].function.arguments).unwrap_err();
        assert_eq!(array_err.0, ArgsParseFailureKind::MalformedJson);
        assert!(!array_err.0.is_truncated());
    }

    #[test]
    fn redact_and_parse_redacted_marker() {
        let marker = redact_tool_call_arguments(4096);
        assert_eq!(marker, "<redacted: 4096 bytes, truncated>");
        assert!(is_redacted_marker(&marker));
        assert_eq!(parse_redacted_byte_length(&marker), Some(4096));

        let invalid = inspect_tool_call_arguments(&marker).unwrap_err();
        assert_eq!(invalid.0, ArgsParseFailureKind::TruncatedJson);
        assert!(invalid.0.is_truncated());
        assert!(invalid.1.contains("output budget exceeded"));
    }
}
