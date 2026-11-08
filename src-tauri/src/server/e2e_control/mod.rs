//! Loopback-only HTTP control plane for unattended Session API E2E.
//!
//! Compile-time gated behind `#[cfg(debug_assertions)]` by `server/mod.rs`, so
//! release binaries cannot serve these routes at all. At runtime (debug builds)
//! the control plane additionally requires `LIBRAGENT_E2E_CONTROL=1` and an
//! explicit `LIBRAGENT_DATA_DIR`, and every request must carry the per-process
//! capability token in the `X-LibrAgent-E2E-Control` header.

mod handlers;
mod token;

use std::convert::Infallible;
use std::sync::Arc;

use warp::{Filter, Rejection, Reply};

use crate::agent::AgentSessionManager;

pub use handlers::{
    capability_guard, e2e_control_routes, e2e_control_routes_with_capability,
    plan_user_prompt_action, tool_approval_error_status, E2eControlStatus, PlannedPromptAction,
    UserPromptBody,
};
pub use token::{
    capability_token, constant_time_eq, e2e_control_enabled, header_matches, init_capability_token,
    E2E_CONTROL_CAPABILITY_FILE, E2E_CONTROL_HEADER,
};

/// The production guard chain for the E2E control plane: loopback Host/Origin
/// guard, then loopback peer guard, then the capability-guarded routes.
///
/// `server/routes.rs` and the integration tests both build this exact chain
/// from this single function, so removing either guard from the wiring makes
/// the tests fail instead of silently widening exposure.
pub fn e2e_control_guarded_routes(
    agent_manager: impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible> + Clone + Send,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    e2e_control_guarded_routes_with_capability(
        capability_token().map(str::to_string),
        agent_manager,
    )
}

/// Same chain with the capability passed in explicitly (tests must not touch
/// the process-global token).
pub fn e2e_control_guarded_routes_with_capability(
    capability: Option<String>,
    agent_manager: impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible> + Clone + Send,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    crate::server::access_guard::access_guard(false)
        .and(crate::server::access_guard::loopback_peer_guard())
        .and(handlers::e2e_control_routes_with_capability(
            capability,
            agent_manager,
        ))
}
