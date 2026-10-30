use tracing::info;

/// Normalize shell command for proper execution
/// Handles platform-specific quoting and escaping rules
pub fn normalize_shell_command(raw_command: &str) -> String {
    #[cfg(windows)]
    {
        // Windows: PowerShell handles both single and double quotes correctly
        // No normalization needed - pass command as-is to avoid breaking nested quotes
        // in Python/Node.js inline commands like: python -c "print('Hello')"
        info!("Windows command (no normalization): {}", raw_command);
        raw_command.to_string()
    }

    #[cfg(not(windows))]
    {
        // Unix shell quoting normalization
        let mut normalized = raw_command.to_string();

        // 1. Close a quote left open at the end of the command
        if let Some(quote) = find_unclosed_quote(&normalized) {
            normalized.push(quote);
            info!("Shell command: Added missing {} quote", quote);
        }

        // 2. Inject -S flag for sudo commands to read from stdin in non-interactive/non-PTY shells
        normalized = inject_sudo_stdin_flag(&normalized);

        normalized
    }
}

/// Returns the quote character left open at the end of a POSIX shell command, if any.
///
/// Apostrophes and quotes in comments, heredoc bodies and escapes are literal text,
/// not quote delimiters, so they are skipped. `$'...'` (ANSI-C quoting) honors
/// backslash escapes. An unterminated heredoc is not a quote problem: returns `None`.
#[cfg_attr(windows, allow(dead_code))]
fn find_unclosed_quote(command: &str) -> Option<char> {
    let chars: Vec<char> = command.chars().collect();
    let len = chars.len();
    // (delimiter, strip leading tabs) for heredocs whose body starts at the next newline
    let mut pending_heredocs: Vec<(String, bool)> = Vec::new();
    let mut prev_unescaped_dollar = false;
    let mut i = 0;

    while i < len {
        let c = chars[i];
        let after_dollar = prev_unescaped_dollar;
        prev_unescaped_dollar = false;

        match c {
            '\\' => i += 2,
            '\'' => {
                i += 1;
                loop {
                    if i >= len {
                        return Some('\'');
                    }
                    match chars[i] {
                        '\\' if after_dollar => i += 2,
                        '\'' => break,
                        _ => i += 1,
                    }
                }
                i += 1;
            }
            '"' => {
                i += 1;
                loop {
                    if i >= len {
                        return Some('"');
                    }
                    match chars[i] {
                        '\\' => i += 2,
                        '"' => break,
                        _ => i += 1,
                    }
                }
                i += 1;
            }
            '#' if i == 0 || is_word_break(chars[i - 1]) => {
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
            }
            '<' if chars.get(i + 1) == Some(&'<') && chars.get(i + 2) != Some(&'<') => {
                i += 2;
                if let Some(heredoc) = parse_heredoc_delimiter(&chars, &mut i) {
                    pending_heredocs.push(heredoc);
                }
            }
            '\n' => {
                i += 1;
                for (delimiter, strip_tabs) in pending_heredocs.drain(..) {
                    loop {
                        if i >= len {
                            return None;
                        }
                        let line_end = chars[i..]
                            .iter()
                            .position(|&ch| ch == '\n')
                            .map_or(len, |p| i + p);
                        let mut line = &chars[i..line_end];
                        if strip_tabs {
                            while line.first() == Some(&'\t') {
                                line = &line[1..];
                            }
                        }
                        i = line_end + 1;
                        if line.iter().copied().eq(delimiter.chars()) {
                            break;
                        }
                    }
                }
            }
            _ => {
                prev_unescaped_dollar = c == '$';
                i += 1;
            }
        }
    }

    None
}

/// Characters after which a `#` starts a comment.
#[cfg_attr(windows, allow(dead_code))]
fn is_word_break(c: char) -> bool {
    c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')')
}

