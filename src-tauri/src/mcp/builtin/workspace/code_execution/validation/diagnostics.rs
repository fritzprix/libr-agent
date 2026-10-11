/// Last few non-empty lines from stdout/stderr (joined).
fn output_tail(stdout: &str, stderr: &str, max_lines: usize) -> String {
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (true, true) => String::new(),
    };

    let lines: Vec<&str> = combined
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

/// True when the *tail* of output looks like a process waiting for stdin.
///
/// Intentionally narrow: whole-buffer substrings like `"? "`, `confirm`, or
/// `skipping` false-positive on diffs and logs (e.g. `git diff`).
pub fn looks_like_waiting_prompt(stdout: &str, stderr: &str) -> bool {
    let tail = output_tail(stdout, stderr, 3).to_lowercase();
    if tail.is_empty() {
        return false;
    }

    const STRONG_TAIL_PROMPTS: &[&str] = &[
        "[y/n]",
        "[yes/no]",
        "(y/n)",
        "(yes/no)",
        "enter password:",
        "password:",
    ];
    if STRONG_TAIL_PROMPTS
        .iter()
        .any(|prompt| tail.contains(prompt))
    {
        return true;
    }

    let last = tail.lines().next_back().unwrap_or("").trim();
    last.ends_with('?')
        && (last.contains("overwrite")
            || last.contains("(y/n)")
            || last.contains("[y/n]")
            || last.contains("yes/no")
            || last.contains("y/n"))
}

/// Returns true if the command contains a shell pipeline (`|` not part of `||`).
pub fn has_shell_pipeline(command: &str) -> bool {
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '|' {
            if chars.peek() == Some(&'|') {
                chars.next();
            } else {
                return true;
            }
        }
    }
    false
}

/// Warning appended when a pipeline returns exit code 0 but output shows failure markers.
///
/// Upstream commands often fail while the pipeline exit code stays 0 (rightmost command
/// wins). Agents commonly use `cmd 2>&1 | tail`, which moves failure text onto stdout, so
/// both streams are scanned.
pub fn exit_zero_pipeline_failure_warning(
    command: &str,
    stdout: &str,
    stderr: &str,
) -> Option<&'static str> {
    if !has_shell_pipeline(command) {
        return None;
    }
    if stdout.trim().is_empty() && stderr.trim().is_empty() {
        return None;
    }

    let combined = format!("{stdout}\n{stderr}").to_lowercase();
    let has_failure_marker = combined.contains("no such file")
        || combined.contains("command not found")
        || combined.contains("error:")
        || combined.contains("cannot open")
        || combined.contains("permission denied");

    if has_failure_marker {
        Some("⚠️ Note: Output reports errors despite exit code 0. In a shell pipeline, exit code 0 reflects the last command; an upstream command may have failed. Consider `set -o pipefail` or running commands separately.")
    } else {
        None
    }
}

/// Backward-compatible wrapper: stderr-only scan (stdout empty).
pub fn exit_zero_stderr_warning(command: &str, stderr: &str) -> Option<&'static str> {
    exit_zero_pipeline_failure_warning(command, "", stderr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_waiting_prompt_ignores_git_diff_noise() {
        let diff = r#"
diff --git a/src/lib/ai-service/factory.ts b/src/lib/ai-service/factory.ts
index 111..222 100644
--- a/src/lib/ai-service/factory.ts
+++ b/src/lib/ai-service/factory.ts
@@ -10,6 +10,7 @@ export function create() {
   const options?: Options;
   // confirm strategy selection
   return factory;
 }
"#;
        assert!(!looks_like_waiting_prompt(diff, ""));
        assert!(!looks_like_waiting_prompt("? foo.ts\n M bar.ts", ""));
        assert!(!looks_like_waiting_prompt(
            "no changes made\nskipping file",
            ""
        ));
    }

    #[test]
    fn test_looks_like_waiting_prompt_matches_strong_tail_only() {
        assert!(looks_like_waiting_prompt(
            "Package name: (demo)\nOverwrite? (y/n)",
            ""
        ));
        assert!(looks_like_waiting_prompt("", "Enter password:"));
        assert!(looks_like_waiting_prompt(
            "lots of log\nmore log\nContinue [y/n]",
            ""
        ));
        assert!(!looks_like_waiting_prompt("Overwrite complete", ""));
    }

    #[test]
    fn test_has_shell_pipeline() {
        assert!(has_shell_pipeline("xxd main.db-wal | head -50"));
        assert!(has_shell_pipeline("cat file.txt | grep foo | wc -l"));
        assert!(!has_shell_pipeline("which foo || echo not found"));
        assert!(!has_shell_pipeline(
            "python app.py --arg1 || python app.py --arg2"
        ));
        assert!(!has_shell_pipeline("ls -la"));
    }

    #[test]
    fn test_exit_zero_stderr_warning() {
        // Pipeline with stderr failure marker -> warning generated
        let warning = exit_zero_stderr_warning(
            "cd /app && xxd main.db-wal | head -50",
            "xxd: main.db-wal: No such file or directory",
        );
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("pipefail"));

        // Pipeline with command not found
        let warning_cnf = exit_zero_stderr_warning(
            "some_missing_cmd | head -10",
            "bash: line 1: some_missing_cmd: command not found",
        );
        assert!(warning_cnf.is_some());

        // Pipeline with clean/empty stderr -> no warning
        assert!(exit_zero_stderr_warning("cat file.txt | head -5", "").is_none());
        assert!(exit_zero_stderr_warning("cat file.txt | head -5", "   ").is_none());

        // Pipeline with non-failure stderr -> no warning
        assert!(exit_zero_stderr_warning(
            "cargo test | tee test.log",
            "warning: unused variable `x`"
        )
        .is_none());

        // Non-pipeline command with stderr failure marker -> no pipeline warning
        assert!(exit_zero_stderr_warning(
            "python script.py",
            "FileNotFoundError: [Errno 2] No such file or directory: 'foo'"
        )
        .is_none());
    }

    #[test]
    fn test_exit_zero_pipeline_failure_warning_scans_stdout_after_redirect() {
        // Harbor/agent pattern: failure merged into stdout via 2>&1 | tail
        let warning = exit_zero_pipeline_failure_warning(
            "ffmpeg -i video.mp4 -vn audio.wav 2>&1 | tail -5",
            "bash: line 1: ffmpeg: command not found",
            "",
        );
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("pipefail"));

        // stdout clean, stderr has marker (legacy path)
        assert!(exit_zero_pipeline_failure_warning(
            "xxd missing | head -50",
            "",
            "xxd: missing: No such file or directory",
        )
        .is_some());

        // No markers
        assert!(
            exit_zero_pipeline_failure_warning("cat file.txt | head -5", "hello\n", "",).is_none()
        );

        // Non-pipeline: ignore markers on stdout
        assert!(exit_zero_pipeline_failure_warning(
            "ffmpeg -i video.mp4",
            "bash: line 1: ffmpeg: command not found",
            "",
        )
        .is_none());
    }
}
