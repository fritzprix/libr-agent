//! Scheduled task subsystem.
//!
//! Provides a background worker that fires cron-backed tasks and a runner
//! that handles session creation / reuse and message injection.

pub mod runner;
pub mod worker;

pub use worker::SchedulerWorker;

pub const SCHEDULE_TIMEZONE_UTC: &str = "utc";
pub const SCHEDULE_TIMEZONE_LOCAL: &str = "local";

pub const TASK_CATEGORY_GLOBAL: &str = "GLOBAL";
pub const TASK_CATEGORY_SESSION: &str = "SESSION";

pub fn is_session_task(category: &str) -> bool {
    category == TASK_CATEGORY_SESSION
}

pub fn is_one_shot_task(cron_expression: &Option<String>) -> bool {
    cron_expression.is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleTimezone {
    Utc,
    Local,
}

impl ScheduleTimezone {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            SCHEDULE_TIMEZONE_UTC => Ok(Self::Utc),
            SCHEDULE_TIMEZONE_LOCAL => Ok(Self::Local),
            _ => Err(format!("Invalid schedule timezone '{value}'")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utc => SCHEDULE_TIMEZONE_UTC,
            Self::Local => SCHEDULE_TIMEZONE_LOCAL,
        }
    }
}

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
        5 => {
            let mut normalized = parts
                .iter()
                .map(|part| (*part).to_string())
                .collect::<Vec<_>>();
            normalized[4] = remap_weekday_for_cron(&normalized[4]);
            format!("0 {} *", normalized.join(" "))
        }
        6 => {
            let mut normalized = parts
                .iter()
                .map(|part| (*part).to_string())
                .collect::<Vec<_>>();
            normalized[5] = remap_weekday_for_cron(&normalized[5]);
            format!("{} *", normalized.join(" "))
        }
        _ => expr.to_string(),
    }
}

fn remap_weekday_for_cron(day: &str) -> String {
    day.split(',')
        .map(remap_weekday_segment_for_cron)
        .collect::<Vec<_>>()
        .join(",")
}

fn remap_weekday_segment_for_cron(segment: &str) -> String {
    if let Some((base, step)) = segment.split_once('/') {
        return format!("{}/{}", remap_weekday_base_for_cron(base), step);
    }

    remap_weekday_base_for_cron(segment)
}

fn remap_weekday_base_for_cron(base: &str) -> String {
    if let Some((start, end)) = base.split_once('-') {
        return format!(
            "{}-{}",
            remap_weekday_value_for_cron(start),
            remap_weekday_value_for_cron(end)
        );
    }

    remap_weekday_value_for_cron(base)
}

fn remap_weekday_value_for_cron(value: &str) -> String {
    match value.parse::<u8>() {
        Ok(0 | 7) => "1".to_string(),
        Ok(day @ 1..=6) => (day + 1).to_string(),
        _ => value.to_string(),
    }
}
