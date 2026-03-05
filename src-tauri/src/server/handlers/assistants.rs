use warp::{http::StatusCode, Rejection, Reply};

use super::types::ErrorResponse;

pub async fn get_assistant(assistant_id: String) -> Result<impl Reply, Rejection> {
    use crate::repositories::assistant_repository::AssistantRepository;

    let repo = crate::state::get_assistant_repository();
    match repo.get_assistant(&assistant_id).await {
        Ok(Some(assistant)) => Ok(warp::reply::with_status(
            warp::reply::json(&assistant),
            StatusCode::OK,
        )),
        Ok(None) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Assistant not found: {}", assistant_id),
            }),
            StatusCode::NOT_FOUND,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch assistant: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn get_assistants() -> Result<impl Reply, Rejection> {
    use crate::repositories::assistant_repository::AssistantRepository;

    let repo = crate::state::get_assistant_repository();
    match repo.list_assistants().await {
        Ok(assistants) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "assistants": assistants })),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch assistants: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
