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
        // Unix shell quoting normalization (existing logic)
        let mut normalized = raw_command.to_string();

        // 1. Detect incomplete quote pairs using a state machine
        let mut double_quote_count = 0;
        let mut single_quote_count = 0;
        let mut in_double_quote = false;
        let mut in_single_quote = false;
        let mut escaped = false;

        for c in normalized.chars() {
            if in_single_quote {
                // Inside single quotes, backslash is literal, only single quote escapes
                if c == '\'' {
                    in_single_quote = false;
                    single_quote_count += 1;
                }
            } else if in_double_quote {
                // Inside double quotes, backslash escapes next char
                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' {
                    escaped = true;
                    continue;
                }
                if c == '"' {
                    in_double_quote = false;
                    double_quote_count += 1;
                }
            } else {
                // Normal state
                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' {
                    escaped = true;
                    continue;
                }
                if c == '"' {
                    in_double_quote = true;
                    double_quote_count += 1;
                } else if c == '\'' {
                    in_single_quote = true;
                    single_quote_count += 1;
                }
            }
        }

        // 2. Add missing closing quotes
        if double_quote_count % 2 != 0 {
            normalized.push('"');
            info!("Shell command: Added missing double quote");
        }
        if single_quote_count % 2 != 0 {
            normalized.push('\'');
            info!("Shell command: Added missing single quote");
        }

        // 3. Fix consecutive quote patterns
        if normalized.contains("\"\"") {
            normalized = fix_consecutive_quotes(&normalized);
        }

        normalized
    }
}

/// Fix consecutive quotes based on context
#[cfg(not(windows))]
fn fix_consecutive_quotes(input: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' {
            // Consecutive quotes found

            // Check if the first quote is escaped (preceded by odd number of backslashes)
            let mut backslash_count = 0;
            let mut j = i;
            while j > 0 && chars[j - 1] == '\\' {
                backslash_count += 1;
                j -= 1;
            }

            if backslash_count % 2 != 0 {
                // It is an escaped quote (e.g. \"), so it's not a start of consecutive quotes
                result.push(chars[i]);
                i += 1;
                continue;
            }

            if i > 0 && chars[i - 1] != ' ' && chars[i - 1] != '=' {
                // If no space or equals before, escape the first one
                result.push('\\');
                result.push('"');
                i += 1; // Second quote processed in next loop
            } else if i + 2 < chars.len() && chars[i + 2] != ' ' {
                // If no space after, escape the second one
                result.push('"');
                result.push('\\');
                result.push('"');
                i += 2;
            } else {
                // Default: keep one, remove one
                result.push('"');
                i += 2;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    #[cfg(windows)]
    fn test_normalize_shell_command_windows() {
        // Windows should pass through everything as-is
        assert_eq!(normalize_shell_command("echo hello"), "echo hello");
        assert_eq!(normalize_shell_command("echo \"hello"), "echo \"hello");
    }
}
