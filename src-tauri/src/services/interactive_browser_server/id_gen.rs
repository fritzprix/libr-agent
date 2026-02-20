use chrono::Utc;
use std::sync::atomic::{AtomicU16, Ordering};

/// Global counter for generating unique session IDs within the same millisecond
static SESSION_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Generates a unique session ID based on timestamp and a global counter.
///
/// Format: timestamp (5 hex) + counter (3 hex) = 8 chars.
/// This supports ~4096 sessions per millisecond without collision.
pub fn generate_session_id() -> String {
    let now = Utc::now();
    let timestamp = (now.timestamp_millis() % 100000) as u32; // Last 5 digits
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst) % 4096;
    format!("{:05X}{:03X}", timestamp, counter)
}
