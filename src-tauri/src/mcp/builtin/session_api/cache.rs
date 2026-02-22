use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock as TokioRwLock;
use tokio::time::sleep;

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
        // Remove oldest half instead of wholesale clear to avoid burst-poll spikes
        let mut keys_by_age: Vec<(String, Instant)> = cache
            .iter()
            .map(|(k, v)| (k.clone(), v.last_checked_at))
            .collect();
        keys_by_age.sort_by_key(|(_, t)| *t);
        for (k, _) in keys_by_age.into_iter().take(1024) {
            cache.remove(&k);
        }
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

    // Track whether this is a brand-new cache entry. First-time callers should
    // never be penalised — the interval is only meaningful *between* repeat calls.
    let is_new_entry = !cache.contains_key(&key);
    let entry = cache.entry(key).or_insert(MessageFetchCacheEntry {
        digest: 0,
        last_checked_at: now,
        rapid_call_count: 0,
        cooldown_until: None,
    });

    // Determine the sleep duration (if any) while holding the lock, then drop the
    // lock before actually sleeping — holding a write-lock across an await point
    // would starve every other cache reader/writer for the entire wait period.
    //
    // Returns Some(hint) when rapid-poll threshold was hit so the caller can
    // prepend guidance (e.g. "use awaitAgent") to the actual fetch result.
    let sleep_duration: Option<Duration>;
    // Whether the forced-cooldown threshold was just triggered.
    let forced_cooldown_hit: bool;

    if is_new_entry {
        // First-ever call for this session-pair: never rate-limit; just record
        // the timestamp so subsequent rapid calls can be detected.
        sleep_duration = None;
        forced_cooldown_hit = false;
    } else if let Some(cooldown_until) = entry.cooldown_until {
        if now < cooldown_until {
            let remaining = cooldown_until.duration_since(now);
            entry.last_checked_at = now;
            sleep_duration = Some(remaining);
            forced_cooldown_hit = true; // still inside a previously-triggered cooldown
        } else {
            entry.cooldown_until = None;
            entry.rapid_call_count = 0;
            sleep_duration = None;
            forced_cooldown_hit = false;
        }
    } else {
        let elapsed = now.duration_since(entry.last_checked_at).as_secs();
        if options.min_interval_seconds > 0 && elapsed < options.min_interval_seconds {
            entry.rapid_call_count = entry.rapid_call_count.saturating_add(1);
            entry.last_checked_at = now;

            if options.forced_rest_seconds > 0
                && entry.rapid_call_count >= options.rapid_call_threshold
            {
                let forced_duration = Duration::from_secs(options.forced_rest_seconds);
                entry.cooldown_until = Some(now + forced_duration);
                entry.rapid_call_count = 0;
                sleep_duration = Some(forced_duration);
                forced_cooldown_hit = true;
            } else {
                let wait_secs = options.min_interval_seconds - elapsed;
                sleep_duration = Some(Duration::from_secs(wait_secs));
                forced_cooldown_hit = false;
            }
        } else {
            entry.last_checked_at = now;
            entry.rapid_call_count = 0;
            sleep_duration = None;
            forced_cooldown_hit = false;
        }
    }

    // Drop the write lock BEFORE sleeping so other tasks aren't starved.
    drop(cache);

    if let Some(duration) = sleep_duration {
        sleep(duration).await;
    }

    // Return a hint only when rapid-polling was severe enough to trigger the
    // forced cooldown — nudge the agent toward awaitAgent for future waits.
    if forced_cooldown_hit {
        Some(
            "[Hint] Rapid getAgentLog polling detected. \
            If you are waiting for a session to finish, use awaitAgent instead — \
            it blocks efficiently until the session reaches a terminal state \
            without consuming extra context or triggering rate limits."
                .to_string(),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `MessageSummaryOptions` tailored for rate-limit unit tests.
    /// `min_interval_seconds=5`, `forced_rest_seconds=20`, `rapid_call_threshold=threshold`
    fn test_options(threshold: u32) -> MessageSummaryOptions {
        MessageSummaryOptions {
            summary_only: true,
            include_raw_preview: false,
            preview_limit: 3,
            skip_if_unchanged: true,
            min_interval_seconds: 5,
            forced_rest_seconds: 20,
            rapid_call_threshold: threshold,
        }
    }

    // -----------------------------------------------------------------------
    // Regression: first call on a new session-pair must NEVER be rate-limited.
    // The trace in .trace.json showed the first getAgentLog was returned as
    // "Skipped" because the old code compared now == last_checked_at (just set
    // by or_insert), yielding elapsed = 0 < min_interval → immediate skip.
    // -----------------------------------------------------------------------

    /// First call for a brand-new (caller, session) pair: returns None instantly,
    /// no sleep, no hint.
    #[tokio::test(start_paused = true)]
    async fn first_call_passes_through_immediately() {
        let result = min_interval_notice(
            Some("caller-first-call-test"),
            "session-first-call-test",
            None,
            test_options(3),
        )
        .await;

        assert!(result.is_none(), "first call must not be rate-limited");
    }

    // -----------------------------------------------------------------------
    // A rapid second call within min_interval sleeps the remainder but returns
    // None (below forced-cooldown threshold → no awaitAgent hint yet).
    // -----------------------------------------------------------------------

    /// Rapid second call: blocks (tokio clock paused so instant) but returns
    /// None because threshold has not been reached.
    #[tokio::test(start_paused = true)]
    async fn rapid_second_call_sleeps_but_returns_no_hint() {
        let opts = test_options(3); // threshold=3, so 1 rapid call ≠ cooldown

        // First call — seeds the cache entry.
        let _ = min_interval_notice(
            Some("caller-rapid-second"),
            "session-rapid-second",
            None,
            opts,
        )
        .await;

        // Second call immediately after (elapsed ≈ 0 s < min_interval=5 s).
        // Tokio clock is paused so sleep(5s) returns instantly.
        let result = min_interval_notice(
            Some("caller-rapid-second"),
            "session-rapid-second",
            None,
            opts,
        )
        .await;

        assert!(
            result.is_none(),
            "below threshold: should sleep but NOT return the awaitAgent hint"
        );
    }

    // -----------------------------------------------------------------------
    // Hitting the rapid-call threshold returns the awaitAgent hint so the agent
    // can switch strategy without experiencing repeated failures first.
    // -----------------------------------------------------------------------

    /// After `threshold` rapid calls the next one returns Some(hint).
    #[tokio::test(start_paused = true)]
    async fn forced_cooldown_threshold_returns_awaitagent_hint() {
        // threshold=2: call 1 seeds (None), call 2 rapid count=1 (None),
        //              call 3 rapid count=2 >= 2 → forced cooldown → Some(hint)
        let opts = test_options(2);
        let caller = "caller-cooldown-hint";
        let session = "session-cooldown-hint";

        for i in 0..2 {
            let res = min_interval_notice(Some(caller), session, None, opts).await;
            assert!(res.is_none(), "call {i} before threshold must not carry hint");
        }

        let hint = min_interval_notice(Some(caller), session, None, opts).await;
        assert!(
            hint.is_some(),
            "call at threshold must return the awaitAgent hint"
        );
    }

    /// The hint text explicitly names `awaitAgent` so the agent can act on it.
    #[tokio::test(start_paused = true)]
    async fn hint_text_references_awaitagent() {
        let opts = test_options(2);
        let caller = "caller-hint-text";
        let session = "session-hint-text";

        // 3 calls: seed + 2 rapid → threshold triggered on call 3.
        let mut last_hint: Option<String> = None;
        for _ in 0..3 {
            last_hint = min_interval_notice(Some(caller), session, None, opts).await;
        }

        let hint = last_hint.expect("threshold call must return a hint");
        assert!(
            hint.contains("awaitAgent"),
            "hint must reference awaitAgent; got: {hint}"
        );
    }
}
