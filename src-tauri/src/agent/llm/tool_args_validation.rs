//! Shared validation / classification for tool-call `function.arguments` JSON.
//!
//! Used both for early bad-response retry (before cache/execute) and for the
//! guided tool-result fallback when parse still fails at execution time.

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
