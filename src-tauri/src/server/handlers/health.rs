use warp::{http::StatusCode, Rejection, Reply};

use crate::server::readiness;

use super::types::HealthResponse;

/// `GET /api/health` — liveness plus a readiness signal.
///
/// `status: "ok"` means the HTTP server is up. `ready` becomes true once
/// startup session recovery has ended, so callers can tell "process up" apart
/// from "ready to run a session". `recovery` says how it ended; `failed`
/// means sessions may still be marked busy from a previous run.
pub async fn health() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_status(
        warp::reply::json(&HealthResponse {
            status: "ok".to_string(),
            service: "libr-agent-session-api".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ready: readiness::is_ready(),
            recovery: readiness::recovery_state(),
        }),
        StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::Reply;

    #[tokio::test]
    async fn health_reports_liveness_and_readiness() {
        let reply = health().await.expect("health should succeed");
        let response = reply.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .expect("body readable");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json body");
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "libr-agent-session-api");
        assert_eq!(
            json["ready"],
            readiness::is_ready(),
            "ready must mirror the process readiness flag"
        );
        let recovery = json["recovery"].as_str().expect("recovery is a string");
        assert!(
            ["pending", "ok", "failed", "skipped"].contains(&recovery),
            "{recovery}"
        );
        assert_eq!(json["ready"], recovery != "pending");
    }
}
