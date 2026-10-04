//! JSON parse/serialize helpers for object-document editing.
//!
//! Ops mutate `serde_json::Value`. YAML/TOML can be added later as sibling
//! helpers + extension checks in [`ensure_json_path`] — keep parse/serialize
//! as plain functions (no format trait).

use serde_json::Value;

pub const MAX_OPS: usize = 32;

/// Reject non-`.json` paths early (MVP).
pub fn ensure_json_path(path: &str) -> Result<(), String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".json") {
        return Ok(());
    }
    Err(format!(
        "editObject currently supports .json files only (got '{path}'). Use workspace__strReplace or workspace__writeFile for other formats."
    ))
}

pub fn parse_json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|error| {
        format!(
            "Invalid JSON: {error}. Comments/trailing commas (JSONC) are not supported — fix the file with workspace__readFile + workspace__writeFile/strReplace first."
        )
    })
}

/// Pretty-print with 2-space indent and trailing newline. Re-parses to guarantee validity.
pub fn serialize_json(value: &Value) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("Failed to serialize JSON: {error}"))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    parse_json(&text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{ensure_json_path, parse_json, serialize_json};
    use serde_json::json;

    #[test]
    fn detects_json_extension_case_insensitively() {
        assert!(ensure_json_path("package.json").is_ok());
        assert!(ensure_json_path("Config.JSON").is_ok());
        assert!(ensure_json_path("a.yaml").is_err());
    }

    #[test]
    fn serialize_pretty_with_trailing_newline() {
        let text = serialize_json(&json!({"b": 1, "a": 2})).unwrap();
        assert!(text.ends_with('\n'));
        assert!(text.contains("\n  "));
        assert!(text.find("\"b\"").unwrap() < text.find("\"a\"").unwrap());
    }

    #[test]
    fn parse_rejects_jsonc_comments() {
        let err = parse_json("{\n  // comment\n  \"a\": 1\n}").unwrap_err();
        assert!(err.contains("Invalid JSON"), "{err}");
    }
}
