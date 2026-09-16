pub mod async_exec;
pub mod handlers;
pub mod isolated;
pub mod persistent;
pub mod policy;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

use super::validation;

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

/// Present SIGINT/SIGTERM shell exits as an informational interrupt, not ✗ failure.
pub fn shell_signal_interrupt_result(
    exit_code: i32,
    duration_ms: u64,
    stdout: &str,
    stderr: &str,
    structured_data: serde_json::Value,
) -> MCPResult {
    let signal = validation::signal_interrupt_label(exit_code).unwrap_or("a termination signal");
    let header = format!(
        "Command interrupted by {} in {} (exit code: {})",
        signal,
        format_duration_ms(duration_ms),
        exit_code
    );
    let message = format_command_io_message(&header, "Command output", stdout, "Stderr", stderr);

    guided_error(
        ErrorCategory::ProcessInterrupted,
        message,
        ToolGroup::Workspace,
    )
    .guidance(validation::shell_signal_interrupt_guidance(exit_code))
    .to_mcp_result_with_data(Some(structured_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::MCPContent;

    fn text_of(result: &MCPResult) -> String {
        result
            .content
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|item| match item {
                MCPContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn signal_interrupt_result_is_notice_not_error() {
        let data = serde_json::json!({
            "command": "python test_interrupt.py",
            "exit_code": 130,
            "status": "interrupted",
            "duration_ms": 42,
            "execution_type": "persistent"
        });
        let result = shell_signal_interrupt_result(130, 42, "cleanup ok\n", "", data);
        let text = text_of(&result);

        assert_eq!(result.is_error, Some(false));
        assert!(!text.contains('✗'));
        assert!(text.contains("Notice:"));
        assert!(text.contains("SIGINT"));
        assert!(text.contains("exit code: 130"));
        assert!(text.contains("cleanup ok"));
        assert!(text.contains("often expected when testing cancellation"));
        assert_eq!(
            result
                .structured_content
                .as_ref()
                .and_then(|v| v.get("status"))
                .and_then(|v| v.as_str()),
            Some("interrupted")
        );
        assert_eq!(
            result
                .structured_content
                .as_ref()
                .and_then(|v| v.get("exit_code"))
                .and_then(|v| v.as_i64()),
            Some(130)
        );
    }

    #[test]
    fn sigterm_interrupt_result_labels_signal() {
        let data = serde_json::json!({
            "exit_code": 143,
            "status": "interrupted"
        });
        let result = shell_signal_interrupt_result(143, 10, "", "terminated\n", data);
        let text = text_of(&result);

        assert_eq!(result.is_error, Some(false));
        assert!(text.contains("SIGTERM"));
        assert!(text.contains("exit code: 143"));
    }
}
