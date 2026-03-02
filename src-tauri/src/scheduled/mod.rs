//! Scheduled task subsystem.
//!
//! Provides a background worker that fires cron-backed tasks and a runner
//! that handles session creation / reuse and message injection.

pub mod runner;
pub mod worker;

pub use worker::SchedulerWorker;

/// Convert a standard 5-field cron expression (min hour dom month dow)
/// to the 7-field format required by the `cron` crate (sec min hour dom month dow year).
pub fn normalize_cron(expr: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() == 5 {
        // Prepend seconds=0 and append year=*
        format!("0 {} *", parts.join(" "))
    } else {
        expr.to_string()
    }
}
