//! MCP server name sanitization for LLM tool identifiers.
//!
//! Tool names are exposed as `{server_name}__{tool_name}`. Provider schemas reject
//! whitespace and most punctuation:
//! - OpenAI / Anthropic: `^[a-zA-Z0-9_-]{1,64}$`
//! - Gemini: `^[a-zA-Z_][a-zA-Z0-9_]*$`
//!
//! We normalize to Gemini-safe identifiers (underscore only, letter/`_` start) so
//! one runtime key works across providers. Existing DB rows with spaces/hyphens
//! remain valid via session-load sanitization.

use std::collections::HashSet;

/// Regex-equivalent: `^[A-Za-z_][A-Za-z0-9_]*$` with no `__` substring.
pub fn is_valid_mcp_server_name(name: &str) -> bool {
    if name.is_empty() || name.contains("__") {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Sanitize a raw MCP server name into a session / tool-prefix identifier.
///
/// - Replaces non `[A-Za-z0-9_]` characters with `_`
/// - Collapses consecutive `_`
/// - Strips trailing `_`; keeps at most one leading `_` when present
/// - Prefixes digit-leading names with `s_` (Gemini first-char rule)
/// - Falls back to `mcp_server` when the result would otherwise be empty
pub fn sanitize_mcp_server_name(name: &str) -> String {
    let mapped: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let mut collapsed = String::with_capacity(mapped.len());
    let mut last_was_underscore = false;
    for c in mapped.chars() {
        if c == '_' {
            if last_was_underscore {
                continue;
            }
            collapsed.push('_');
            last_was_underscore = true;
        } else {
            collapsed.push(c);
            last_was_underscore = false;
        }
    }

    let had_leading_underscore = collapsed.starts_with('_');
    let core = collapsed.trim_matches('_');
    if core.is_empty() {
        return "mcp_server".to_string();
    }

    let out = if had_leading_underscore {
        format!("_{core}")
    } else {
        core.to_string()
    };

    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("s_{out}")
    } else {
        out
    }
}

/// Allocate a unique sanitized session key for `raw_name`.
///
/// On collision with an already-taken key, appends a short alphanumeric prefix of
/// `server_id` (and a numeric counter if still colliding).
pub fn unique_session_server_name(
    raw_name: &str,
    server_id: &str,
    taken: &mut HashSet<String>,
) -> String {
    let base = sanitize_mcp_server_name(raw_name);
    if taken.insert(base.clone()) {
        return base;
    }

    let short_id: String = server_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();

    let id_part = if short_id.is_empty() {
        "id".to_string()
    } else {
        short_id
    };

    let mut candidate = format!("{base}_{id_part}");
    let mut n = 2u32;
    while !taken.insert(candidate.clone()) {
        candidate = format!("{base}_{id_part}_{n}");
        n = n.saturating_add(1);
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_spaces_and_collapses_runs() {
        assert_eq!(sanitize_mcp_server_name("My Server"), "My_Server");
        assert_eq!(sanitize_mcp_server_name("Google Drive"), "Google_Drive");
        assert_eq!(sanitize_mcp_server_name("My  Server"), "My_Server");
        assert_eq!(sanitize_mcp_server_name("  spaced  "), "spaced");
    }

    #[test]
    fn sanitize_replaces_hyphens_and_punctuation() {
        assert_eq!(sanitize_mcp_server_name("yahoo-finance"), "yahoo_finance");
        assert_eq!(sanitize_mcp_server_name("bad-server"), "bad_server");
        assert_eq!(sanitize_mcp_server_name("a.b@c"), "a_b_c");
    }

    #[test]
    fn sanitize_prefixes_digit_leading_names() {
        assert_eq!(sanitize_mcp_server_name("2FA Server"), "s_2FA_Server");
        assert_eq!(sanitize_mcp_server_name("123"), "s_123");
    }

    #[test]
    fn sanitize_empty_or_symbols_falls_back() {
        assert_eq!(sanitize_mcp_server_name(""), "mcp_server");
        assert_eq!(sanitize_mcp_server_name("   "), "mcp_server");
        assert_eq!(sanitize_mcp_server_name("@@@"), "mcp_server");
        assert_eq!(sanitize_mcp_server_name("___"), "mcp_server");
    }

    #[test]
    fn sanitize_preserves_already_valid_names() {
        assert_eq!(sanitize_mcp_server_name("filesystem"), "filesystem");
        assert_eq!(sanitize_mcp_server_name("My_Server"), "My_Server");
        assert_eq!(sanitize_mcp_server_name("_private"), "_private");
    }

    #[test]
    fn sanitize_never_emits_double_underscore() {
        let samples = [
            "My  Server",
            "a--b",
            "a__b",
            " a - b ",
            "x___y",
            "__leading",
            "trailing__",
        ];
        for sample in samples {
            let sanitized = sanitize_mcp_server_name(sample);
            assert!(
                !sanitized.contains("__"),
                "sanitize({sample:?}) produced {sanitized:?}"
            );
            assert!(is_valid_mcp_server_name(&sanitized));
        }
    }

    #[test]
    fn is_valid_rejects_spaces_hyphens_and_delimiter() {
        assert!(is_valid_mcp_server_name("filesystem"));
        assert!(is_valid_mcp_server_name("My_Server"));
        assert!(is_valid_mcp_server_name("_x"));
        assert!(!is_valid_mcp_server_name(""));
        assert!(!is_valid_mcp_server_name("My Server"));
        assert!(!is_valid_mcp_server_name("yahoo-finance"));
        assert!(!is_valid_mcp_server_name("a__b"));
        assert!(!is_valid_mcp_server_name("2bad"));
    }

    #[test]
    fn unique_session_name_disambiguates_collisions() {
        let mut taken = HashSet::new();
        let first = unique_session_server_name("My Server", "aaaaaaaa-1111", &mut taken);
        let second = unique_session_server_name("My_Server", "bbbbbbbb-2222", &mut taken);
        assert_eq!(first, "My_Server");
        assert_eq!(second, "My_Server_bbbbbbbb");
        assert_ne!(first, second);
    }
}
