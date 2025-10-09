/// Message search module providing BM25-based full-text search functionality.
///
/// This module implements:
/// - BM25 search engine for message content
/// - Session-level index management with persistence
/// - Incremental index building and background reindexing
/// - Index metadata tracking in SQLite
pub mod background_worker;
pub mod index_storage;
pub mod message_index;

pub use background_worker::IndexingWorker;
