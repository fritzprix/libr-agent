//! Loopback-only HTTP control plane for unattended Session API E2E (debug builds only).
//!
//! Enabled only when `debug_assertions` are on **and** `LIBRAGENT_E2E_CONTROL=1`.
//! Requires a per-process capability token (`X-LibrAgent-E2E-Control`).

mod handlers;
mod token;

pub use handlers::{
    e2e_control_routes, get_interactions_snapshot, submit_interactive_shell,
    submit_tool_approval, E2eControlStatus,
};
pub use token::{
    capability_token, e2e_control_enabled, init_capability_token, read_capability_from_env,
    E2E_CONTROL_HEADER,
};
