pub mod context;
pub mod envelope;
pub mod execute;
pub mod output;
pub mod registry;
pub mod schema;

pub use context::{
    append_hook_context, await_hook_run, clear_hook_context, get_hook_context, set_hook_context,
};
pub use envelope::{
    build_envelope_value, HookEnvelopeParams, HookPlatform, SessionEndReason, SessionStartSource,
};
pub use execute::{
    dispatch_hook_event, matcher_matches, notify_stub_event, run_session_start, spawn_session_start,
};
pub use output::{normalize, parse_hook_stdout, HookDecision, HookOutput};
pub use registry::{
    compute_script_hash, delete_source, get_source, get_user_plugins_directory, list_sources,
    save_source, set_hooks_approved, PluginSourceManifest, PluginSourceRecord,
};
pub use schema::{
    load_hooks_from_root, HookDispatchMode, HookEvent, HookHandler, HookMatcherGroup, HooksConfig,
};
