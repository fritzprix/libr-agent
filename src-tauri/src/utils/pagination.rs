use serde::{Deserialize, Serialize};

/// Generic pagination wrapper for query results.
/// This type is shared across all paginated responses in the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

impl<T> Page<T> {
    /// Create a new Page instance with calculated navigation flags and total pages
    pub fn new(items: Vec<T>, page: usize, page_size: usize, total_items: usize) -> Self {
        let has_next_page = (page * page_size) < total_items;
        let has_previous_page = page > 1;
        let total_pages = if page_size > 0 {
            (total_items + page_size - 1) / page_size
        } else {
            1
        };

        Self {
            items,
            page,
            page_size,
            total_items,
            total_pages,
            has_next_page,
            has_previous_page,
        }
    }

    /// Transform the items in the page using a mapping function
    pub fn map<U, F>(self, f: F) -> Page<U>
    where
        F: FnMut(T) -> U,
    {
        Page {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            page_size: self.page_size,
            total_items: self.total_items,
            total_pages: self.total_pages,
            has_next_page: self.has_next_page,
            has_previous_page: self.has_previous_page,
        }
    }
}

/// Common pagination parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PaginationParams {
    pub page: usize,
    pub page_size: usize,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 10,
        }
    }
}

impl PaginationParams {
    /// Create new pagination parameters, normalizing invalid values.
    ///
    /// - If `page` is 0, it is treated as page 1.
    /// - If `page_size` is 0, it is treated as 1 to avoid invalid pagination.
    pub fn new(page: usize, page_size: usize) -> Self {
        let normalized_page = if page == 0 { 1 } else { page };
        let normalized_page_size = if page_size == 0 { 1 } else { page_size };

        Self {
            page: normalized_page,
            page_size: normalized_page_size,
        }
    }

    /// Get offset for database queries (u64 for SeaORM compatibility)
    pub fn offset(&self) -> u64 {
        let page_u64 = self.page.saturating_sub(1) as u64;
        let page_size_u64 = self.page_size as u64;
        page_u64.saturating_mul(page_size_u64)
    }

    /// Get limit for database queries (u64 for SeaORM compatibility)
    pub fn limit(&self) -> u64 {
        self.page_size as u64
    }
}
