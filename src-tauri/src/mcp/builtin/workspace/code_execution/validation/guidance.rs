/// True when shell stderr/stdout indicates a quote or heredoc parse failure.
///
/// Harbor trajectories show agents retrying nested quotes/`cat <<EOF` one-liners
/// after `unexpected EOF while looking for matching` — escalate to writeFile.
/// Bash localizes this message (e.g. Korean), so match English and common locales.
pub fn looks_like_shell_quote_parse_error(stdout: &str, stderr: &str) -> bool {
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (true, true) => return false,
    };
    let lower = combined.to_ascii_lowercase();

    const ENGLISH_SIGNALS: &[&str] = &[
        "unexpected eof while looking for matching",
        "syntax error: unexpected end of file",
        "syntax error near unexpected token",
        "here-document at line",
        "delimited by end-of-file",
        "missing terminating",
        "unmatched '",
        "unmatched \"",
    ];
    if ENGLISH_SIGNALS.iter().any(|signal| lower.contains(signal)) {
        return true;
    }

    // Localized bash (ko_KR): "`''을(를) 찾는 도중 예상치 못한 파일의 끝"
    // and heredoc: "here-document가 ... 파일의 끝으로 구분함"
    const LOCALIZED_SIGNALS: &[&str] = &[
        "예상치 못한 파일의 끝",
        "파일의 끝으로 구분",
        "찾는 도중 예상치 못한",
    ];
    LOCALIZED_SIGNALS
        .iter()
        .any(|signal| combined.contains(signal))
}

fn write_file_then_shell_guidance() -> Vec<String> {
    use crate::mcp::builtin::workspace::types::RUN_SHELL_TOOL;

    vec![
        "Shell could not parse nested quotes or a heredoc in this one-liner.".to_string(),
        format!(
            "Use workspace__writeFile to save the script, then {} with a short command (e.g. `bash script.sh` or `python3 script.py`).",
            RUN_SHELL_TOOL
        ),
        "Do not embed multi-line scripts with nested quotes inside a single shell call.".to_string(),
    ]
}

/// True when the failed command likely executed a script file (not a missing binary).
fn looks_like_script_file_run(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    const SCRIPT_EXTS: &[&str] = &[
        ".py", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".sh", ".bash", ".zsh", ".vim", ".rb", ".pl",
        ".ps1",
    ];
    SCRIPT_EXTS.iter().any(|ext| lower.contains(ext))
}

fn rewrite_loop_recovery_hint() -> String {
    "If this ran a script you just wrote, prefer a minimal diagnostic one-liner (or a tiny repro) before rewriting the entire script.".to_string()
}

fn with_rewrite_loop_hint(mut guidance: Vec<String>, command: &str) -> Vec<String> {
    if looks_like_script_file_run(command) {
        guidance.push(rewrite_loop_recovery_hint());
    }
    guidance
}

/// True when Docker/OCI could not start the process because the configured
/// container working directory is missing (`chdir to cwd (...) failed`).
///
/// Harbor traces show exit 127 with this stderr; the generic "command not found"
/// recovery misleads agents into apt-install loops.
pub fn looks_like_docker_chdir_workdir_failure(stdout: &str, stderr: &str) -> bool {
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (true, true) => return false,
    };
    let lower = combined.to_ascii_lowercase();
    if !lower.contains("chdir to cwd") {
        return false;
    }
    lower.contains("oci runtime")
        || lower.contains("unable to start container process")
        || lower.contains("set in config.json failed")
        || lower.contains("no such file or directory")
}

fn docker_chdir_workdir_failure_guidance() -> Vec<String> {
    vec![
        "Container shell could not start: the configured working directory is missing (Docker chdir failed) — this is not a missing binary."
            .to_string(),
        "Do not install packages or retry the same command expecting PATH fixes.".to_string(),
        "Prefer non-shell tools for the goal (e.g. desktop__computerControl / media__captureScreen), or report the isolation/workdir failure if shell is required."
            .to_string(),
    ]
}