/// Parses the delimiter word after `<<` (cursor just past it), advancing the cursor.
/// Returns `None` when the text is not a heredoc (e.g. arithmetic `$((1<<2))`).
#[cfg_attr(windows, allow(dead_code))]
fn parse_heredoc_delimiter(chars: &[char], i: &mut usize) -> Option<(String, bool)> {
    let strip_tabs = chars.get(*i) == Some(&'-');
    if strip_tabs {
        *i += 1;
    }
    while matches!(chars.get(*i), Some(' ' | '\t')) {
        *i += 1;
    }

    let first = *chars.get(*i)?;
    if !(first.is_alphabetic() || matches!(first, '_' | '\'' | '"' | '\\')) {
        return None;
    }

    let mut delimiter = String::new();
    while let Some(&c) = chars.get(*i) {
        if c.is_whitespace() || matches!(c, ';' | '&' | '|' | '<' | '>' | '(' | ')') {
            break;
        }
        *i += 1;
        match c {
            '\'' | '"' => {
                while let Some(&inner) = chars.get(*i) {
                    *i += 1;
                    if inner == c {
                        break;
                    }
                    delimiter.push(inner);
                }
            }
            '\\' => {
                if let Some(&escaped) = chars.get(*i) {
                    delimiter.push(escaped);
                    *i += 1;
                }
            }
            _ => delimiter.push(c),
        }
    }

    (!delimiter.is_empty()).then_some((delimiter, strip_tabs))
}

