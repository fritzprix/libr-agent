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

/// One tool call whose arguments are not a valid JSON object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidToolArgsIssue {
    pub tool_call_id: String,
    pub tool_name: String,
    pub kind: ArgsParseFailureKind,
    pub parse_error: String,
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

    match serde_json::from_str::<serde_json::Value>(raw_arguments) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(_) => Err((
            ArgsParseFailureKind::MalformedJson,
            "arguments must decode to a JSON object".to_string(),
        )),
        Err(error) => Err((classify_args_parse_failure(&error), error.to_string())),
    }
}

/// Collect every tool call in the batch whose arguments are invalid.
pub fn find_invalid_tool_call_args(tool_calls: &[ToolCall]) -> Vec<InvalidToolArgsIssue> {
    let mut issues = Vec::new();
    for tool_call in tool_calls {
        if let Err((kind, parse_error)) = inspect_tool_call_arguments(&tool_call.function.arguments)
        {
            issues.push(InvalidToolArgsIssue {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.function.name.clone(),
                kind,
                parse_error,
            });
        }
    }
    issues
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
        assert!(find_invalid_tool_call_args(&[tool_call("1", "ping", "")]).is_empty());
    }

    #[test]
    fn valid_object_args_pass() {
        assert!(inspect_tool_call_arguments(r#"{"path":"a.txt"}"#).is_ok());
        assert!(
            find_invalid_tool_call_args(&[tool_call("1", "readFile", r#"{"path":"a.txt"}"#)])
                .is_empty()
        );
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
        let err = inspect_tool_call_arguments("[1,2]").unwrap_err();
        assert_eq!(err.0, ArgsParseFailureKind::MalformedJson);
    }

    #[test]
    fn find_invalid_collects_only_bad_calls() {
        let issues = find_invalid_tool_call_args(&[
            tool_call("good", "listFiles", r#"{"path":"src"}"#),
            tool_call("bad", "readFile", r#"{"path":"foo"#),
        ]);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].tool_call_id, "bad");
        assert!(issues[0].kind.is_truncated());
    }
}
