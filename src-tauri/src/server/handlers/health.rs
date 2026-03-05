use warp::{http::StatusCode, Rejection, Reply};

use super::types::HealthResponse;

pub async fn health() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_status(
        warp::reply::json(&HealthResponse {
            status: "ok".to_string(),
            service: "libr-agent-session-api".to_string(),
        }),
        StatusCode::OK,
    ))
}