/// Inject -S flag for sudo commands to read from stdin in non-interactive/non-PTY shells
#[cfg(not(windows))]
pub fn inject_sudo_stdin_flag(command: &str) -> String {
    let mut result = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;
    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if in_single_quote {
            if chars[i] == '\'' {
                in_single_quote = false;
            }
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if in_double_quote {
            if escaped {
                escaped = false;
            } else if chars[i] == '\\' {
                escaped = true;
            } else if chars[i] == '"' {
                in_double_quote = false;
            }
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if escaped {
            escaped = false;
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if chars[i] == '\\' {
            escaped = true;
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if chars[i] == '\'' {
            in_single_quote = true;
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if chars[i] == '"' {
            in_double_quote = true;
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // Check for 'sudo' word outside of quotes
        if chars[i..].starts_with(&['s', 'u', 'd', 'o']) {
            // Check word boundaries for 'sudo'
            let is_start_boundary = i == 0 || {
                let prev = chars[i - 1];
                prev.is_whitespace()
                    || prev == ';'
                    || prev == '&'
                    || prev == '|'
                    || prev == '('
                    || prev == ')'
                    || prev == '{'
                    || prev == '}'
                    || prev == '`'
            };

            let has_space_after = i + 4 < chars.len() && chars[i + 4].is_whitespace();
            let is_end_boundary = i + 4 == chars.len() || has_space_after;

            if is_start_boundary && is_end_boundary {
                result.push_str("sudo");
                i += 4;

                // Check if it's already followed by -S or --stdin
                let mut next_idx = i;
                while next_idx < chars.len() && chars[next_idx].is_whitespace() {
                    next_idx += 1;
                }

                let mut already_has_stdin_flag = false;
                if next_idx < chars.len() {
                    let rest = &chars[next_idx..];
                    if rest.starts_with(&['-', 'S']) {
                        let after_flag = next_idx + 2;
                        if after_flag == chars.len()
                            || chars[after_flag].is_whitespace()
                            || chars[after_flag] == ';'
                            || chars[after_flag] == '&'
                            || chars[after_flag] == '|'
                        {
                            already_has_stdin_flag = true;
                        }
                    } else if rest.starts_with(&['-', '-', 's', 't', 'd', 'i', 'n']) {
                        let after_flag = next_idx + 7;
                        if after_flag == chars.len()
                            || chars[after_flag].is_whitespace()
                            || chars[after_flag] == ';'
                            || chars[after_flag] == '&'
                            || chars[after_flag] == '|'
                        {
                            already_has_stdin_flag = true;
                        }
                    }
                }

                if !already_has_stdin_flag {
                    result.push_str(" -S");
                }
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_unclosed_quote_ignores_literal_quote_characters() {
        for cmd in [
            "echo \"it's done\"",
            "echo \"a\\\"b\"",
            "echo it\\'s",
            "ls # don't care",
            "echo hi; # it's a comment\necho 'ok'",
            "echo $'it\\'s'",
            "cat <<'EOF' > f.txt\nit's fine\nEOF",
            "cat <<EOF\nshe said \"hi\nEOF\necho done",
            "cat <<-\"END\"\n\tdon't\n\tEND",
            "cat <<A <<B\nit's\nA\n\"\nB",
            "echo $((1<<2)) 'x'",
            "cat <<EOF\nunterminated heredoc it's",
            "echo ''\"\"",
        ] {
            assert_eq!(find_unclosed_quote(cmd), None, "{cmd:?}");
        }
    }

    #[test]
    fn find_unclosed_quote_detects_open_quote() {
        assert_eq!(find_unclosed_quote("echo \"hello"), Some('"'));
        assert_eq!(find_unclosed_quote("echo 'hello"), Some('\''));
        assert_eq!(find_unclosed_quote("echo \"it's"), Some('"'));
        assert_eq!(find_unclosed_quote("echo a#b 'x"), Some('\''));
        assert_eq!(
            find_unclosed_quote("cat <<EOF\nbody\nEOF\necho 'x"),
            Some('\'')
        );
    }

    #[test]
    #[cfg(not(windows))]
    fn test_normalize_shell_command_unix() {
        // Basic cases
        assert_eq!(normalize_shell_command("echo hello"), "echo hello");
        assert_eq!(normalize_shell_command("echo 'hello'"), "echo 'hello'");
        assert_eq!(normalize_shell_command("echo \"hello\""), "echo \"hello\"");

        // Missing quotes
        assert_eq!(normalize_shell_command("echo \"hello"), "echo \"hello\"");
        assert_eq!(normalize_shell_command("echo 'hello"), "echo 'hello'");

        // Escaped quotes (should NOT be counted as closing quotes)
        assert_eq!(
            normalize_shell_command("echo \"foo\\\"bar\""),
            "echo \"foo\\\"bar\""
        );

        // Nested quotes
        assert_eq!(
            normalize_shell_command("echo '\"hello\"'"),
            "echo '\"hello\"'"
        );
        assert_eq!(
            normalize_shell_command("echo \"'hello'\""),
            "echo \"'hello'\""
        );

        // Complex case with multiple escapes
        assert_eq!(
            normalize_shell_command("echo \"path: \\\"/tmp/foo\\\"\""),
            "echo \"path: \\\"/tmp/foo\\\"\""
        );

        // Trailing backslash (should be preserved)
        assert_eq!(normalize_shell_command("echo hello \\"), "echo hello \\");

        // Empty-string arguments are valid shell and must pass through unchanged
        for cmd in [
            "echo \"\"",
            "X=\"\" ; echo \"[$X]\"",
            "cat f && echo \"\" && echo \"=== B ===\" || echo \"no\"",
            "printf \"%s\\n\" \"\" \"x\"",
            "echo '{\"a\":\"\"}'",
            "echo \"it's done\"",
            "ls # don't care",
            "cat <<'EOF' > f.txt\nit's fine\nEOF",
        ] {
            assert_eq!(normalize_shell_command(cmd), cmd);
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn test_inject_sudo_stdin_flag() {
        assert_eq!(
            inject_sudo_stdin_flag("sudo apt update"),
            "sudo -S apt update"
        );
        assert_eq!(
            inject_sudo_stdin_flag("sudo -S apt update"),
            "sudo -S apt update"
        );
        assert_eq!(
            inject_sudo_stdin_flag("sudo --stdin apt update"),
            "sudo --stdin apt update"
        );
        assert_eq!(
            inject_sudo_stdin_flag("sudo -u root apt update"),
            "sudo -S -u root apt update"
        );
        assert_eq!(
            inject_sudo_stdin_flag("echo 'sudo apt update'"),
            "echo 'sudo apt update'"
        );
        assert_eq!(
            inject_sudo_stdin_flag("cd /tmp && sudo apt update"),
            "cd /tmp && sudo -S apt update"
        );
        assert_eq!(
            inject_sudo_stdin_flag("  sudo   apt update"),
            "  sudo -S   apt update"
        );
        assert_eq!(
            inject_sudo_stdin_flag("`sudo apt update`"),
            "`sudo -S apt update`"
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_normalize_shell_command_windows() {
        // Windows should pass through everything as-is
        assert_eq!(normalize_shell_command("echo hello"), "echo hello");
        assert_eq!(normalize_shell_command("echo \"hello"), "echo \"hello");
    }
}
