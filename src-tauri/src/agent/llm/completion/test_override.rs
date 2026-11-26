//! Process-wide override for [`super::request_llm_completion_with_recovery`].
//!
//! Integration tests that restart a turn after settlement must not drive the
//! production completion path through a mistyped `AppHandle` (the
//! `MockRuntime` → Wry cast at the harness). Installing an override lets those
//! tests inject a completion request that drains pending work through an
//! `AgentEventDispatcher` without touching `AppHandle` (#3004 / #3028).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

type OverrideFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
type OverrideFn = Arc<dyn Fn(String) -> OverrideFuture + Send + Sync>;

static OVERRIDE: Mutex<Option<OverrideFn>> = Mutex::new(None);

/// Keeps a [`install`] setting in force; dropping it restores the production path.
pub struct CompletionOverrideGuard(());

impl Drop for CompletionOverrideGuard {
    fn drop(&mut self) {
        *OVERRIDE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

/// Replace the production completion request until the guard is dropped.
///
/// The override is process-wide: callers must serialize their use of it (for
/// example with the settlement harness lock) so concurrent callers do not race.
#[must_use = "the setting ends when the guard is dropped"]
pub fn install<F, Fut>(f: F) -> CompletionOverrideGuard
where
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    let boxed: OverrideFn = Arc::new(move |session_id| Box::pin(f(session_id)));
    *OVERRIDE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(boxed);
    CompletionOverrideGuard(())
}

/// Run the installed override for `session_id`, if any.
pub(super) async fn try_run(session_id: String) -> Option<Result<(), String>> {
    let override_fn = OVERRIDE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    match override_fn {
        Some(f) => Some(f(session_id).await),
        None => None,
    }
}
