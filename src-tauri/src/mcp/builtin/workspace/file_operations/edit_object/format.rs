//! Parse/serialize helpers for object-document editing.
//!
//! JSON ops mutate `serde_json::Value`; TOML ops mutate a `toml_edit::DocumentMut` so
//! comments and the formatting of unedited keys survive. Keep these as plain functions
//! (no format trait).

use serde_json::Value;
use toml_edit::DocumentMut;

pub const MAX_OPS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    Json,
    Toml,
}

/// The document format for `path`, from its extension (case-insensitive).
pub fn detect_format(path: &str) -> Result<ObjectFormat, String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".json") {
        Ok(ObjectFormat::Json)
    } else if lower.ends_with(".toml") {
        Ok(ObjectFormat::Toml)
    } else {
        Err(format!(
            "editObject supports .json and .toml only (got '{path}')."
        ))
    }
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

pub fn parse_toml(text: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>()
        .map_err(|error| format!("Invalid TOML: {}", error.to_string().trim_end()))
}

/// Render the edited document as-is. Re-parses to guarantee validity.
pub fn serialize_toml(document: &DocumentMut) -> Result<String, String> {
    let text = document.to_string();
    parse_toml(&text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{detect_format, parse_json, parse_toml, serialize_json, ObjectFormat};
    use serde_json::json;

    #[test]
    fn detects_format_from_extension_case_insensitively() {
        assert_eq!(detect_format("package.json"), Ok(ObjectFormat::Json));
        assert_eq!(detect_format("Config.JSON"), Ok(ObjectFormat::Json));
        assert_eq!(detect_format("Cargo.toml"), Ok(ObjectFormat::Toml));
        assert_eq!(detect_format("pyproject.TOML"), Ok(ObjectFormat::Toml));
        for path in ["a.yaml", "a.yml", "main.rs"] {
            let error = detect_format(path).unwrap_err();
            assert!(error.contains(".json and .toml only"), "{error}");
        }
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

    #[test]
    fn parse_rejects_invalid_toml() {
        let err = parse_toml("a = \n").unwrap_err();
        assert!(err.starts_with("Invalid TOML"), "{err}");
    }
}
