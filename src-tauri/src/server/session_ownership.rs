//! Per-process session ownership (#2759).
//!
//! The frontend process that opens a session (the hosting GUI, a GUI attached
//! to another backend, or a TUI) owns it for the **process lifetime**:
//! ownership is released only when that process's backend attachment is gone
//! — a clean detach or a lapsed #2758 heartbeat. There is no idle timeout, no
//! release on session switch, no read-only mirror and no takeover.
//!
//! Ownership validity is *derived* from the attachment registry at query
//! time: an entry whose owning attachment is no longer live is stale and the
//! next claimer takes over, so a crashed frontend releases its sessions
//! without any cleanup pass. The GUI that hosts the backend never appears in
//! the registry (its own process *is* the backend), so it uses the special
//! always-live [`HOST_ATTACHMENT_ID`] for its local operations.

use crate::server::frontend_attachments;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

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
    attachment_id == HOST_ATTACHMENT_ID || frontend_attachments::is_live(attachment_id)
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
        // The owner's attachment is gone (detached or heartbeat lapsed):
        // the process is dead, so this claim takes over.
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

/// Drop ownership entries of a cleanly detached attachment. Crash releases
/// need no call: liveness is derived from the registry, see module docs.
pub fn release_attachment(attachment_id: &str) {
    owners().retain(|_, owner| owner != attachment_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::frontend_attachments::AttachResponse;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use warp::http::StatusCode;

    fn unique_id(tag: &str) -> String {
        format!("{tag}-{}", uuid::Uuid::new_v4())
    }

    /// An attachment id registered as live in the registry. Production
    /// claimers always hold a live attachment; unregistered ids read as
    /// dead (see `dead_owner_is_replaced`).
    fn live_att(tag: &str) -> String {
        let id = unique_id(tag);
        frontend_attachments::register_live(&id);
        id
    }

    #[test]
    fn claim_across_two_attachments_refuses_the_second() {
        let session = unique_id("session");
        let first = live_att("att");
        let second = live_att("att");

        assert!(claim(&session, &first).is_ok());
        let err = claim(&session, &second).unwrap_err();
        assert!(err.contains(&session));
        assert!(err.contains(OPEN_ELSEWHERE_MESSAGE));
        // The first holder keeps ownership and can re-enter freely.
        assert!(claim(&session, &first).is_ok());
        assert_eq!(owner(&session).as_deref(), Some(first.as_str()));
    }

    #[test]
    fn ownership_survives_switching_sessions() {
        // The owner decision (2026-11-15): opening another session does not
        // release anything — a process may own several sessions at once.
        let att = live_att("att");
        let first = unique_id("session");
        let second = unique_id("session");

        assert!(claim(&first, &att).is_ok());
        assert!(claim(&second, &att).is_ok());
        assert_eq!(owner(&first).as_deref(), Some(att.as_str()));
        assert_eq!(owner(&second).as_deref(), Some(att.as_str()));
        assert!(!is_open_elsewhere(&first, Some(&att)));
        assert!(is_open_elsewhere(&first, Some(&unique_id("att"))));
    }

    #[test]
    fn dead_owner_is_replaced() {
        // An attachment id the registry has never seen models a frontend
        // whose heartbeat lapsed (the registry pruned it).
        let session = unique_id("session");
        let dead = unique_id("att");
        let alive = live_att("att");

        assert!(claim(&session, &dead).is_ok());
        assert!(claim(&session, &alive).is_ok());
        assert_eq!(owner(&session).as_deref(), Some(alive.as_str()));
    }

    #[test]
    fn release_attachment_drops_only_its_sessions() {
        let att = live_att("att");
        let other = live_att("att");
        let mine = unique_id("session");
        let theirs = unique_id("session");

        assert!(claim(&mine, &att).is_ok());
        assert!(claim(&theirs, &other).is_ok());
        release_attachment(&att);
        assert_eq!(owner(&mine), None);
        assert_eq!(owner(&theirs).as_deref(), Some(other.as_str()));
    }

    #[test]
    fn host_attachment_always_owns_and_never_blocks_itself() {
        let session = unique_id("session");
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
        assert_eq!(owner(&session).as_deref(), Some(HOST_ATTACHMENT_ID));
        assert!(!is_open_elsewhere(&session, Some(HOST_ATTACHMENT_ID)));
        assert!(is_open_elsewhere(&session, Some("someone-else")));
        assert!(claim(&session, "someone-else").is_err());
        assert!(claim(&session, HOST_ATTACHMENT_ID).is_ok());
    }

    #[test]
    fn simultaneous_opens_have_exactly_one_winner() {
        let session = unique_id("session");
        let winners = AtomicUsize::new(0);
        let atts: Vec<String> = (0..16).map(|_| live_att("att")).collect();
        std::thread::scope(|scope| {
            for att in &atts {
                let session = &session;
                let winners = &winners;
                scope.spawn(move || {
                    if claim(session, att).is_ok() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(winners.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn clean_detach_releases_ownership() {
        // Real registry round-trip: attach over the routes, claim, detach,
        // then a different attachment may take over.
        let api = frontend_attachments::routes();
        let resp = warp::test::request()
            .method("POST")
            .path("/api/frontends")
            .reply(&api)
            .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let attached: AttachResponse = serde_json::from_slice(resp.body()).unwrap();

        let session = unique_id("session");
        assert!(claim(&session, &attached.id).is_ok());
        assert!(claim(&session, &unique_id("att")).is_err());

        let resp = warp::test::request()
            .method("DELETE")
            .path(&format!("/api/frontends/{}", attached.id))
            .reply(&api)
            .await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let taker = live_att("att");
        assert!(claim(&session, &taker).is_ok());
        assert_eq!(owner(&session).as_deref(), Some(taker.as_str()));
    }
}
