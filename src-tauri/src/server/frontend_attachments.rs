//! Frontend attachment count for the shared backend (#2758).
//!
//! Every frontend that attaches to this backend over the Session API holds one
//! attachment and refreshes it with a heartbeat. The backend's own GUI window
//! is not counted here: while it is open the process stays up anyway. When it
//! closes, the process keeps running (window hidden) until no attachment is
//! left, then exits. A frontend that stops sending heartbeats (crash, kill) is
//! dropped after [`HEARTBEAT_TIMEOUT`], so it cannot keep the backend alive.
//!
//! Exit at zero does not wait for running turns: the session stays `busy` in
//! the database and the next backend start recovers it to `paused` (#2700).

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

/// How often an attached frontend sends a heartbeat.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// An attachment without a heartbeat for this long is dropped.
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);

/// Attachment ids and the time of their last attach or heartbeat.
#[derive(Debug, Default)]
pub struct AttachmentRegistry {
    last_seen: HashMap<String, Instant>,
}

impl AttachmentRegistry {
    pub fn attach(&mut self, now: Instant) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.last_seen.insert(id.clone(), now);
        id
    }

    /// Refresh an attachment. False when the id is unknown (never attached,
    /// detached, or already dropped as stale); the frontend then attaches again.
    pub fn heartbeat(&mut self, id: &str, now: Instant) -> bool {
        self.prune(now);
        match self.last_seen.get_mut(id) {
            Some(seen) => {
                *seen = now;
                true
            }
            None => false,
        }
    }

    pub fn detach(&mut self, id: &str) {
        self.last_seen.remove(id);
    }

    /// Attachments that sent a heartbeat within [`HEARTBEAT_TIMEOUT`].
    pub fn live_count(&mut self, now: Instant) -> usize {
        self.prune(now);
        self.last_seen.len()
    }

    fn prune(&mut self, now: Instant) {
        self.last_seen
            .retain(|_, seen| now.saturating_duration_since(*seen) < HEARTBEAT_TIMEOUT);
    }
}

static REGISTRY: Lazy<Mutex<AttachmentRegistry>> =
    Lazy::new(|| Mutex::new(AttachmentRegistry::default()));

fn with_registry<T>(f: impl FnOnce(&mut AttachmentRegistry) -> T) -> T {
    let mut registry = REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut registry)
}

/// Live attachments of other frontends to this backend.
pub fn live_count() -> usize {
    with_registry(|registry| registry.live_count(Instant::now()))
}

/// Response of `POST /api/frontends`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachResponse {
    pub id: String,
}

async fn attach_handler() -> Result<impl Reply, Rejection> {
    let id = with_registry(|registry| registry.attach(Instant::now()));
    Ok(warp::reply::with_status(
        warp::reply::json(&AttachResponse { id }),
        StatusCode::CREATED,
    ))
}

async fn heartbeat_handler(id: String) -> Result<impl Reply, Rejection> {
    let known = with_registry(|registry| registry.heartbeat(&id, Instant::now()));
    Ok(if known {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    })
}

async fn detach_handler(id: String) -> Result<impl Reply, Rejection> {
    with_registry(|registry| registry.detach(&id));
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/frontends`, `POST /api/frontends/:id/heartbeat`,
/// `DELETE /api/frontends/:id`.
pub fn routes() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let attach = warp::post()
        .and(warp::path!("api" / "frontends"))
        .and_then(attach_handler);
    let heartbeat = warp::post()
        .and(warp::path!("api" / "frontends" / String / "heartbeat"))
        .and_then(heartbeat_handler);
    let detach = warp::delete()
        .and(warp::path!("api" / "frontends" / String))
        .and_then(detach_handler);
    attach.or(heartbeat).or(detach)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_and_detach_change_the_count() {
        let now = Instant::now();
        let mut registry = AttachmentRegistry::default();
        let first = registry.attach(now);
        let second = registry.attach(now);
        assert_eq!(registry.live_count(now), 2);

        registry.detach(&first);
        assert_eq!(registry.live_count(now), 1);
        registry.detach(&second);
        assert_eq!(registry.live_count(now), 0);
    }

    #[test]
    fn missed_heartbeat_drops_the_frontend() {
        let start = Instant::now();
        let mut registry = AttachmentRegistry::default();
        let alive = registry.attach(start);
        let crashed = registry.attach(start);

        let later = start + HEARTBEAT_TIMEOUT - Duration::from_secs(1);
        assert!(registry.heartbeat(&alive, later));
        assert_eq!(registry.live_count(later), 2);

        let after_timeout = start + HEARTBEAT_TIMEOUT + Duration::from_secs(1);
        assert_eq!(registry.live_count(after_timeout), 1);
        assert!(!registry.heartbeat(&crashed, after_timeout));
        assert!(registry.heartbeat(&alive, after_timeout));
    }

    #[test]
    fn heartbeat_for_unknown_id_is_rejected() {
        let mut registry = AttachmentRegistry::default();
        assert!(!registry.heartbeat("never-attached", Instant::now()));
    }

    #[tokio::test]
    async fn routes_attach_heartbeat_and_detach() {
        let api = routes();
        let resp = warp::test::request()
            .method("POST")
            .path("/api/frontends")
            .reply(&api)
            .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let attached: AttachResponse = serde_json::from_slice(resp.body()).unwrap();

        let heartbeat_path = format!("/api/frontends/{}/heartbeat", attached.id);
        let resp = warp::test::request()
            .method("POST")
            .path(&heartbeat_path)
            .reply(&api)
            .await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let resp = warp::test::request()
            .method("DELETE")
            .path(&format!("/api/frontends/{}", attached.id))
            .reply(&api)
            .await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let resp = warp::test::request()
            .method("POST")
            .path(&heartbeat_path)
            .reply(&api)
            .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
