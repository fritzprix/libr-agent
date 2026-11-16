use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::time::Duration;

use super::schema::DEFAULT_HOOK_TIMEOUT_SECS;

/// Stored hook context for one session.
struct HookEntry {
    text: String,
    /// Set while a SessionStart re-run is in flight: the next non-empty
    /// append replaces `text` instead of appending to it (#2812 G3.2). The
    /// previous text survives until the new run actually produces output, so
    /// a slow, failing, or exit-2 re-run can never erase the bootstrap.
    replace_next: bool,
}

static HOOK_CONTEXT: std::sync::OnceLock<RwLock<HashMap<String, HookEntry>>> =
    std::sync::OnceLock::new();

/// Sessions with a SessionStart hook run in flight (#2812 G3.1). The marker
/// is registered synchronously before the run is spawned, so the first LLM
/// request of a new session can wait for the hook instead of building a
/// prompt without `## Plugin Hook Context`.
static HOOK_INFLIGHT: std::sync::OnceLock<RwLock<HashSet<String>>> = std::sync::OnceLock::new();

fn store() -> &'static RwLock<HashMap<String, HookEntry>> {
    HOOK_CONTEXT.get_or_init(|| RwLock::new(HashMap::new()))
}

fn inflight() -> &'static RwLock<HashSet<String>> {
    HOOK_INFLIGHT.get_or_init(|| RwLock::new(HashSet::new()))
}

pub fn set_hook_context(session_id: &str, text: String) {
    if let Ok(mut map) = store().write() {
        map.insert(
            session_id.to_string(),
            HookEntry {
                text,
                replace_next: false,
            },
        );
    }
}

pub fn get_hook_context(session_id: &str) -> Option<String> {
    store()
        .read()
        .ok()
        .and_then(|map| map.get(session_id).map(|e| e.text.clone()))
}

pub fn clear_hook_context(session_id: &str) {
    if let Ok(mut map) = store().write() {
        map.remove(session_id);
    }
}

/// Drop every session's hook-context slot (#2812 criterion 4). Called when
/// a plugin source is uninstalled: the removed source's bootstrap must not
/// keep living in any open session's prompt. Remaining sources re-inject on
/// their next SessionStart firing (resume / clear / compact).
pub fn clear_all_hook_contexts() {
    if let Ok(mut map) = store().write() {
        map.clear();
    }
}

pub fn append_hook_context(session_id: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    if let Ok(mut map) = store().write() {
        match map.get_mut(session_id) {
            Some(entry) => {
                if std::mem::take(&mut entry.replace_next) {
                    // First output of a re-run replaces the previous text.
                    entry.text = text.to_string();
                } else {
                    if !entry.text.is_empty() {
                        entry.text.push_str("\n\n");
                    }
                    entry.text.push_str(text);
                }
            }
            None => {
                map.insert(
                    session_id.to_string(),
                    HookEntry {
                        text: text.to_string(),
                        replace_next: false,
                    },
                );
            }
        }
    }
}

/// Arm one-shot replace semantics for the next append of this session
/// (#2812 G3.2). Call [`finish_hook_replacement`] when the run ends: if no
/// output arrived, the disarmed flag keeps any previous text untouched.
pub fn begin_hook_replacement(session_id: &str) {
    if let Ok(mut map) = store().write() {
        match map.get_mut(session_id) {
            Some(entry) => entry.replace_next = true,
            None => {
                map.insert(
                    session_id.to_string(),
                    HookEntry {
                        text: String::new(),
                        replace_next: true,
                    },
                );
            }
        }
    }
}

/// Disarm replacement after a run ends without output; the previous context
/// (if any) stays as it was (#2812 G3.2).
pub fn finish_hook_replacement(session_id: &str) {
    if let Ok(mut map) = store().write() {
        if let Some(entry) = map.get_mut(session_id) {
            entry.replace_next = false;
            if entry.text.is_empty() {
                // A placeholder armed before the first-ever run produced
                // nothing; do not leave an empty entry behind.
                map.remove(session_id);
            }
        }
    }
}

/// Register an in-flight SessionStart hook run. Every caller must release it
/// exactly once via [`end_hook_run`] (the hook runner wraps it in a guard so
/// timeouts and panics still release).
pub fn begin_hook_run(session_id: &str) {
    if let Ok(mut set) = inflight().write() {
        set.insert(session_id.to_string());
    }
}

pub fn end_hook_run(session_id: &str) {
    if let Ok(mut set) = inflight().write() {
        set.remove(session_id);
    }
}

pub fn hook_run_pending(session_id: &str) -> bool {
    inflight()
        .read()
        .map(|set| set.contains(session_id))
        .unwrap_or(false)
}

