//! Session lifecycle management: resume, config update, and status transitions.
//!
//! `resume` loads a persisted session back into the active map, `config_update` applies
//! configuration changes to a live session, and `status` owns every status transition
//! (plain, with-reason, with-dispatcher) plus the queued-to-busy promotion and the
//! wake-fence refusal helpers.

mod config_update;
mod resume;
mod status;

pub use config_update::update_session_config;
pub use resume::resume_session;
pub use status::{
    end_fence_refused_wake, end_fence_refused_wake_with_dispatcher, promote_queued_session_to_busy,
    promote_queued_session_to_busy_with_dispatcher, record_session_error_with_dispatcher,
    update_session_status, update_session_status_with_dispatcher,
    update_session_status_with_reason,
};
