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

pub fn write_file_then_shell_guidance() -> Vec<String> {
    use crate::mcp::builtin::workspace::types::RUN_SHELL_TOOL;

    vec![
        "Shell could not parse nested quotes or a heredoc in this one-liner.".to_string(),
        format!(
            "Use workspace__writeFile to save the script, then {} with a short command (e.g. `bash script.sh` or `python3 script.py`).",
            RUN_SHELL_TOOL
        ),
        format!(
            "On bash shells, {} also accepts a script field that runs multi-line code verbatim.",
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

/// Skill aliases are resolved by file tools only; a shell sees them as literal relative paths.
fn command_uses_skill_alias(command: &str) -> bool {
    ["@system-skills/", "@user-skills/", "@skills/"]
        .iter()
        .any(|alias| command.contains(alias))
}

fn skill_alias_in_shell_guidance(docker_session: bool) -> Vec<String> {
    let roots = if docker_session {
        Some((
            crate::session_isolation::CONTAINER_SYSTEM_SKILLS_DIR.to_string(),
            crate::session_isolation::CONTAINER_USER_SKILLS_DIR.to_string(),
        ))
    } else {
        crate::services::skill_service::get_system_skills_directory()
            .ok()
            .zip(crate::services::skill_service::get_user_skills_directory().ok())
            .map(|(system, user)| (system.display().to_string(), user.display().to_string()))
    };
    let location = match roots {
        Some((system, user)) => format!(
            "In this session use {system}/… for @system-skills/… and {user}/… for @user-skills/…."
        ),
        None => "Use the skill's base directory from the skill reference instead.".to_string(),
    };
    vec![
        "Skill aliases (@system-skills/…, @user-skills/…) are file-tool paths; the shell does not resolve them.".to_string(),
        location,
    ]
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
    docker_session: bool,
) -> Vec<String> {
    if let Some(hint) =
        crate::mcp::builtin::workspace::persistent_shell::sudo_shims::password_hint(stdout, stderr)
    {
        return vec![hint.to_string()];
    }
    if looks_like_shell_quote_parse_error(stdout, stderr) {
        return write_file_then_shell_guidance();
    }
    if looks_like_docker_chdir_workdir_failure(stdout, stderr) {
        return docker_chdir_workdir_failure_guidance();
    }
    if looks_like_windows_store_alias_stub(exit_code, stdout, stderr, command) {
        return windows_store_alias_stub_guidance();
    }
    if command_uses_skill_alias(command) {
        return skill_alias_in_shell_guidance(docker_session);
    }

    match exit_code {
        Some(1) => {
            let first = if stdout.is_empty() && stderr.is_empty() {
                "General command failure - the command printed no output"
            } else {
                "General command failure - review error output above"
            };
            with_rewrite_loop_hint(
                vec![
                    first.to_string(),
                    "Verify command syntax and required files exist".to_string(),
                    "Use workspace__listDirectory to check file paths".to_string(),
                ],
                command,
            )
        }
        // Exit 2 is not proof of a shell error: argparse-style usage errors and
        // programs with their own status codes also return 2 after running. Real
        // shell parse errors are caught above by `looks_like_shell_quote_parse_error`.
        Some(2) => with_rewrite_loop_hint(
            vec![
                if stdout.is_empty() && stderr.is_empty() {
                    "Exit code 2 - the command ran and printed no output. Many programs use 2 for a usage error or as their own status."
                } else {
                    "Exit code 2 - the command ran. Many programs use 2 for a usage error or as their own status; read its output above."
                }
                .to_string(),
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
        // The shell executors report -1 when the process ended without an exit status. No
        // rewrite-loop hint: a killed process says nothing about whether the script is wrong.
        Some(-1) | None => no_exit_status_guidance(stdout, stderr),
        Some(code) => {
            let mut guidance = vec![format!("Command failed with exit code: {code}")];
            if !(stdout.is_empty() && stderr.is_empty()) {
                guidance.push("Review error output above for specific failure reasons".to_string());
            }
            guidance.push("Verify command syntax and required dependencies".to_string());
            with_rewrite_loop_hint(guidance, command)
        }
    }
}

fn no_exit_status_guidance(stdout: &str, stderr: &str) -> Vec<String> {
    let mut guidance = vec![
        "Exit code -1 (or no exit code) means the process ended without an exit status: it timed out, was killed, or was stopped by a signal."
            .to_string(),
    ];
    if stdout.is_empty() && stderr.is_empty() {
        guidance.push("No output was captured, so there is no error output to review.".to_string());
    } else {
        guidance.push(
            "Review the output above to find the last step that ran before the process stopped."
                .to_string(),
        );
    }
    guidance.push(
        "Do not retry the same command unchanged. Check whether it was stopped on purpose, ran out of memory, or waits for input that it cannot get."
            .to_string(),
    );
    guidance
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
    fn failed_command_with_skill_alias_points_at_session_skill_dirs() {
        let stderr =
            "python3: can't open file '/app/@system-skills/skill-deployer/scripts/deploy_skill.py'";
        let command = "python3 @system-skills/skill-deployer/scripts/deploy_skill.py --help";

        let docker = shell_command_failure_guidance(Some(2), "", stderr, command, true);
        assert!(docker
            .iter()
            .any(|line| line.contains("/opt/libragent/skills/system/…")));

        let host = shell_command_failure_guidance(Some(2), "", stderr, command, false);
        assert!(host
            .iter()
            .any(|line| line.contains("shell does not resolve")));
        assert!(host
            .iter()
            .all(|line| !line.contains("/opt/libragent/skills")));

        let unrelated =
            shell_command_failure_guidance(Some(2), "", "error", "python3 run.py", true);
        assert!(unrelated
            .iter()
            .all(|line| !line.contains("/opt/libragent/skills")));
    }

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
            false,
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
    fn exit_2_with_no_output_says_so_instead_of_pointing_at_output() {
        let guidance = shell_command_failure_guidance(Some(2), "", "", "foo -x", false);
        let joined = guidance.join("\n");
        assert!(joined.contains("printed no output"), "{joined}");
        assert!(!joined.contains("read its output above"), "{joined}");
    }

    #[test]
    fn test_shell_command_failure_guidance_exit_2_without_quote_signal() {
        let guidance =
            shell_command_failure_guidance(Some(2), "", "usage: foo [-a]", "foo -x", false);
        let joined = guidance.join("\n");
        assert!(
            joined.contains("Exit code 2 - the command ran"),
            "exit 2 must be described as a completed command, not shell misuse: {joined}"
        );
        assert!(!joined.contains("Misuse of shell command"));
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
            false,
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
            false,
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
        let guidance =
            shell_command_failure_guidance(Some(9009), "", stderr, "python3 --version", false);
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
            false,
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
    fn exit_minus_one_without_output_does_not_point_at_error_output() {
        for exit_code in [Some(-1), None] {
            let joined =
                shell_command_failure_guidance(exit_code, "", "", "sleep 999", false).join("\n");
            assert!(joined.contains("timed out, was killed, or was stopped by a signal"));
            assert!(joined.contains("No output was captured"));
            assert!(!joined.contains("Review error output above"), "{joined}");
        }

        let with_output =
            shell_command_failure_guidance(Some(-1), "step 3 of 5", "", "make", false).join("\n");
        assert!(with_output.contains("Review the output above"));
    }

    #[test]
    fn other_exit_codes_only_point_at_error_output_when_some_was_captured() {
        for exit_code in [1, 42] {
            let empty =
                shell_command_failure_guidance(Some(exit_code), "", "", "grep -q x f", false)
                    .join("\n")
                    .to_ascii_lowercase();
            assert!(!empty.contains("review error output above"), "{empty}");

            let with_stderr =
                shell_command_failure_guidance(Some(exit_code), "", "boom", "grep -q x f", false)
                    .join("\n")
                    .to_ascii_lowercase();
            assert!(
                with_stderr.contains("review error output above"),
                "{with_stderr}"
            );
        }
    }

    #[test]
    fn test_shell_command_failure_guidance_routes_sigint() {
        let guidance = shell_command_failure_guidance(Some(130), "", "", "python app.py", false);
        assert!(guidance.iter().any(|step| step.contains("SIGINT")));
    }

    #[test]
    fn sudo_password_failures_get_the_store_as_hint() {
        use crate::mcp::builtin::workspace::persistent_shell::sudo_shims::{
            PASSWORD_PROMPT_HINT, WRONG_SUDO_PASSWORD_HINT,
        };
        let no_tty = shell_command_failure_guidance(
            Some(1),
            "",
            "sudo: a terminal is required to read the password",
            "sudo apt update",
            false,
        );
        assert_eq!(no_tty, vec![PASSWORD_PROMPT_HINT.to_string()]);
        let wrong = shell_command_failure_guidance(
            Some(1),
            "",
            "sudo: 1 incorrect password attempt",
            "sudo apt update",
            false,
        );
        assert_eq!(wrong, vec![WRONG_SUDO_PASSWORD_HINT.to_string()]);
    }
}
