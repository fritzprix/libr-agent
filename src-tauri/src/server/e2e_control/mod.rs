//! Loopback-only HTTP control plane for unattended Session API E2E.
//!
//! Compile-time gated behind `#[cfg(debug_assertions)]` by `server/mod.rs`, so
//! release binaries cannot serve these routes at all. At runtime (debug builds)
//! the control plane additionally requires `LIBRAGENT_E2E_CONTROL=1` and an
//! explicit `LIBRAGENT_DATA_DIR`, and every request must carry the per-process
//! capability token in the `X-LibrAgent-E2E-Control` header.

mod handlers;
mod token;

pub use handlers::{capability_guard, e2e_control_routes, E2eControlStatus};
pub use token::{
    capability_token, constant_time_eq, e2e_control_enabled, header_matches,
    init_capability_token, E2E_CONTROL_CAPABILITY_FILE, E2E_CONTROL_HEADER,
};
