pub mod async_exec;
pub mod handlers;
pub mod isolated;
pub mod persistent;

pub(super) fn format_duration_ms(duration_ms: u64) -> String {
    if duration_ms == 0 {
        "< 1ms".to_string()
    } else {
        format!("{}ms", duration_ms)
    }
}
