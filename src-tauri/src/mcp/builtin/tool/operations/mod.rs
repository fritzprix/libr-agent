//! MCP tool server mutation and discovery handlers.
//!
//! Public API is re-exported here so `tool/mod.rs` can keep calling `operations::*`.

mod list;
mod mutations;
mod persistence;
mod verify;

pub use list::list_tools;
pub use mutations::{delete_server, register_server, update_server};
pub use verify::verify_server;
