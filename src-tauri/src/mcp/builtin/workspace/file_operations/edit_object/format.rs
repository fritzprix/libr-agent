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

/// Render the edited document with the line endings of `original` (`toml_edit`
/// writes LF), adding a UTF-8 BOM when `utf8_bom`. Re-parses to guarantee validity.
pub fn serialize_toml(
    document: &DocumentMut,
    original: &str,
    utf8_bom: bool,
) -> Result<String, String> {
    let mut text = document.to_string();
    if uses_crlf(original) {
        text = text.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    parse_toml(&text)?;
    if utf8_bom {
        text.insert(0, '\u{feff}');
    }
    Ok(text)
}

/// True when every line break in `text` is CRLF. Mixed files are written as LF.
fn uses_crlf(text: &str) -> bool {
    let breaks = text.matches('\n').count();
    breaks > 0 && text.matches("\r\n").count() == breaks
}

#[cfg(test)]
mod tests {
    use super::{
        detect_format, parse_json, parse_toml, serialize_json, serialize_toml, ObjectFormat,
    };
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
    fn serialize_toml_keeps_crlf_and_bom() {
        let original = "# top\r\na = 1\r\n\r\n[t]\r\nx = 1\r\n";
        let mut document = parse_toml(original).unwrap();
        document["a"] = toml_edit::value(2);
        let text = serialize_toml(&document, original, true).unwrap();
        assert_eq!(text, "\u{feff}# top\r\na = 2\r\n\r\n[t]\r\nx = 1\r\n");
    }

    #[test]
    fn serialize_toml_writes_lf_for_lf_and_mixed_files() {
        for original in ["a = 1\nb = 2\n", "a = 1\r\nb = 2\n"] {
            let document = parse_toml(original).unwrap();
            let text = serialize_toml(&document, original, false).unwrap();
            assert!(!text.contains('\r'), "{text:?}");
            assert!(!text.starts_with('\u{feff}'), "{text:?}");
        }
    }

    #[test]
    fn parse_rejects_invalid_toml() {
        let err = parse_toml("a = \n").unwrap_err();
        assert!(err.starts_with("Invalid TOML"), "{err}");
    }
}
