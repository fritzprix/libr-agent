//! Background tool discovery for MCP servers: stdio and HTTP loaders, the entry
//! points that spawn them, and the types they share.

mod dispatcher;
mod helpers;
mod http_loader;
mod stdio_loader;
#[cfg(test)]
mod tests;
mod types;

pub(super) use dispatcher::{spawn_background_tool_loading, spawn_retry_failed_servers};
pub(super) use types::{BackgroundDiscoveryPlan, RetryFailedServersParams};
