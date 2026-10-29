use warp::hyper::StatusCode;
use warp::{Rejection, Reply};

use crate::server::pairing::{self, PairingRequestBody};

/// `POST /api/pairings` handler.
pub async fn handle_pairing_request(body: PairingRequestBody) -> Result<impl Reply, Rejection> {
    match pairing::handle_pairing_request(body).await {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        )),
        Err((status, message)) => {
            let error_body = serde_json::json!({ "error": message });
            Ok(warp::reply::with_status(
                warp::reply::json(&error_body),
                status,
            ))
        }
    }
}
