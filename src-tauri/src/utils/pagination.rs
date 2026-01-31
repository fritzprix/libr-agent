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
    pub total_pages: u64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

impl<T> Page<T> {
    /// Creates a new Page with calculated navigation flags and total pages
    pub fn new(items: Vec<T>, page: u64, page_size: u64, total_items: u64) -> Self {
        let safe_page_size = if page_size == 0 { 10 } else { page_size };
        let has_next_page = page.saturating_mul(safe_page_size) < total_items;
        let has_previous_page = page > 1;

        // Ceiling division: (total + size - 1) / size
        let total_pages = if total_items == 0 {
            0
        } else {
            (total_items.saturating_add(safe_page_size).saturating_sub(1)) / safe_page_size
        };

        Self {
            items,
            page,
            page_size: safe_page_size,
            total_items,
            total_pages,
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

/// Helper function to perform in-memory pagination on a vector of items.
/// Useful when search/filtering is done in memory (e.g. global search)
/// rather than in the database query.
pub fn paginate_in_memory<T: Clone>(
    all_items: Vec<T>,
    page: u64,
    page_size: u64,
) -> Page<T> {
    let total_items = all_items.len() as u64;
    // Handle 0-based index calculation safely
    let start_idx = (page.saturating_sub(1) as usize).saturating_mul(page_size as usize);
    let end_idx = start_idx.saturating_add(page_size as usize).min(all_items.len());

    let items = if start_idx < all_items.len() {
        all_items[start_idx..end_idx].to_vec()
    } else {
        Vec::new()
    };

    Page::new(items, page, page_size, total_items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paginate_in_memory_basic() {
        let items: Vec<i32> = (1..=25).collect();

        // Page 1, size 10
        let page1 = paginate_in_memory(items.clone(), 1, 10);
        assert_eq!(page1.items.len(), 10);
        assert_eq!(page1.items[0], 1);
        assert_eq!(page1.items[9], 10);
        assert_eq!(page1.total_pages, 3);
        assert!(page1.has_next_page);
        assert!(!page1.has_previous_page);

        // Page 2, size 10
        let page2 = paginate_in_memory(items.clone(), 2, 10);
        assert_eq!(page2.items.len(), 10);
        assert_eq!(page2.items[0], 11);
        assert_eq!(page2.items[9], 20);
        assert!(page2.has_next_page);
        assert!(page2.has_previous_page);

        // Page 3, size 10 (partial)
        let page3 = paginate_in_memory(items.clone(), 3, 10);
        assert_eq!(page3.items.len(), 5);
        assert_eq!(page3.items[0], 21);
        assert_eq!(page3.items[4], 25);
        assert!(!page3.has_next_page);
        assert!(page3.has_previous_page);
    }

    #[test]
    fn test_paginate_in_memory_out_of_bounds() {
        let items: Vec<i32> = vec![1, 2, 3];
        let page = paginate_in_memory(items, 5, 10);

        assert!(page.items.is_empty());
        assert_eq!(page.total_items, 3);
        assert_eq!(page.page, 5);
    }

    #[test]
    fn test_paginate_in_memory_empty() {
        let items: Vec<i32> = Vec::new();
        let page = paginate_in_memory(items, 1, 10);

        assert!(page.items.is_empty());
        assert_eq!(page.total_items, 0);
        assert_eq!(page.total_pages, 0);
    }
}
