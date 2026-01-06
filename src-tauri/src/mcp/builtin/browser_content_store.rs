use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiktoken_rs::cl100k_base;

#[derive(Clone, Debug)]
pub struct ContentPage {
    pub content: String,
    pub page_number: usize,
    pub total_pages: usize,
}

#[derive(Clone, Debug)]
struct ContentSession {
    pages: Vec<String>,
    timestamp: u64,
}

/// In-memory content store for browser extracted content
#[derive(Clone, Debug)]
pub struct BrowserContentStore {
    store: Arc<DashMap<String, ContentSession>>,
}

impl BrowserContentStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
        }
    }

    /// Save content with token-based pagination
    pub fn save_content(
        &self,
        session_id: &str,
        content: String,
        target_tokens_per_page: usize,
        auto_merge: bool,
    ) -> (usize, String, Option<String>, bool) {
        let pages = Self::paginate_by_tokens(&content, target_tokens_per_page);
        let total_pages = pages.len();
        let first_page = pages.first().cloned().unwrap_or_default();

        // Auto-merge logic: merge if ≤2 pages OR total content < 5000 tokens
        let total_tokens = Self::count_tokens(&content);
        let should_auto_merge = auto_merge && (total_pages <= 2 || total_tokens < 5000);
        let merged_content = if should_auto_merge {
            Some(content.clone())
        } else {
            None
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        let session = ContentSession { pages, timestamp };

        self.store.insert(session_id.to_string(), session);

        (total_pages, first_page, merged_content, should_auto_merge)
    }

    /// Get a specific page of content (1-based index)
    pub fn get_page(&self, session_id: &str, page: usize) -> Option<ContentPage> {
        let session = self.store.get(session_id)?;
        let page_index = page.saturating_sub(1);

        if page_index >= session.pages.len() {
            return None;
        }

        Some(ContentPage {
            content: session.pages[page_index].clone(),
            page_number: page,
            total_pages: session.pages.len(),
        })
    }

    /// Check if content exists for session
    pub fn has_content(&self, session_id: &str) -> bool {
        self.store.contains_key(session_id)
    }

    /// Clean up old sessions (called periodically)
    pub fn cleanup_old_sessions(&self, max_age_secs: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        self.store
            .retain(|_, session| now.saturating_sub(session.timestamp) < max_age_secs);
    }

    /// Count tokens in text using cl100k_base tokenizer (GPT-4, GPT-3.5-turbo)
    fn count_tokens(text: &str) -> usize {
        match cl100k_base() {
            Ok(bpe) => bpe.encode_with_special_tokens(text).len(),
            Err(_) => {
                // Fallback: approximate 1 token ≈ 3 chars for mixed content
                text.chars().count() / 3
            }
        }
    }

    /// Paginate content by token count with line-based chunking
    fn paginate_by_tokens(content: &str, target_tokens: usize) -> Vec<String> {
        let bpe = match cl100k_base() {
            Ok(bpe) => bpe,
            Err(_) => {
                // Fallback to character-based pagination
                return Self::paginate_content_fallback(content, target_tokens * 3);
            }
        };

        let mut pages = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut current_page = String::new();
        let mut current_tokens = 0;

        for (i, line) in lines.iter().enumerate() {
            let is_last_line = i == lines.len() - 1;
            let line_with_newline = if is_last_line {
                line.to_string()
            } else {
                format!("{}\n", line)
            };

            let line_tokens = bpe.encode_with_special_tokens(&line_with_newline).len();

            // If current page + new line fits, add it
            if current_tokens + line_tokens <= target_tokens {
                current_page.push_str(&line_with_newline);
                current_tokens += line_tokens;
            } else {
                // Doesn't fit
                if !current_page.is_empty() {
                    // Push current page and start new one
                    pages.push(current_page.clone());
                    current_page = line_with_newline;
                    current_tokens = line_tokens;
                } else {
                    // Line is too large, push it anyway (overflow)
                    pages.push(line_with_newline);
                    current_page.clear();
                    current_tokens = 0;
                }
            }
        }

        if !current_page.is_empty() {
            pages.push(current_page);
        }

        if pages.is_empty() {
            pages.push(String::new());
        }

        pages
    }

    /// Fallback character-based pagination (if tokenizer fails)
    fn paginate_content_fallback(content: &str, page_size: usize) -> Vec<String> {
        let mut pages = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut current_page = String::new();

        for (i, line) in lines.iter().enumerate() {
            let is_last_line = i == lines.len() - 1;
            let line_with_newline = if is_last_line {
                line.to_string()
            } else {
                format!("{}\n", line)
            };

            // If current page + new line fits, add it
            if current_page.len() + line_with_newline.len() <= page_size {
                current_page.push_str(&line_with_newline);
            } else {
                // Doesn't fit
                if !current_page.is_empty() {
                    // Push current page and start new one
                    pages.push(current_page.clone());
                    current_page = line_with_newline;
                } else {
                    // Line is too large, push it anyway (overflow)
                    pages.push(line_with_newline);
                    current_page.clear();
                }
            }
        }

        if !current_page.is_empty() {
            pages.push(current_page);
        }

        if pages.is_empty() {
            pages.push(String::new());
        }

        pages
    }
}

impl Default for BrowserContentStore {
    fn default() -> Self {
        Self::new()
    }
}