/// Poll until the in-flight marker for `session_id` is released. Returns
/// `true` when released, `false` on timeout.
pub async fn wait_for_hook_run(session_id: &str, cap: Duration) -> bool {
    const POLL: Duration = Duration::from_millis(10);
    let deadline = std::time::Instant::now() + cap;
    while hook_run_pending(session_id) {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(POLL).await;
    }
    true
}

/// Prompt-build gate (#2812 G3.1 / G4.1): when a SessionStart hook is still
/// running and no non-empty context is stored yet, the first LLM request
/// waits for it — bounded by the default hook timeout. A blank placeholder
/// armed before the first run produces output is not context, so it must
/// not skip the wait. A hook that exits 2, times out, or fails releases the
/// marker, so the wait never outlives the run. Sessions that already have
/// non-empty context (e.g. a compact re-run keeping the previous bootstrap)
/// never wait.
pub async fn await_hook_run(session_id: &str) {
    let has_context = get_hook_context(session_id).is_some_and(|text| !text.trim().is_empty());
    if has_context || !hook_run_pending(session_id) {
        return;
    }
    // The run itself is bounded by each handler's timeout (default
    // DEFAULT_HOOK_TIMEOUT_SECS); a small slack absorbs process teardown.
    let cap = Duration::from_secs(DEFAULT_HOOK_TIMEOUT_SECS + 5);
    if !wait_for_hook_run(session_id, cap).await {
        log::warn!("hook run for session {session_id} outlived the prompt wait cap");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_get_hook_context() {
        let sid = "test-session-hook-context-1";
        clear_hook_context(sid);
        append_hook_context(sid, "first");
        append_hook_context(sid, "second");
        assert_eq!(get_hook_context(sid).as_deref(), Some("first\n\nsecond"));
        set_hook_context(sid, "replaced".to_string());
        assert_eq!(get_hook_context(sid).as_deref(), Some("replaced"));
        clear_hook_context(sid);
        assert!(get_hook_context(sid).is_none());
    }

    #[test]
    fn replacement_keeps_previous_text_until_first_output() {
        let sid = "test-session-hook-replace-1";
        clear_hook_context(sid);
        append_hook_context(sid, "old bootstrap");
        // A re-run arms replacement; failing to produce output must not erase.
        begin_hook_replacement(sid);
        finish_hook_replacement(sid);
        assert_eq!(get_hook_context(sid).as_deref(), Some("old bootstrap"));
        // Success of the re-run replaces the old text outright.
        begin_hook_replacement(sid);
        append_hook_context(sid, "new bootstrap");
        finish_hook_replacement(sid);
        assert_eq!(get_hook_context(sid).as_deref(), Some("new bootstrap"));
        // A second handler in the same run appends to the fresh text.
        append_hook_context(sid, "extra");
        assert_eq!(
            get_hook_context(sid).as_deref(),
            Some("new bootstrap\n\nextra")
        );
        clear_hook_context(sid);
    }

    #[test]
    fn replacement_before_first_run_leaves_nothing_behind() {
        let sid = "test-session-hook-replace-2";
        clear_hook_context(sid);
        begin_hook_replacement(sid);
        finish_hook_replacement(sid);
        assert!(get_hook_context(sid).is_none());
    }

    #[test]
    fn inflight_markers_are_per_session() {
        let a = "test-session-hook-inflight-a";
        let b = "test-session-hook-inflight-b";
        begin_hook_run(a);
        assert!(hook_run_pending(a));
        assert!(!hook_run_pending(b));
        end_hook_run(a);
        assert!(!hook_run_pending(a));
    }

    #[tokio::test]
    async fn wait_for_hook_run_releases_when_marker_ends() {
        let sid = "test-session-hook-inflight-wait";
        begin_hook_run(sid);
        let waiter = {
            let sid = sid.to_string();
            tokio::spawn(async move { wait_for_hook_run(&sid, Duration::from_secs(5)).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished());
        end_hook_run(sid);
        assert!(waiter.await.unwrap());
    }

    #[test]
    fn clear_all_hook_contexts_empties_every_slot() {
        // Criterion 4 / uninstall purge: every session slot must drop.
        let a = "test-session-hook-clear-all-a";
        let b = "test-session-hook-clear-all-b";
        clear_hook_context(a);
        clear_hook_context(b);
        set_hook_context(a, "bootstrap-a".to_string());
        set_hook_context(b, "bootstrap-b".to_string());
        assert!(get_hook_context(a).is_some());
        assert!(get_hook_context(b).is_some());
        clear_all_hook_contexts();
        assert!(get_hook_context(a).is_none());
        assert!(get_hook_context(b).is_none());
    }
}
