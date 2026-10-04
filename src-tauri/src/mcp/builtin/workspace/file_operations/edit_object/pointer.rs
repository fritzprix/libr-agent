//! JSON Pointer helpers (RFC 6901) with one intentional ergonomic exception:
//! path `"/"` denotes the whole document (root), not the empty-string key.

/// Parsed pointer tokens after unescaping `~1` → `/` and `~0` → `~`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonPointer {
    tokens: Vec<String>,
}

impl JsonPointer {
    /// Parse a pointer that must start with `/`.
    ///
    /// - `"/"` → root (zero tokens)
    /// - `"/foo/bar"` → `["foo", "bar"]`
    /// - `"/a~1b"` → `["a/b"]`
    pub fn parse(pointer: &str) -> Result<Self, String> {
        if pointer.is_empty() {
            return Err(
                "JSON Pointer must start with '/'. Use '/' for the whole document.".to_string(),
            );
        }
        if !pointer.starts_with('/') {
            return Err(format!(
                "JSON Pointer must start with '/': got '{pointer}'. Example: /scripts/dev"
            ));
        }
        if pointer == "/" {
            return Ok(Self { tokens: Vec::new() });
        }

        let mut tokens = Vec::new();
        for raw in pointer[1..].split('/') {
            tokens.push(unescape_token(raw)?);
        }
        Ok(Self { tokens })
    }

    pub fn is_root(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    pub fn display(&self) -> String {
        if self.is_root() {
            return "/".to_string();
        }
        let mut out = String::new();
        for token in &self.tokens {
            out.push('/');
            out.push_str(&escape_token(token));
        }
        out
    }
}

fn unescape_token(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '~' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('0') => out.push('~'),
            Some('1') => out.push('/'),
            Some(other) => {
                return Err(format!(
                    "Invalid JSON Pointer escape '~{other}' (only ~0 and ~1 are allowed)"
                ));
            }
            None => {
                return Err(
                    "Invalid JSON Pointer escape: trailing '~' (expected ~0 or ~1)".to_string(),
                );
            }
        }
    }
    Ok(out)
}

fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Interpret a pointer token as an array index (decimal, no leading plus).
pub fn parse_array_index(token: &str) -> Result<usize, String> {
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "Array index token must be a non-negative decimal integer, got '{token}'"
        ));
    }
    if token.len() > 1 && token.starts_with('0') {
        return Err(format!(
            "Array index token must not have leading zeros, got '{token}'"
        ));
    }
    token
        .parse::<usize>()
        .map_err(|_| format!("Array index token out of range: '{token}'"))
}

#[cfg(test)]
mod tests {
    use super::{parse_array_index, JsonPointer};

    #[test]
    fn parses_root_and_nested() {
        assert!(JsonPointer::parse("/").unwrap().is_root());
        assert_eq!(
            JsonPointer::parse("/scripts/dev").unwrap().tokens(),
            ["scripts", "dev"]
        );
        assert_eq!(JsonPointer::parse("/a~1b").unwrap().tokens(), ["a/b"]);
        assert_eq!(JsonPointer::parse("/a~0b").unwrap().tokens(), ["a~b"]);
    }

    #[test]
    fn rejects_invalid_pointers() {
        assert!(JsonPointer::parse("").is_err());
        assert!(JsonPointer::parse("scripts").is_err());
        assert!(JsonPointer::parse("/a~2b").is_err());
    }

    #[test]
    fn parses_array_index_strictly() {
        assert_eq!(parse_array_index("0").unwrap(), 0);
        assert_eq!(parse_array_index("12").unwrap(), 12);
        assert!(parse_array_index("01").is_err());
        assert!(parse_array_index("-1").is_err());
        assert!(parse_array_index("1e2").is_err());
    }
}
