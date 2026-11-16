//! Per-process session ownership (#2759).
//!
//! The process that hosts the backend owns sessions for its lifetime using
//! [`HOST_ATTACHMENT_ID`]. There is no cross-process frontend attachment
//! registry (#2854): HTTP callers must not send fabricated attachment ids.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use warp::http::StatusCode;

/// Attachment id used by the backend-hosting process for its own session
/// operations. Always live: the backend dies together with this process.
pub const HOST_ATTACHMENT_ID: &str = "host";

/// Exact refusal wording the GUI shows when entry is refused (#2759).
pub const OPEN_ELSEWHERE_MESSAGE: &str = "is open in another process";

/// session_id -> attachment id of the process that opened it.
static OWNERS: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn owners() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    OWNERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn attachment_is_live(attachment_id: &str) -> bool {
    attachment_id == HOST_ATTACHMENT_ID
}

/// Atomically open `session_id` for `attachment_id` (check-and-set under one
/// lock, so two near-simultaneous opens of the same session produce exactly
/// one winner).
///
/// `Ok(())`: the session is unowned, already owned by this attachment, or was
/// owned by an attachment that is no longer live.
/// `Err`: a different live process owns the session.
pub fn claim(session_id: &str, attachment_id: &str) -> Result<(), String> {
    let mut map = owners();
    if let Some(owner) = map.get(session_id) {
        if owner == attachment_id {
            return Ok(());
        }
        if attachment_is_live(owner) {
            return Err(format!("Session {} {}", session_id, OPEN_ELSEWHERE_MESSAGE));
        }
    }
    map.insert(session_id.to_string(), attachment_id.to_string());
    Ok(())
}

/// Live owner of `session_id`, if any.
pub fn owner(session_id: &str) -> Option<String> {
    let map = owners();
    map.get(session_id)
        .filter(|owner| attachment_is_live(owner))
        .cloned()
}

/// True when a live attachment other than `requester` owns the session.
/// A `None` requester (a caller without an attachment, e.g. an external
/// script) sees any live owner as "open elsewhere".
pub fn is_open_elsewhere(session_id: &str, requester: Option<&str>) -> bool {
    let map = owners();
    match map.get(session_id) {
        Some(owner) => attachment_is_live(owner) && Some(owner.as_str()) != requester,
        None => false,
    }
}

/// Drop the ownership entry of one session. Used when opening a brand-new
/// session fails after its claim, so no entry outlives a session that was
/// never created.
pub fn release_session(session_id: &str) {
    owners().remove(session_id);
}

type HttpRefusal = (StatusCode, String);

const ATTACHMENT_HEADER_REMOVED: &str =
    "x-frontend-attachment is not supported; attachment registry removed";

/// Refuse any HTTP caller that sends an `x-frontend-attachment` header (#2854).
///
/// - No header: external caller; the request continues and claims nothing.
/// - Any header value: `400` — cross-process attachment registration is gone;
///   only the in-process host may own sessions.
pub fn reject_attachment_header(header: Option<&str>) -> Result<(), HttpRefusal> {
    match header {
        None => Ok(()),
        Some(_) => Err((
            StatusCode::BAD_REQUEST,
            ATTACHMENT_HEADER_REMOVED.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use warp::http::StatusCode;

    fn unique_id(tag: &str) -> String {
        format!("{tag}-{}", uuid::Uuid::new_v4())
    }

    #[test]
    fn only_host_attachment_stays_live() {
        assert!(attachment_is_live(HOST_ATTACHMENT_ID));
        assert!(!attachment_is_live(&unique_id("att")));
    }

    #[test]
    fn host_blocks_a_second_non_host_claimant() {
        let session = unique_id("session");
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
        let err = claim(&session, &unique_id("att")).unwrap_err();
        assert!(err.contains(OPEN_ELSEWHERE_MESSAGE));
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
    }

    #[test]
    fn stale_non_host_owner_is_replaced() {
        let session = unique_id("session");
        let stale = unique_id("att");
        assert!(claim(&session, &stale).is_ok());
        assert_eq!(owner(&session), None, "non-host owner is not live");
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
        assert_eq!(owner(&session).as_deref(), Some(HOST_ATTACHMENT_ID));
    }

    #[test]
    fn ownership_survives_switching_sessions() {
        let first = unique_id("session");
        let second = unique_id("session");

        assert!(claim(&first, HOST_ATTACHMENT_ID).is_ok());
        assert!(claim(&second, HOST_ATTACHMENT_ID).is_ok());
        assert_eq!(owner(&first).as_deref(), Some(HOST_ATTACHMENT_ID));
        assert_eq!(owner(&second).as_deref(), Some(HOST_ATTACHMENT_ID));
    }

    #[test]
    fn release_session_drops_one_entry() {
        let kept = unique_id("session");
        let dropped = unique_id("session");
        assert!(claim(&kept, HOST_ATTACHMENT_ID).is_ok());
        assert!(claim(&dropped, HOST_ATTACHMENT_ID).is_ok());

        release_session(&dropped);
        assert_eq!(owner(&dropped), None);
        assert_eq!(owner(&kept).as_deref(), Some(HOST_ATTACHMENT_ID));
    }

    #[test]
    fn any_attachment_header_is_rejected() {
        for header in [HOST_ATTACHMENT_ID, unique_id("att").as_str()] {
            let (status, message) = reject_attachment_header(Some(header)).unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(message.contains("attachment registry removed"), "{message}");
        }
    }

    #[test]
    fn a_header_less_request_neither_claims_nor_refuses() {
        let session = unique_id("session");
        assert!(reject_attachment_header(None).is_ok());
        assert_eq!(owner(&session), None);

        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
        assert!(reject_attachment_header(None).is_ok());
        assert_eq!(owner(&session).as_deref(), Some(HOST_ATTACHMENT_ID));
    }

    #[test]
    fn host_attachment_always_owns_and_never_blocks_itself() {
        let session = unique_id("session");
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
        assert!(!is_open_elsewhere(&session, Some(HOST_ATTACHMENT_ID)));
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
    }

    #[test]
    fn simultaneous_host_opens_all_succeed() {
        let session = unique_id("session");
        let winners = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    if claim(&session, HOST_ATTACHMENT_ID).is_ok() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(winners.load(Ordering::SeqCst), 16);
        assert_eq!(owner(&session).as_deref(), Some(HOST_ATTACHMENT_ID));
    }
}
