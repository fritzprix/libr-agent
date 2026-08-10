//! Message persistence repository (SQLite via SeaORM).
//!
//! Module layout:
//! - [`types`] — public cursors/pages and [`MessageRepository`] trait
//! - [`persist`] — DB row ↔ domain mapping and error-column UI envelope
//! - [`index_meta`] — search index metadata for sessions
//! - [`sqlite`] — [`SqliteMessageRepository`] query orchestration

mod index_meta;
mod persist;
mod sqlite;
mod types;

#[cfg(test)]
mod tests;

pub use sqlite::SqliteMessageRepository;
pub use types::{MessagePaginationCursor, MessageRepository, MessageSlicePage};
