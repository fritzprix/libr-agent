use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use tokio::sync::oneshot;

/// A call that is running for a session. `generation` tells a finished call
/// apart from the one that replaced it.
pub(super) struct InFlight {
    generation: u64,
    handle: tokio::task::AbortHandle,
}

/// In-flight calls by session, so a cancel can stop the HTTP request.
pub(super) static IN_FLIGHT: LazyLock<DashMap<String, InFlight>> = LazyLock::new(DashMap::new);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

tokio::task_local! {
    /// The generation of the tracked call this task is running, so a request
    /// started from inside the running call (the #2967 resample after invalid
    /// tool-call arguments) can tell that the entry it is replacing is its own
    /// task and must not abort it.
    static CALL_GENERATION: u64;
}

/// Stop this session's in-flight call, if any. Called for every
/// `llm:completion-cancel`, so a workflow cancel or a recovery retry does not
/// leave an HTTP request running.
pub fn cancel(session_id: &str) {
    if let Some((_, call)) = IN_FLIGHT.remove(session_id) {
        call.handle.abort();
        log::info!("Rust LLM executor: cancelled the call for session {session_id}");
    }
}

/// Run `work` as the one in-flight call of `session_id`.
///
/// The call is registered before `work` starts, so it can neither finish ahead
/// of its own entry nor leave a stale one. A call this replaces is aborted: the
/// new request supersedes it and settles the session. Each call removes only
/// its own entry when it ends.
///
/// Exception (#2967): when the running call requests the replacement itself —
/// the resample after invalid tool-call arguments does exactly that — the
/// previous entry is this very task. Aborting it would kill the work that must
/// process the replacement's reply, so the call is left to finish and its
/// already-replaced entry is dropped by the generation check when it ends.
pub(super) fn spawn_tracked<F>(session_id: &str, work: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    let (registered, wait_registered) = oneshot::channel::<()>();
    let key = session_id.to_string();
    let task = tokio::spawn({
        let key = key.clone();
        async move {
            // Closed channel means the entry was dropped; run anyway.
            let _ = wait_registered.await;
            CALL_GENERATION.scope(generation, work).await;
            IN_FLIGHT.remove_if(&key, |_, call| call.generation == generation);
        }
    });
    let call = InFlight {
        generation,
        handle: task.abort_handle(),
    };
    if let Some(previous) = IN_FLIGHT.insert(key, call) {
        if previous.generation
            == CALL_GENERATION
                .try_with(|running| *running)
                .unwrap_or(u64::MAX)
        {
            log::warn!(
                "Rust LLM executor: the running call for session {session_id} requested a replacement of itself; keeping it alive to settle the session"
            );
        } else {
            previous.handle.abort();
            log::warn!(
                "Rust LLM executor: a new call for session {session_id} replaced one still running"
            );
        }
    }
    let _ = registered.send(());
}
