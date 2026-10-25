use crate::models::chat::Message;

/// Maximum number of messages to keep in memory cache (sliding window)
pub const MAX_CACHED_MESSAGES: usize = 1000;

/// Soft ceiling before eviction runs. Allows brief growth past
/// [`MAX_CACHED_MESSAGES`] so single appends do not pay an O(N) front-shift
/// on every push; when crossed, drain oldest down to the hard max in one pass.
pub const MESSAGE_CACHE_SOFT_LIMIT: usize = MAX_CACHED_MESSAGES + 50;

/// Enforce the in-memory message cache sliding window.
///
/// When `messages.len()` exceeds [`MESSAGE_CACHE_SOFT_LIMIT`], drops the oldest
/// excess in a single `drain` so the cache is exactly [`MAX_CACHED_MESSAGES`]
/// long (newest suffix preserved). No-op at or below the soft limit.
pub fn enforce_cached_message_limit(messages: &mut Vec<Message>) {
    if messages.len() <= MESSAGE_CACHE_SOFT_LIMIT {
        return;
    }
    let excess = messages.len() - MAX_CACHED_MESSAGES;
    let last_evicted_id = messages.get(excess - 1).map(|m| m.id.clone());
    messages.drain(..excess);
    if let Some(id) = last_evicted_id {
        log::debug!("Sliding window drained {excess} message(s); last evicted id={id}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_message(id: &str) -> Message {
        let mut msg = Message::new_user_message("s1".to_string(), id.to_string(), None, None);
        msg.id = id.to_string();
        msg
    }

    #[test]
    fn enforce_cached_message_limit_noop_at_or_below_soft_limit() {
        let mut messages: Vec<Message> = (0..MESSAGE_CACHE_SOFT_LIMIT)
            .map(|i| stub_message(&format!("m{i}")))
            .collect();
        let before_len = messages.len();
        let first_id = messages.first().map(|m| m.id.clone());

        enforce_cached_message_limit(&mut messages);

        assert_eq!(messages.len(), before_len);
        assert_eq!(messages.first().map(|m| m.id.as_str()), first_id.as_deref());
    }

    #[test]
    fn enforce_cached_message_limit_drains_to_hard_max_preserving_newest() {
        let total = MESSAGE_CACHE_SOFT_LIMIT + 10;
        let mut messages: Vec<Message> =
            (0..total).map(|i| stub_message(&format!("m{i}"))).collect();

        enforce_cached_message_limit(&mut messages);

        assert_eq!(messages.len(), MAX_CACHED_MESSAGES);
        let expected_first = format!("m{}", total - MAX_CACHED_MESSAGES);
        let expected_last = format!("m{}", total - 1);
        assert_eq!(
            messages.first().map(|m| m.id.as_str()),
            Some(expected_first.as_str())
        );
        assert_eq!(
            messages.last().map(|m| m.id.as_str()),
            Some(expected_last.as_str())
        );
    }
}
