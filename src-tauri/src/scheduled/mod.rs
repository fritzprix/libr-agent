//! Scheduled task subsystem.
//!
//! Provides a background worker that fires cron-backed tasks and a runner
//! that handles session creation / reuse and message injection.

pub mod runner;
pub mod worker;

pub use worker::SchedulerWorker;

/// Convert a user-supplied cron expression to the 7-field format required by the
/// `cron` crate (sec min hour dom month dow year).
///
/// - 5 fields (min hour dom month dow)  → prepend `0` (sec) and append `*` (year)
/// - 6 fields (sec min hour dom month dow) → append `*` (year)
/// - 7 fields — passed through unchanged (already in expected format)
/// - other field counts — passed through unchanged (will fail at parse time)
pub fn normalize_cron(expr: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    match parts.len() {
        5 => format!("0 {} *", parts.join(" ")),
        6 => format!("{} *", parts.join(" ")),
        _ => expr.to_string(),
    }
}
