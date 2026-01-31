use serde::{Deserialize, Serialize};

/// Generic pagination wrapper for query results.
/// This type is shared across all paginated responses in the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u64,
    pub page_size: u64,
    pub total_items: u64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

impl<T> Page<T> {
    /// Creates a new Page with calculated navigation flags
    pub fn new(items: Vec<T>, page: u64, page_size: u64, total_items: u64) -> Self {
        let has_next_page = page.saturating_mul(page_size) < total_items;
        let has_previous_page = page > 1;
        Self {
            items,
            page,
            page_size,
            total_items,
            has_next_page,
            has_previous_page,
        }
    }
}

/// Pagination parameters for queries
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationParams {
    pub page: u64,
    pub page_size: u64,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 10,
        }
    }
}
