//! Permissions for requests that arrive through a managed session's own endpoint (#2343,
//! contract section 3).
//!
//! The endpoint's router is built with a fixed `origin` (the session that owns the socket).
//! A request may only reach that session and its direct children; the routes that act on
//! other sessions, on settings, on tool servers or on human approvals are refused. The main
//! TCP port has no origin and is not affected.

use warp::filters::path::FullPath;
use warp::http::Method;
use warp::reject::Reject;
use warp::{Filter, Rejection};

use crate::repositories::session_repository::SessionRepository;

/// Rejection for a request the endpoint's origin session may not make.
#[derive(Debug)]
pub struct EndpointForbidden;

impl Reject for EndpointForbidden {}

/// What a request needs before it may proceed.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Allow,
    Deny,
    /// The request acts on `session` (a session ref from the path).
    Owned {
        session: String,
        /// Only a direct child qualifies; the origin session itself does not (delete, terminate).
        direct_child_only: bool,
    },
}

fn owned(session: &str, direct_child_only: bool) -> Decision {
    Decision::Owned {
        session: session.to_string(),
        direct_child_only,
    }
}

/// Classifies a request by method and path. Unknown paths are denied.
fn classify(method: &Method, path: &str) -> Decision {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let read = method == Method::GET;
    match segments.as_slice() {
        ["api", "health"] | ["api", "assistants"] | ["api", "assistants", _] if read => {
            Decision::Allow
        }
        // Settings may be read; any change is refused.
        ["api", "settings", ..] if read => Decision::Allow,
        ["api", "sessions"] if method == Method::POST => Decision::Allow,
        // A wake is authorized by its token alone, not by ownership.
        ["api", "sessions", _, "wake", _] if method == Method::POST => Decision::Allow,
        // Approvals are for a human on the main port.
        ["api", "sessions", _, "channel", "permission"] => Decision::Deny,
        ["api", "sessions", id, "terminate"] => owned(id, true),
        ["api", "sessions", id] if method == Method::DELETE => owned(id, true),
        ["api", "sessions", id, ..] => owned(id, false),
        // `/api/channel` (auto-selects a session), `/mcp*`, anything unknown.
        _ => Decision::Deny,
    }
}

/// `target_parent` is the parent of `target` when `target` is not the origin session.
fn is_permitted(
    origin: &str,
    target: &str,
    direct_child_only: bool,
    target_parent: Option<&str>,
) -> bool {
    if target == origin {
        return !direct_child_only;
    }
    target_parent == Some(origin)
}

async fn authorize(origin: &str, method: &Method, path: &str) -> bool {
    match classify(method, path) {
        Decision::Allow => true,
        Decision::Deny => false,
        Decision::Owned {
            session,
            direct_child_only,
        } => {
            let Ok(target) =
                crate::server::handlers::helpers::resolve_http_session_ref(&session).await
            else {
                return false;
            };
            let parent = if target == origin {
                None
            } else {
                match crate::state::get_session_repository()
                    .get_session(&target)
                    .await
                {
                    Ok(Some(metadata)) => metadata.parent_session_id,
                    _ => return false,
                }
            };
            is_permitted(origin, &target, direct_child_only, parent.as_deref())
        }
    }
}

