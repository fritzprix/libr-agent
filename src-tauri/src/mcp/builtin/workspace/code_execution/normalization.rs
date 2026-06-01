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

        // 4. Inject -S flag for sudo commands to read from stdin in non-interactive/non-PTY shells
        normalized = inject_sudo_stdin_flag(&normalized);

        normalized
    }
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
    }

    #[test]
    #[cfg(windows)]
    fn test_normalize_shell_command_windows() {
        // Windows should pass through everything as-is
        assert_eq!(normalize_shell_command("echo hello"), "echo hello");
        assert_eq!(normalize_shell_command("echo \"hello"), "echo \"hello");
    }
}
