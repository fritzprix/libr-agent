//! Localhost Session API request guards (Origin / Host).
//!
//! CLI clients (Harbor, libr-delegate, bench scripts) send no `Origin` and use
//! `Host: localhost:<port>` / `127.0.0.1:<port>`. Browser cross-site calls send
//! a non-empty `Origin` and are rejected. When the server is bound to loopback
//! only, non-loopback `Host` values are rejected (DNS-rebinding mitigation).
//! When `expose` is true, Host is not restricted to loopback (LAN clients use
//! the machine IP); Origin rejection still applies.

use warp::http::HeaderMap;
use warp::hyper::StatusCode;
use warp::reject::Reject;
use warp::{Filter, Rejection, Reply};

#[derive(Debug)]
pub struct AccessDenied {
    pub reason: &'static str,
}

impl Reject for AccessDenied {}

/// Returns `Ok(())` if the request may proceed.
pub fn check_access(headers: &HeaderMap, expose: bool) -> Result<(), AccessDenied> {
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let origin = origin.trim();
        if !origin.is_empty() {
            return Err(AccessDenied {
                reason: "cross-origin requests are not allowed",
            });
        }
    }

    if !expose {
        match headers.get("host").and_then(|v| v.to_str().ok()) {
            Some(host) if is_loopback_host(host) => {}
            Some(_) => {
                return Err(AccessDenied {
                    reason: "host must be localhost, 127.0.0.1, or [::1]",
                });
            }
            None => {
                return Err(AccessDenied {
                    reason: "missing host header",
                });
            }
        }
    }

    Ok(())
}

/// `Host` values accepted when bound to loopback only.
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() {
        return false;
    }

    // Bracketed IPv6: [::1] or [::1]:port
    if let Some(rest) = host.strip_prefix("[::1]") {
        return rest.is_empty() || rest.starts_with(':');
    }

    let hostname = match host.rsplit_once(':') {
        Some((name, port)) if !name.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => name,
        _ => host,
    };

    hostname.eq_ignore_ascii_case("localhost") || hostname == "127.0.0.1"
}

/// Warp filter: reject with 403 when Origin/Host checks fail.
pub fn access_guard(expose: bool) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::headers_cloned()
        .and_then(move |headers: HeaderMap| async move {
            match check_access(&headers, expose) {
                Ok(()) => Ok(()),
                Err(denied) => Err(warp::reject::custom(denied)),
            }
        })
        .untuple_one()
}

/// Warp filter: reject with 403 when the remote peer address is None or not a loopback address.
pub fn loopback_peer_guard() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::addr::remote()
        .and_then(|addr: Option<std::net::SocketAddr>| async move {
            match addr {
                Some(a) if a.ip().is_loopback() => Ok(()),
                _ => Err(warp::reject::custom(AccessDenied {
                    reason: "pairing is only permitted from loopback",
                })),
            }
        })
        .untuple_one()
}

pub async fn handle_rejection(err: Rejection) -> Result<warp::reply::Response, Rejection> {
    if let Some(denied) = err.find::<AccessDenied>() {
        let body = serde_json::json!({ "error": denied.reason });
        return Ok(
            warp::reply::with_status(warp::reply::json(&body), StatusCode::FORBIDDEN)
                .into_response(),
        );
    }
    if err.find::<super::api_key::KeyRequired>().is_some() {
        let body = serde_json::json!({
            "error": "the Session API needs a key while Docker sessions exist; start LibrAgent with LIBRAGENT_SESSION_API_KEY"
        });
        return Ok(warp::reply::with_header(
            warp::reply::with_status(warp::reply::json(&body), StatusCode::UNAUTHORIZED),
            "www-authenticate",
            "Bearer",
        )
        .into_response());
    }
    if let Some(unauthorized) = err.find::<super::api_key::Unauthorized>() {
        let body = serde_json::json!({ "error": "missing or invalid API key" });
        return Ok(warp::reply::with_header(
            warp::reply::with_status(warp::reply::json(&body), StatusCode::UNAUTHORIZED),
            "www-authenticate",
            unauthorized.www_authenticate(),
        )
        .into_response());
    }
    Err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback_peer_guard_accepts_loopback_and_rejects_non_loopback_or_missing() {
        let filter = loopback_peer_guard().map(|| "ok").recover(handle_rejection);

        // IPv4 loopback
        let res = warp::test::request()
            .remote_addr("127.0.0.1:54321".parse().unwrap())
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);

        // IPv6 loopback
        let res = warp::test::request()
            .remote_addr("[::1]:54321".parse().unwrap())
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);

        // LAN/remote address: rejected with 403
        let res = warp::test::request()
            .remote_addr("192.168.1.100:54321".parse().unwrap())
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 403);
        assert!(
            String::from_utf8_lossy(res.body()).contains("pairing is only permitted from loopback")
        );

        // Missing remote address (None): rejected with 403
        let res = warp::test::request().reply(&filter).await;
        assert_eq!(res.status(), 403);
        assert!(
            String::from_utf8_lossy(res.body()).contains("pairing is only permitted from loopback")
        );
    }
}
