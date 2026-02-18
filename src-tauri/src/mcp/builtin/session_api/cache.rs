use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock as TokioRwLock;
use serde_json::Value;

use super::types::{MessageFetchCacheEntry, MessageSummaryOptions};

static MESSAGE_FETCH_CACHE: OnceLock<TokioRwLock<HashMap<String, MessageFetchCacheEntry>>> =
    OnceLock::new();

fn message_fetch_cache() -> &'static TokioRwLock<HashMap<String, MessageFetchCacheEntry>> {
    MESSAGE_FETCH_CACHE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

pub fn compute_messages_digest(messages: &[Value]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    for message in messages {
        message
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .hash(&mut hasher);

        message
            .get("updatedAt")
            .or_else(|| message.get("updated_at"))
            .or_else(|| message.get("createdAt"))
            .or_else(|| message.get("created_at"))
            .map(|v| v.to_string())
            .unwrap_or_else(|| "0".to_string())
            .hash(&mut hasher);

        message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .hash(&mut hasher);
    }

    hasher.finish()
}

pub fn message_cache_key(
    caller_session_id: Option<&str>,
    target_session_id: &str,
    limit: Option<u64>,
) -> String {
    let caller = caller_session_id.unwrap_or("no-caller");
    let limit_text = limit
        .map(|value| value.to_string())
        .unwrap_or_else(|| "default".to_string());
    format!("{}::{}::{}", caller, target_session_id, limit_text)
}

pub async fn unchanged_messages_notice(
    messages: &[Value],
    caller_session_id: Option<&str>,
    target_session_id: &str,
    limit: Option<u64>,
) -> Option<String> {
    let digest = compute_messages_digest(messages);
    let key = message_cache_key(caller_session_id, target_session_id, limit);

    let mut cache = message_fetch_cache().write().await;
    if cache.len() > 2048 {
        cache.clear();
    }

    let now = Instant::now();
    let entry = cache.entry(key).or_insert(MessageFetchCacheEntry {
        digest,
        last_checked_at: now,
        rapid_call_count: 0,
        cooldown_until: None,
    });

    let previous = entry.digest;
    entry.digest = digest;
    entry.last_checked_at = now;

    if previous == digest {
        Some(format!(
            "Fetched {} messages for session {}\n\nNo new message changes since last fetch. Skip repeated ingestion and continue with current context.",
            messages.len(),
            target_session_id
        ))
    } else {
        None
    }
}

pub async fn min_interval_notice(
    caller_session_id: Option<&str>,
    target_session_id: &str,
    limit: Option<u64>,
    options: MessageSummaryOptions,
) -> Option<String> {
    if options.min_interval_seconds == 0 && options.forced_rest_seconds == 0 {
        return None;
    }

    let key = message_cache_key(caller_session_id, target_session_id, limit);
    let mut cache = message_fetch_cache().write().await;

    let now = Instant::now();
    let entry = cache.entry(key).or_insert(MessageFetchCacheEntry {
        digest: 0,
        last_checked_at: now,
        rapid_call_count: 0,
        cooldown_until: None,
    });

    if let Some(cooldown_until) = entry.cooldown_until {
        if now < cooldown_until {
            let wait_seconds = cooldown_until.duration_since(now).as_secs().max(1);
            entry.last_checked_at = now;
            return Some(format!(
                "Forced cooldown active for session {}.\n\nPlease wait {}s before calling getMessages again.",
                target_session_id, wait_seconds
            ));
        }
        entry.cooldown_until = None;
        entry.rapid_call_count = 0;
    }

    let elapsed = now.duration_since(entry.last_checked_at).as_secs();
    if options.min_interval_seconds > 0 && elapsed < options.min_interval_seconds {
        entry.rapid_call_count = entry.rapid_call_count.saturating_add(1);
        entry.last_checked_at = now;

        if options.forced_rest_seconds > 0
            && entry.rapid_call_count >= options.rapid_call_threshold
        {
            let cooldown_until = now + Duration::from_secs(options.forced_rest_seconds);
            entry.cooldown_until = Some(cooldown_until);
            entry.rapid_call_count = 0;
            return Some(format!(
                "Too many rapid getMessages calls detected for session {}.\n\nForced cooldown started: {}s. Let the model rest before polling again.",
                target_session_id, options.forced_rest_seconds
            ));
        }

        let wait_seconds = options.min_interval_seconds - elapsed;
        return Some(format!(
            "Skipped getMessages for session {} to preserve context budget.\n\nPlease wait {}s before polling again (minIntervalSeconds={}; rapidCount={}/{}).",
            target_session_id,
            wait_seconds,
            options.min_interval_seconds,
            entry.rapid_call_count,
            options.rapid_call_threshold
        ));
    }

    entry.last_checked_at = now;
    entry.rapid_call_count = 0;
    None
}