/// Windows cmd.exe / App Execution Alias stub: binary found on PATH but not installed
/// (Microsoft Store placeholder). Common for `python` / `python3` under WindowsApps.
///
/// Exit 9009 alone is NOT enough — on Windows that is the generic "command not found"
/// code for any missing binary (`mvn`, `ffmpeg`, …).
pub fn looks_like_windows_store_alias_stub(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    command: &str,
) -> bool {
    let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    let cmd_lower = command.to_ascii_lowercase();
    let store_text = combined.contains("microsoft store")
        || combined.contains("app execution aliases")
        || combined.contains("windowsapps");
    let python_not_found = combined.contains("python was not found");
    let pythonish_command = cmd_lower.contains("python");
    (matches!(exit_code, Some(9009)) && (store_text || pythonish_command || python_not_found))
        || (store_text && python_not_found)
}

fn windows_store_alias_stub_guidance() -> Vec<String> {
    vec![
        "Microsoft Store execution-alias stub detected — this program is NOT installed (exit 9009 is common for WindowsApps python/python3 placeholders)."
            .to_string(),
        "Do not search the filesystem for another interpreter; switch to an installed runtime (e.g. Node.js) or a native LibrAgent tool fallback."
            .to_string(),
        "If Python is required, install a real distribution and ensure its directory appears on PATH ahead of Local\\Microsoft\\WindowsApps."
            .to_string(),
    ]
}

/// Outcome-conditioned next-step hints for failed one-shot / persistent shell runs.
pub fn shell_command_failure_guidance(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    command: &str,
) -> Vec<String> {
    if looks_like_shell_quote_parse_error(stdout, stderr) {
        return write_file_then_shell_guidance();
    }
    if looks_like_docker_chdir_workdir_failure(stdout, stderr) {
        return docker_chdir_workdir_failure_guidance();
    }
    if looks_like_windows_store_alias_stub(exit_code, stdout, stderr, command) {
        return windows_store_alias_stub_guidance();
    }

    match exit_code {
        Some(1) => with_rewrite_loop_hint(
            vec![
                "General command failure - review error output above".to_string(),
                "Verify command syntax and required files exist".to_string(),
                "Use workspace__listDirectory to check file paths".to_string(),
            ],
            command,
        ),
        Some(2) => with_rewrite_loop_hint(
            vec![
                "Misuse of shell command or invalid arguments".to_string(),
                format!(
                    "For multi-line or quote-heavy scripts, use workspace__writeFile then {} with a short command.",
                    crate::mcp::builtin::workspace::types::RUN_SHELL_TOOL
                ),
                "Verify required files exist with workspace__listDirectory.".to_string(),
            ],
            command,
        ),
        Some(127) => vec![
            "Command not found - program is not installed or not in PATH".to_string(),
            "Verify the program is installed on the system".to_string(),
            "Check for typos in the command name".to_string(),
        ],
        Some(126) => vec![
            "Command found but not executable".to_string(),
            "Check file permissions".to_string(),
            "Verify the file is a valid executable".to_string(),
        ],
        Some(code) if is_signal_interrupt_exit(code) => shell_signal_interrupt_guidance(code),
        Some(code) => with_rewrite_loop_hint(
            vec![
                format!("Command failed with exit code: {code}"),
                "Review error output above for specific failure reasons".to_string(),
                "Verify command syntax and required dependencies".to_string(),
            ],
            command,
        ),
        None => with_rewrite_loop_hint(
            vec![
                "Command failed without an exit code".to_string(),
                "Review error output above for specific failure reasons".to_string(),
                "Verify command syntax and required dependencies".to_string(),
            ],
            command,
        ),
    }
}

/// Exit codes that mean the process was stopped by a common termination signal
/// (`128 + signal`): SIGINT → 130, SIGTERM → 143.
pub fn is_signal_interrupt_exit(exit_code: i32) -> bool {
    signal_interrupt_label(exit_code).is_some()
}

