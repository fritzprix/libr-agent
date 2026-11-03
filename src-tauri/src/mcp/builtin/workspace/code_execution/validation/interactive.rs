use crate::mcp::builtin::workspace::StdinDelivery;
use serde_json::Value;

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
            if starts_with_non_repl_flag(rest) {
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

fn starts_with_non_repl_flag(rest: &str) -> bool {
    matches!(
        rest.split_whitespace().next(),
        Some("-c" | "-m" | "-e" | "--eval")
    )
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

/// Commands that prompt for a password when they run.
#[cfg(unix)]
const PRIVILEGE_COMMANDS: [&str; 4] = ["sudo", "su", "doas", "pkexec"];

/// Shells whose `-c` argument holds a nested command line.
#[cfg(unix)]
const SHELL_COMMANDS: [&str; 6] = ["bash", "sh", "zsh", "dash", "ksh", "ash"];

/// Guard against pathological nesting such as `bash -c 'sh -c ...'` chains.
#[cfg(unix)]
const MAX_NESTING_DEPTH: usize = 4;

/// Platform-specific privilege detection for Unix systems
/// Detects commands that require elevated privileges (sudo, su, doas, pkexec)
///
/// Detection must stay in sync with `normalization::inject_sudo_stdin_flag`,
/// which appends `-S` to every unquoted `sudo` in the command line. If that
/// rewrite fires while this check does not, sudo reads the shell's own stdin
/// and the call blocks until timeout (#2534, seen as the 5 minute hang in
/// #2532). So a privilege command counts wherever it starts a command
/// segment: after `&&`, `||`, `;`, `|`, newlines, parentheses, braces or
/// backticks, and inside a shell's `-c` string. A privilege word that is only
/// an argument (`echo sudo`) or quoted data (`echo 'sudo x'`) does not count,
/// because no password dialog is warranted there.
#[cfg(unix)]
pub fn detect_privilege_escalation(command: &str) -> bool {
    detect_privilege_escalation_inner(command, 0)
}

#[cfg(unix)]
fn detect_privilege_escalation_inner(command: &str, depth: usize) -> bool {
    split_command_segments(command).into_iter().any(|segment| {
        let Some((start, end)) = first_word_span(segment) else {
            return false;
        };
        let word = &segment[start..end];

        if PRIVILEGE_COMMANDS.contains(&word) {
            return true;
        }

        if SHELL_COMMANDS.contains(&word) && depth < MAX_NESTING_DEPTH {
            if let Some(inner) = shell_inner_command(&segment[end..]) {
                return detect_privilege_escalation_inner(inner, depth + 1);
            }
        }

        false
    })
}

/// Split a command line at unquoted command separators.
///
/// Escaped and quoted separators stay inside their segment so that data such
/// as `echo 'a; b'` is not mistaken for a command chain.
#[cfg(unix)]
fn split_command_segments(command: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for (index, ch) in command.char_indices() {
        if in_single_quote {
            if ch == '\'' {
                in_single_quote = false;
            }
            continue;
        }

        if in_double_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double_quote = false;
            }
            continue;
        }

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            ';' | '&' | '|' | '\n' | '(' | ')' | '{' | '}' | '`' => {
                segments.push(&command[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    segments.push(&command[start..]);
    segments
}

/// Byte span of the first word in `segment`, ignoring leading whitespace and
/// opening quotes so that `'sudo true'` still reports `sudo`.
#[cfg(unix)]
fn first_word_span(segment: &str) -> Option<(usize, usize)> {
    let trimmed = segment.len() - segment.trim_start().len();
    let unquoted = segment[trimmed..].trim_start_matches(['\'', '"']);
    let word_start = segment.len() - unquoted.len();
    let word_len = unquoted
        .find(|c: char| c.is_whitespace() || c == '\'' || c == '"')
        .unwrap_or(unquoted.len());

    (word_len > 0).then_some((word_start, word_start + word_len))
}

/// Remove one layer of matching surrounding quotes.
#[cfg(unix)]
fn strip_wrapping_quotes(text: &str) -> &str {
    let mut chars = text.chars();
    let Some(open) = chars.next() else {
        return text;
    };
    let close = match open {
        '\'' => '\'',
        '"' => '"',
        _ => return text,
    };

    if text.len() >= 2 && text.ends_with(close) {
        &text[open.len_utf8()..text.len() - close.len_utf8()]
    } else {
        text
    }
}

/// Extract the command string passed to a shell's `-c` flag, if any.
#[cfg(unix)]
fn shell_inner_command(args: &str) -> Option<&str> {
    let mut rest = args.trim_start();

    while rest.starts_with('-') && !rest.starts_with("--") {
        let flag_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let flag = &rest[..flag_end];
        let inner = rest[flag_end..].trim_start();

        if flag.len() > 1 && flag[1..].contains('c') {
            // The shell consumes the quoting, so `sh -c 'a && sudo b'` runs
            // `a && sudo b`. Unwrap that layer before looking inside it.
            let payload = strip_wrapping_quotes(inner);
            return (!payload.is_empty()).then_some(payload);
        }

        rest = inner;
    }

    None
}

/// Platform-specific privilege detection for Windows
/// Windows UAC cannot be detected from command string
/// Agent must explicitly set require_user_input=true
#[cfg(windows)]
pub fn detect_privilege_escalation(_command: &str) -> bool {
    false
}

/// Resolve how interactive user input should be delivered to a shell command.
pub fn resolve_stdin_delivery(command: &str, args: &Value) -> StdinDelivery {
    if let Some(explicit) = args
        .get("stdinDelivery")
        .or_else(|| args.get("stdin_delivery"))
        .and_then(|value| value.as_str())
    {
        return match explicit.to_ascii_lowercase().as_str() {
            "child" => StdinDelivery::Child,
            _ => StdinDelivery::Host,
        };
    }

    let cmd_lower = command.to_ascii_lowercase();

    if cmd_lower.contains("read-host") || cmd_lower.contains("get-credential") {
        return StdinDelivery::Host;
    }

    #[cfg(unix)]
    if detect_privilege_escalation(command) {
        return StdinDelivery::Host;
    }

    if command.contains("--code-stdin")
        || command.contains("--password-stdin")
        || cmd_lower.contains(" --stdin")
        || cmd_lower.ends_with(" --stdin")
    {
        return StdinDelivery::Child;
    }

    #[cfg(unix)]
    if command.trim() == "cat" {
        return StdinDelivery::Child;
    }

    StdinDelivery::Host
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

    #[test]
    fn test_resolve_stdin_delivery_auto_detects_child_stdin_flags() {
        let args = Value::Object(Default::default());
        assert_eq!(
            resolve_stdin_delivery("python setup.py --action sign_in --code-stdin", &args),
            StdinDelivery::Child
        );
        assert_eq!(
            resolve_stdin_delivery("python setup.py --action sign_in --password-stdin", &args),
            StdinDelivery::Child
        );
    }

    #[test]
    fn test_resolve_stdin_delivery_prefers_host_for_read_host() {
        let args = Value::Object(Default::default());
        assert_eq!(
            resolve_stdin_delivery("$code = Read-Host; python setup.py --code-stdin", &args),
            StdinDelivery::Host
        );
    }

    #[test]
    fn test_resolve_stdin_delivery_honors_explicit_override() {
        let args = serde_json::json!({ "stdinDelivery": "child" });
        assert_eq!(
            resolve_stdin_delivery("$env:LIBRAGENT_TELEGRAM_CODE = Read-Host", &args),
            StdinDelivery::Child
        );
    }

    /// Regression for #2534: `cd x && sudo ...` was invisible to the prefix
    /// check, so `sudo -S` was still injected by `normalization.rs` while no
    /// password dialog opened. sudo then read the shell's own stdin and the
    /// call blocked until timeout (the 5 minute hang in #2532).
    #[test]
    #[cfg(unix)]
    fn test_detect_privilege_escalation_finds_chained_commands() {
        for command in [
            "cd /tmp && sudo true",
            "echo x; sudo true",
            "ls | sudo tee out.txt",
            "ls || sudo true",
            "(cd /tmp && sudo true)",
            "{ sudo true; }",
            "echo x\ndoas true",
            "true&&sudo true",
            "bash -c 'sudo true'",
            "bash -c \"sudo true\"",
            "sh -lc 'cd /tmp && sudo true'",
        ] {
            assert!(
                detect_privilege_escalation(command),
                "expected privilege detection for chained command: {command:?}"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_detect_privilege_escalation_keeps_prefix_detection() {
        for command in [
            "sudo apt update",
            "   sudo apt update",
            "su - root",
            "doas true",
            "pkexec ls",
        ] {
            assert!(
                detect_privilege_escalation(command),
                "expected privilege detection for: {command:?}"
            );
        }
    }

    /// A false positive opens a password dialog nobody asked for, so plain
    /// arguments and quoted text must not count as escalation.
    #[test]
    #[cfg(unix)]
    fn test_detect_privilege_escalation_ignores_arguments_and_quotes() {
        for command in [
            "ls -la",
            "echo sudo",
            "echo test; echo sudo",
            "cat sudoers",
            "sudoku --version",
            "echo 'sudo apt install'",
            "echo \"sudo apt install\"",
            "printf '%s' su",
            "bash -c 'echo sudo'",
            "bash build.sh",
        ] {
            assert!(
                !detect_privilege_escalation(command),
                "unexpected privilege detection for: {command:?}"
            );
        }
    }
}
