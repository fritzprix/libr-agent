/// Detect commands that commonly require interactive input
///
/// This function checks for patterns indicating a command will wait for user input,
/// such as npm init without --yes, npx create-* without --force, REPL modes, etc.
///
/// Returns true if the command is likely to require interactive input.
pub fn is_likely_interactive_command(command: &str) -> bool {
    let cmd_lower = command.to_lowercase();
    let cmd_trimmed = cmd_lower.trim();

    // Pattern 1: Package manager initialization without non-interactive flags
    let package_init_patterns = [
        ("npm init", &["--yes", "-y"] as &[&str]),
        ("pnpm init", &["--yes", "-y"] as &[&str]),
        ("yarn init", &["--yes", "-y", "--private"] as &[&str]),
        ("bun init", &["--yes", "-y"] as &[&str]),
    ];

    for (pattern, non_interactive_flags) in package_init_patterns {
        if cmd_lower.contains(pattern) {
            let has_flag = non_interactive_flags
                .iter()
                .any(|flag| cmd_lower.contains(flag));
            if !has_flag {
                return true;
            }
        }
    }

    // Pattern 2: Scaffolding/creation tools (REMOVED: Too strict, prone to false positives)
    // We now rely on user intention or manual bypass for these.

    // Pattern 3: PowerShell interactive cmdlets (always interactive)
    let ps_interactive_cmdlets = ["read-host", "get-credential", "out-gridview"];
    for cmdlet in ps_interactive_cmdlets {
        if cmd_lower.contains(cmdlet) {
            return true;
        }
    }

    // Pattern 4: REPL mode detection (executable without arguments)
    // Check for bare executables that start interactive sessions
    let repl_executables = [
        "python",
        "python3",
        "py",
        "node",
        "irb",
        "ruby",
        "psql",
        "mysql",
        "mongosh",
        "redis-cli",
    ];

    for exec in repl_executables {
        // Match pattern: command starts with executable and has no script argument
        if cmd_trimmed == exec {
            // Exact match - definitely REPL
            return true;
        }

        // Check if it's "executable" followed only by flags (no positional args)
        if let Some(rest) = cmd_trimmed.strip_prefix(exec) {
            let rest = rest.trim();

            // Exception: "python -c", "python -m", "node -e" are NOT REPL (check first)
            // These execute code or modules non-interactively
            if rest.starts_with("-c ")
                || rest.starts_with("-m ")
                || rest.starts_with("-e ")
                || rest.starts_with("--eval ")
                || rest.starts_with("-c\t")
                || rest.starts_with("-m\t")
                || rest.starts_with("-e\t")
                || rest.starts_with("--eval\t")
            {
                continue;
            }

            // Relaxed REPL detection:
            // 1. If rest is empty -> Bare command (Interactive)
            // 2. If rest starts with -i or --interactive -> Explicit interactive flag
            // 3. Otherwise -> Assume non-interactive (allow --version, --help, -p, etc.)

            if rest.is_empty() {
                return true;
            }

            if rest.starts_with("-i") || rest.starts_with("--interactive") {
                return true;
            }

            // Allow all other flags (including --version, -v, --help, arbitrary args)
            continue;
        }
    }

    // Pattern 5: Git interactive commands
    let git_interactive = [
        "git add -p",
        "git add --patch",
        "git rebase -i",
        "git rebase --interactive",
    ];
    for pattern in git_interactive {
        if cmd_lower.contains(pattern) {
            return true;
        }
    }

    // Pattern 6: Interactive shells invoked directly
    let interactive_shells = [
        "bash\n",
        "bash\r",
        "bash ",
        "sh\n",
        "sh\r",
        "sh ",
        "powershell\n",
        "powershell\r",
        "pwsh\n",
        "pwsh\r",
    ];
    for shell_pattern in interactive_shells {
        if cmd_lower.ends_with(shell_pattern.trim()) || cmd_trimmed == shell_pattern.trim() {
            return true;
        }
    }

    false
}

#[cfg(windows)]
pub fn contains_unquoted_andand(input: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if in_single {
            if ch == '\'' {
                // PowerShell single-quote escaping: '' inside single quotes
                if i + 1 < chars.len() && chars[i + 1] == '\'' {
                    i += 2;
                    continue;
                }
                in_single = false;
            }
            i += 1;
            continue;
        }

        if in_double {
            // PowerShell escape inside double quotes via backtick
            if ch == '`' {
                i += 2;
                continue;
            }
            if ch == '"' {
                in_double = false;
            }
            i += 1;
            continue;
        }

        if ch == '\'' {
            in_single = true;
            i += 1;
            continue;
        }

        if ch == '"' {
            in_double = true;
            i += 1;
            continue;
        }

        if ch == '&' && i + 1 < chars.len() && chars[i + 1] == '&' {
            return true;
        }

        i += 1;
    }

    false
}

/// Platform-specific privilege detection for Unix systems
/// Detects commands that require elevated privileges (sudo, su, doas, pkexec)
#[cfg(unix)]
pub fn detect_privilege_escalation(command: &str) -> bool {
    let trimmed = command.trim_start();
    let patterns = ["sudo ", "su ", "doas ", "pkexec "];
    patterns.iter().any(|p| trimmed.starts_with(p))
}

/// Platform-specific privilege detection for Windows
/// Windows UAC cannot be detected from command string
/// Agent must explicitly set require_user_input=true
#[cfg(windows)]
pub fn detect_privilege_escalation(_command: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_interactive_command() {
        // ✅ Python -m commands should NOT be interactive
        assert!(!is_likely_interactive_command(
            "python -m unittest discover tests"
        ));
        assert!(!is_likely_interactive_command("python -m pytest"));
        assert!(!is_likely_interactive_command(
            "python3 -m pip install requests"
        ));
        assert!(!is_likely_interactive_command("py -m venv env"));

        // ✅ Python -c commands should NOT be interactive
        assert!(!is_likely_interactive_command("python -c 'print(123)'"));
        assert!(!is_likely_interactive_command(
            "python3 -c \"import sys; print(sys.version)\""
        ));

        // ✅ Node -e commands should NOT be interactive
        assert!(!is_likely_interactive_command(
            "node -e \"console.log('test')\""
        ));

        // ✅ Node with flags (like --version) should NOT be interactive
        assert!(!is_likely_interactive_command("node --version"));
        assert!(!is_likely_interactive_command("npm --version"));

        // ❌ Bare Python/Node should be interactive (REPL)
        assert!(is_likely_interactive_command("python"));
        assert!(is_likely_interactive_command("python3"));
        assert!(is_likely_interactive_command("node"));

        // ❌ Node with REPL flag
        assert!(is_likely_interactive_command("node -i"));
        assert!(is_likely_interactive_command("node --interactive"));

        // ❌ npm init without flags should be interactive
        assert!(is_likely_interactive_command("npm init"));
        // ✅ npm init with --yes should NOT be interactive
        assert!(!is_likely_interactive_command("npm init --yes"));

        // ✅ npx create-* should now be considered NON-interactive by default (relaxed rule)
        assert!(!is_likely_interactive_command("npx create-vite my-app"));
        assert!(!is_likely_interactive_command(
            "npx create-vite my-app --force"
        ));

        // ❌ Read-Host should be interactive
        assert!(is_likely_interactive_command("Read-Host 'Enter password'"));

        // ✅ Normal scripts should NOT be interactive
        assert!(!is_likely_interactive_command("python script.py"));
        assert!(!is_likely_interactive_command("node index.js"));
        assert!(!is_likely_interactive_command("cargo test"));
    }
}
