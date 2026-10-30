/// Normalize shell command for proper execution
/// Quoting is never rewritten: an unclosed quote is reported by the bash syntax
/// pre-check (`validate_bash_syntax`), and guessing a repair here only risks
/// corrupting valid commands the guess misreads.
pub fn normalize_shell_command(raw_command: &str) -> String {
    #[cfg(windows)]
    {
        // Windows: PowerShell handles both single and double quotes correctly
        // No normalization needed - pass command as-is to avoid breaking nested quotes
        // in Python/Node.js inline commands like: python -c "print('Hello')"
        tracing::info!("Windows command (no normalization): {}", raw_command);
        raw_command.to_string()
    }

    #[cfg(not(windows))]
    {
        // Inject -S flag for sudo commands to read from stdin in non-interactive/non-PTY shells
        inject_sudo_stdin_flag(raw_command)
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
    #[cfg(not(windows))]
    fn test_normalize_shell_command_unix() {
        // Quoting is never rewritten. Unclosed quotes are left for the bash syntax
        // pre-check to report rather than patched with a guess.
        for cmd in [
            "echo hello",
            "echo 'hello'",
            "echo \"hello\"",
            "echo \"hello",
            "echo 'hello",
            "echo \"foo\\\"bar\"",
            "echo '\"hello\"'",
            "echo \"'hello'\"",
            "echo \"path: \\\"/tmp/foo\\\"\"",
            "echo hello \\",
            "echo \"\"",
            "X=\"\" ; echo \"[$X]\"",
            "cat f && echo \"\" && echo \"=== B ===\" || echo \"no\"",
            "printf \"%s\\n\" \"\" \"x\"",
            "echo '{\"a\":\"\"}'",
            "echo \"it's done\"",
            "ls # don't care",
            "cat <<'EOF' > f.txt\nit's fine\nEOF",
            "echo \"$(printf '%s' \"it's\")\"",
            "echo \"`printf '%s' \"it's\"`\"",
        ] {
            assert_eq!(normalize_shell_command(cmd), cmd);
        }

        // sudo -S injection still applies
        assert_eq!(
            normalize_shell_command("cd /tmp && sudo apt update"),
            "cd /tmp && sudo -S apt update"
        );
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
