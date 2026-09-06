pub mod async_exec;
pub mod handlers;
pub mod isolated;
pub mod persistent;
pub mod policy;

pub(super) fn format_duration_ms(duration_ms: u64) -> String {
    if duration_ms == 0 {
        "< 1ms".to_string()
    } else {
        format!("{}ms", duration_ms)
    }
}

pub fn format_command_io_message(
    header: &str,
    stdout_label: &str,
    stdout: &str,
    stderr_label: &str,
    stderr: &str,
) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        // Exit success with empty streams is still a factual observation — do not
        // omit IO so agents do not treat "silent success" as missing feedback.
        (true, true) => format!("{header}\n\n(no stdout/stderr captured)"),
        (false, true) => format!("{header}\n\n{stdout_label}:\n{stdout}"),
        (true, false) => format!("{header}\n\n{stderr_label}:\n{stderr}"),
        (false, false) => {
            format!("{header}\n\n{stdout_label}:\n{stdout}\n\n{stderr_label}:\n{stderr}")
        }
    }
}
