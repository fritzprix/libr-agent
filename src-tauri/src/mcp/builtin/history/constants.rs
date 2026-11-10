// Constants for the history module.

/// Default page number for listing sessions and search results.
pub const DEFAULT_LIST_PAGE: u64 = 1;
/// Default page size for listing sessions.
pub const DEFAULT_LIST_PAGE_SIZE: u64 = 20;
/// Default page size for message lists within a session.
pub const DEFAULT_MESSAGE_PAGE_SIZE: u64 = 50;
/// Default page size for search results.
pub const DEFAULT_SEARCH_PAGE_SIZE: u64 = 20;
/// Maximum allowed page size for any list operation.
pub const MAX_PAGE_SIZE: u64 = 100;
/// Maximum number of characters allowed in a single message read.
pub const MAX_MESSAGE_CHARS: usize = 50_000;
/// Default number of characters to return when reading a message.
pub const DEFAULT_MESSAGE_CHARS: usize = 50_000;
/// Default sort order for reading session messages.
pub const DEFAULT_READ_SESSION_ORDER: &str = "desc";
/// Number of characters for content previews/snippets.
pub const PREVIEW_CHARS: usize = 240;
/// Maximum number of messages to scan during a search operation.
pub const SEARCH_SCAN_LIMIT: u64 = 1000;