/// Human-readable label for known signal-interrupt exit codes.
pub fn signal_interrupt_label(exit_code: i32) -> Option<&'static str> {
    match exit_code {
        130 => Some("SIGINT (Ctrl+C)"),
        143 => Some("SIGTERM"),
        _ => None,
    }
}

/// Guidance when a shell command ends via SIGINT/SIGTERM rather than a normal failure.
pub fn shell_signal_interrupt_guidance(exit_code: i32) -> Vec<String> {
    let signal = signal_interrupt_label(exit_code).unwrap_or("a termination signal");
    vec![
        format!(
            "Exit {exit_code} means the process was interrupted by {signal} — often expected when testing cancellation or Ctrl+C handling."
        ),
        "Inspect stdout/stderr above for cleanup evidence before rewriting the program under test."
            .to_string(),
        "Treat this as a failed check only if the output shows unexpected behavior; do not assume the shell exit alone means the implementation is broken."
            .to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_shell_quote_parse_error_detects_bash_eof_quote() {
        assert!(looks_like_shell_quote_parse_error(
            "",
            "bash: -c: line 23: unexpected EOF while looking for matching `''"
        ));
        assert!(looks_like_shell_quote_parse_error(
            "",
            "bash: -c: line 6: unexpected EOF while looking for matching `\"'"
        ));
        assert!(looks_like_shell_quote_parse_error(
            "",
            "syntax error: unexpected end of file"
        ));
        assert!(looks_like_shell_quote_parse_error(
            "",
            "./script.sh: line 10: warning: here-document at line 3 delimited by end-of-file (wanted `EOF')"
        ));
        assert!(looks_like_shell_quote_parse_error(
            "",
            "bash: -c: 줄 1: `''을(를) 찾는 도중 예상치 못한 파일의 끝"
        ));
        assert!(!looks_like_shell_quote_parse_error(
            "",
            "Error: file not found"
        ));
        assert!(!looks_like_shell_quote_parse_error("ok output", ""));
    }

    #[test]
    fn test_shell_command_failure_guidance_escalates_quote_parse() {
        let guidance = shell_command_failure_guidance(
            Some(2),
            "",
            "bash: -c: line 20: unexpected EOF while looking for matching `''",
            "python3 /tmp/script.py",
        );
        let joined = guidance.join("\n");
        assert!(
            joined.contains("writeFile"),
            "quote-parse failures must point at workspace__writeFile: {joined}"
        );
        assert!(
            joined.contains(crate::mcp::builtin::workspace::types::RUN_SHELL_TOOL),
            "guidance must name the platform shell tool: {joined}"
        );
        assert!(
            !joined.contains("Misuse of shell command"),
            "generic exit-2 text must not override quote-parse guidance: {joined}"
        );
        assert!(
            !joined.contains("rewriting the entire script"),
            "quote-parse path should not add rewrite-loop hint: {joined}"
        );
    }

    #[test]
    fn test_shell_command_failure_guidance_exit_2_without_quote_signal() {
        let guidance = shell_command_failure_guidance(Some(2), "", "usage: foo [-a]", "foo -x");
        let joined = guidance.join("\n");
        assert!(joined.contains("Misuse of shell command"));
        assert!(
            joined.contains("writeFile"),
            "plain exit 2 should still offer workspace__writeFile alternative: {joined}"
        );
        assert!(
            !joined.contains("rewriting the entire script"),
            "non-script commands should not get rewrite-loop hint: {joined}"
        );
    }

    #[test]
    fn test_shell_command_failure_guidance_script_run_adds_rewrite_loop_hint() {
        let guidance = shell_command_failure_guidance(
            Some(1),
            "",
            "AssertionError: expected != actual",
            "cd /workspace && python3 repro.py",
        );
        let joined = guidance.join("\n");
        assert!(
            joined.contains("rewriting the entire script"),
            "failed script runs should discourage full rewrite loops: {joined}"
        );
    }

    #[test]
    fn test_shell_command_failure_guidance_command_not_found_skips_rewrite_hint() {
        let guidance = shell_command_failure_guidance(
            Some(127),
            "",
            "bash: line 1: somebin: command not found",
            "somebin --help",
        );
        let joined = guidance.join("\n");
        assert!(joined.contains("Command not found"));
        assert!(
            !joined.contains("rewriting the entire script"),
            "missing binaries are not rewrite-loop cases: {joined}"
        );
    }

    #[test]
    fn test_looks_like_docker_chdir_workdir_failure() {
        let oci = r#"OCI runtime exec failed: exec failed: unable to start container process: chdir to cwd ("/app") set in config.json failed: no such file or directory"#;
        assert!(looks_like_docker_chdir_workdir_failure("", oci));
        assert!(looks_like_docker_chdir_workdir_failure(oci, ""));
        assert!(!looks_like_docker_chdir_workdir_failure(
            "",
            "bash: line 1: foo: command not found"
        ));
        assert!(!looks_like_docker_chdir_workdir_failure(
            "",
            "chdir: cannot change directory"
        ));
    }

    #[test]
    fn test_shell_command_failure_guidance_windows_store_alias_stub() {
        let stderr = "Python was not found; run without arguments to install from the Microsoft Store, or disable this shortcut from Settings > Apps > Advanced app settings > App execution aliases.";
        assert!(looks_like_windows_store_alias_stub(
            Some(9009),
            "",
            stderr,
            "python3 --version"
        ));
        assert!(
            !looks_like_windows_store_alias_stub(
                Some(9009),
                "",
                "'mvn' is not recognized as an internal or external command",
                "mvn -v"
            ),
            "generic Windows 9009 must not be treated as a Store python stub"
        );
        let guidance = shell_command_failure_guidance(Some(9009), "", stderr, "python3 --version");
        let joined = guidance.join("\n");
        assert!(
            joined.contains("execution-alias stub") || joined.contains("NOT installed"),
            "expected Store-stub guidance, got: {joined}"
        );
        assert!(
            !joined.contains("Review error output above for specific failure reasons"),
            "generic exit-code guidance must not replace stub guidance: {joined}"
        );
    }

    #[test]
    fn test_shell_command_failure_guidance_docker_chdir_overrides_exit_127() {
        let stderr = r#"OCI runtime exec failed: exec failed: unable to start container process: chdir to cwd ("/app") set in config.json failed: no such file or directory"#;
        let guidance = shell_command_failure_guidance(
            Some(127),
            "",
            stderr,
            "which google-chrome || which chromium",
        );
        let joined = guidance.join("\n");
        assert!(
            joined.contains("configured working directory is missing"),
            "OCI chdir must own recovery: {joined}"
        );
        assert!(
            !joined.contains("Command not found"),
            "must not mislabel Docker chdir as missing binary: {joined}"
        );
        assert!(
            !joined.contains("installed on the system"),
            "must not push package-install recovery: {joined}"
        );
    }

    #[test]
    fn test_signal_interrupt_exit_codes() {
        assert!(is_signal_interrupt_exit(130));
        assert!(is_signal_interrupt_exit(143));
        assert!(!is_signal_interrupt_exit(0));
        assert!(!is_signal_interrupt_exit(1));
        assert!(!is_signal_interrupt_exit(127));
        assert_eq!(signal_interrupt_label(130), Some("SIGINT (Ctrl+C)"));
        assert_eq!(signal_interrupt_label(143), Some("SIGTERM"));
        assert_eq!(signal_interrupt_label(1), None);
    }

    #[test]
    fn test_shell_signal_interrupt_guidance_avoids_rewrite_pressure() {
        let guidance = shell_signal_interrupt_guidance(130);
        let joined = guidance.join("\n");
        assert!(joined.contains("SIGINT"));
        assert!(joined.contains("often expected"));
        assert!(joined.contains("Inspect stdout/stderr"));
        assert!(!joined.contains("General command failure"));
    }

    #[test]
    fn test_shell_command_failure_guidance_routes_sigint() {
        let guidance = shell_command_failure_guidance(Some(130), "", "", "python app.py");
        assert!(guidance.iter().any(|step| step.contains("SIGINT")));
    }
}
