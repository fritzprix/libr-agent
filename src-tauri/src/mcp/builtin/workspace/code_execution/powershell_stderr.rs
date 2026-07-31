//! Sanitize PowerShell host chrome that leaks into captured stderr.
//!
//! Windows isolated shell runs commands via a temp `.ps1` wrapper. PowerShell may
//! append ScriptStackTrace lines (`at <ScriptBlock>, …`) or NativeCommandError
//! frames (`exe : message` + `At path.ps1` + CategoryInfo) even when the native
//! command succeeded. Those frames confuse LLM tool consumers.

/// Strip PowerShell wrapper / NativeCommandError chrome from stderr.
///
/// Keeps the underlying message (e.g. cargo progress) while removing:
/// - `at <ScriptBlock>, …cmd_*.ps1` ScriptStackTrace lines
/// - `At path.ps1:line char:` frames and `+` continuation / CategoryInfo blocks
/// - `exe : message` wrappers when followed by an `At …` frame
pub fn sanitize_powershell_stderr(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if trimmed.starts_with("at <ScriptBlock>,") {
            i += 1;
            continue;
        }

        // NativeCommandError display: "cargo : Compiling …" followed by At/.ps1 frame
        if let Some(message) = native_command_error_message(line) {
            if i + 1 < lines.len() && lines[i + 1].trim_start().starts_with("At ") {
                if !message.is_empty() {
                    out.push(message);
                }
                i += 1; // move to At line
                i = skip_powershell_error_frame(lines.as_slice(), i);
                continue;
            }
        }

        if trimmed.starts_with("At ") && looks_like_powershell_script_frame(trimmed) {
            i = skip_powershell_error_frame(lines.as_slice(), i);
            continue;
        }

        out.push(line.to_string());
        i += 1;
    }

    let mut sanitized = out.join("\n");
    if stderr.ends_with('\n') && !sanitized.is_empty() {
        sanitized.push('\n');
    }
    sanitized
}

fn native_command_error_message(line: &str) -> Option<String> {
    // "cmd : progress" / "cargo : Compiling foo" — require non-space token before " : "
    let trimmed = line.trim_end();
    let (left, right) = trimmed.split_once(" : ")?;
    if left.is_empty() || left.chars().any(char::is_whitespace) {
        return None;
    }
    // Avoid stripping normal prose that happens to contain " : "
    if left.len() > 64 {
        return None;
    }
    Some(right.trim().to_string())
}

fn looks_like_powershell_script_frame(trimmed_at_line: &str) -> bool {
    trimmed_at_line.contains(".ps1:")
        || trimmed_at_line.contains("<ScriptBlock>")
        || trimmed_at_line.contains("<No file>")
}

fn skip_powershell_error_frame(lines: &[&str], mut i: usize) -> usize {
    if i < lines.len() && lines[i].trim_start().starts_with("At ") {
        i += 1;
    }
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with('+') {
            i += 1;
            continue;
        }
        break;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scriptblock_stack_trace() {
        let raw = "Compiling libragent v0.8.36\n\
                   at <ScriptBlock>, C:\\ws\\.libragent\\tmp\\cmd_abc_17.ps1: line 5\n\
                   at <ScriptBlock>, <No file>: line 1\n";
        let cleaned = sanitize_powershell_stderr(raw);
        assert_eq!(cleaned, "Compiling libragent v0.8.36\n");
        assert!(!cleaned.contains("ScriptBlock"));
    }

    #[test]
    fn unwraps_native_command_error_frame() {
        let raw = "cmd : progress message\n\
                   At C:\\ws\\.libragent\\tmp\\cmd_abc_0.ps1:5 char:3\n\
                   +   cmd /c \"echo progress 1>&2\"\n\
                   +   ~~~~~~~~~~~~~~~~~~~~~~~~~~~\n\
                       + CategoryInfo          : NotSpecified: (progress message:String) [], RemoteException\n\
                       + FullyQualifiedErrorId : NativeCommandError\n";
        let cleaned = sanitize_powershell_stderr(raw);
        assert_eq!(cleaned.trim(), "progress message");
        assert!(!cleaned.contains("CategoryInfo"));
        assert!(!cleaned.contains("cmd_abc_0.ps1"));
    }

    #[test]
    fn preserves_unrelated_stderr() {
        let raw = "error: could not compile `foo`\n\nCaused by:\n  linker failed\n";
        assert_eq!(sanitize_powershell_stderr(raw), raw);
    }
}
