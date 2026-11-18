//! Session endpoints of the Session API.
//!
//! Split from the original single-file module (#2888):
//! - `crud`: create / get / open / list / status-snapshot / child-session handlers
//! - `lifecycle`: patch, resume, terminate, and delete handlers
//! - `tests`: the route-level unit tests
//!
//! The re-exports below keep the pre-split `crate::server::handlers::sessions::*`
//! API unchanged, so `routes.rs` and `events.rs` (`super::sessions::session_status_snapshot`)
//! need no changes.

mod crud;
mod lifecycle;

#[cfg(test)]
mod tests;

pub(super) use crud::session_status_snapshot;
pub use crud::{create_session, get_child_sessions, get_session, list_sessions, open_session};
pub use lifecycle::{delete_session, patch_session, resume_session_workflow, terminate_session};
