use crate::agent::AgentSessionManager;
use crate::server::handlers;
use std::sync::Arc;
use warp::Filter;

pub fn get_routes(
    agent_manager: Arc<AgentSessionManager>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let agent_manager = warp::any().map(move || agent_manager.clone());

    // POST /api/sessions
    let create_session = warp::post()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::end())
        .and(agent_manager.clone())
        .and(warp::body::json())
        .and_then(handlers::create_session);

    // GET /api/sessions/:id
    let get_session = warp::get()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param())
        .and(warp::path::end())
        .and(agent_manager.clone())
        .and_then(handlers::get_session);

    // GET /api/sessions/:id/messages
    let get_messages = warp::get()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param())
        .and(warp::path("messages"))
        .and(warp::path::end())
        .and(warp::query())
        .and(agent_manager.clone())
        .and_then(handlers::get_messages);

    // POST /api/sessions/:id/messages
    let send_message = warp::post()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param())
        .and(warp::path("messages"))
        .and(warp::path::end())
        .and(agent_manager.clone())
        .and(warp::body::json())
        .and_then(handlers::send_message);

    // POST /api/sessions/:id/terminate
    let terminate_session = warp::post()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param())
        .and(warp::path("terminate"))
        .and(warp::path::end())
        .and(agent_manager.clone())
        .and_then(handlers::terminate_session);

    // GET /api/sessions/:id/children
    let get_child_sessions = warp::get()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param())
        .and(warp::path("children"))
        .and(warp::path::end())
        .and_then(handlers::get_child_sessions);

    // GET /api/assistants
    let list_assistants = warp::get()
        .and(warp::path("api"))
        .and(warp::path("assistants"))
        .and(warp::path::end())
        .and_then(handlers::get_assistants);

    // GET /api/health
    let health = warp::get()
        .and(warp::path("api"))
        .and(warp::path("health"))
        .and(warp::path::end())
        .and_then(handlers::health);

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "DELETE"]);

    create_session
        .or(get_session)
        .or(get_messages)
        .or(send_message)
        .or(terminate_session)
        .or(get_child_sessions)
        .or(list_assistants)
        .or(health)
        .with(cors)
}
