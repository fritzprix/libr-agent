//! Hook execution: dispatch entrypoints, handler selection, subprocess and
//! HTTP transport, and the environment a hook process runs with (#3011).
//!
//! Everything the rest of the crate calls is re-exported here, so
//! `hooks::execute::<name>` paths are unchanged by the split.

mod command;
mod dispatch;
mod env;
mod handler;
mod http;
mod matcher;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use dispatch::RecordingHookDispatcher;
pub use dispatch::{
    dispatch_hook_event, dispatch_hook_event_in, dispatch_hook_event_with_sources,
    notify_stub_event, run_session_start, run_session_start_in, run_session_start_with_sources,
    spawn_session_start, DefaultHookDispatcher, HookDispatchResult, HookDispatcher,
};
pub use matcher::matcher_matches;