/// Refuses (403) requests the endpoint's origin session may not make. A no-op without an
/// origin, which is the main TCP port.
pub fn endpoint_guard(
    origin: Option<String>,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::method()
        .and(warp::path::full())
        .and_then(move |method: Method, path: FullPath| {
            let origin = origin.clone();
            async move {
                let allowed = match origin.as_deref() {
                    None => true,
                    Some(origin) => authorize(origin, &method, path.as_str()).await,
                };
                if allowed {
                    Ok::<(), Rejection>(())
                } else {
                    Err(warp::reject::custom(EndpointForbidden))
                }
            }
        })
        .untuple_one()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::access_guard::handle_rejection;

    fn decision(method: Method, path: &str) -> Decision {
        classify(&method, path)
    }

    #[test]
    fn session_routes_need_ownership() {
        for (method, path) in [
            (Method::GET, "/api/sessions/s2"),
            (Method::GET, "/api/sessions/s2/compact-context"),
            (Method::GET, "/api/sessions/s2/messages"),
            (Method::POST, "/api/sessions/s2/messages"),
            (Method::POST, "/api/sessions/s2/channel"),
            (Method::POST, "/api/sessions/s2/resume"),
            (Method::GET, "/api/sessions/s2/children"),
        ] {
            assert_eq!(
                decision(method.clone(), path),
                owned("s2", false),
                "{method} {path}"
            );
        }
    }

    #[test]
    fn delete_and_terminate_are_for_direct_children_only() {
        assert_eq!(
            decision(Method::DELETE, "/api/sessions/s2"),
            owned("s2", true)
        );
        assert_eq!(
            decision(Method::POST, "/api/sessions/s2/terminate"),
            owned("s2", true)
        );
    }

    #[test]
    fn wake_is_token_only_and_approvals_are_always_refused() {
        assert_eq!(
            decision(Method::POST, "/api/sessions/s2/wake/tok"),
            Decision::Allow
        );
        assert_eq!(
            decision(Method::POST, "/api/sessions/s1/channel/permission"),
            Decision::Deny
        );
    }

    #[test]
    fn global_routes_follow_the_contract_table() {
        for (method, path) in [
            (Method::GET, "/api/health"),
            (Method::GET, "/api/assistants"),
            (Method::GET, "/api/assistants/a1"),
            (Method::GET, "/api/settings/preferredModel"),
            (Method::GET, "/api/settings/complexityModelMapping"),
            (Method::POST, "/api/sessions"),
        ] {
            assert_eq!(
                decision(method.clone(), path),
                Decision::Allow,
                "{method} {path}"
            );
        }
        for (method, path) in [
            (Method::PUT, "/api/settings/complexityModelMapping"),
            (Method::POST, "/api/channel"),
            (Method::POST, "/mcp"),
            (Method::POST, "/mcp/control"),
            (Method::POST, "/mcp/s1"),
            (Method::GET, "/mcp/s1"),
            (Method::GET, "/api/unknown"),
            (Method::GET, "/"),
        ] {
            assert_eq!(
                decision(method.clone(), path),
                Decision::Deny,
                "{method} {path}"
            );
        }
    }

    #[test]
    fn ownership_is_the_origin_or_a_direct_child() {
        // The origin session itself.
        assert!(is_permitted("s1", "s1", false, None));
        assert!(
            !is_permitted("s1", "s1", true, None),
            "self delete/terminate"
        );
        // A direct child.
        assert!(is_permitted("s1", "s2", false, Some("s1")));
        assert!(is_permitted("s1", "s2", true, Some("s1")));
        // A grandchild, a sibling's child, an unrelated or parentless session.
        assert!(!is_permitted("s1", "s3", false, Some("s2")));
        assert!(!is_permitted("s1", "s4", false, Some("other")));
        assert!(!is_permitted("s1", "s5", false, None));
    }

    fn routes(
        origin: Option<&str>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
        endpoint_guard(origin.map(str::to_string))
            .map(|| "ok")
            .recover(handle_rejection)
    }

    async fn status(origin: Option<&str>, method: &str, path: &str) -> u16 {
        warp::test::request()
            .method(method)
            .path(path)
            .reply(&routes(origin))
            .await
            .status()
            .as_u16()
    }

    #[tokio::test]
    async fn guard_refuses_the_denied_routes_with_403_for_an_endpoint() {
        for (method, path) in [
            ("POST", "/mcp"),
            ("POST", "/mcp/control"),
            ("POST", "/api/channel"),
            ("PUT", "/api/settings/complexityModelMapping"),
            ("POST", "/api/sessions/s1/channel/permission"),
            ("GET", "/api/unknown"),
        ] {
            assert_eq!(
                status(Some("s1"), method, path).await,
                403,
                "{method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn guard_lets_the_open_routes_through_for_an_endpoint() {
        for (method, path) in [
            ("GET", "/api/health"),
            ("GET", "/api/assistants"),
            ("POST", "/api/sessions"),
            ("POST", "/api/sessions/s2/wake/tok"),
        ] {
            assert_eq!(
                status(Some("s1"), method, path).await,
                200,
                "{method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn guard_is_a_no_op_on_the_main_port() {
        for (method, path) in [
            ("POST", "/mcp"),
            ("POST", "/api/channel"),
            ("PUT", "/api/settings/complexityModelMapping"),
            ("POST", "/api/sessions/s1/channel/permission"),
        ] {
            assert_eq!(status(None, method, path).await, 200, "{method} {path}");
        }
    }
}
