/// Default limit for list and search results
pub const DEFAULT_LIMIT: u64 = 20;
/// Maximum limit for list and search results
pub const MAX_LIMIT: u64 = 100;
/// Default snippet length for non-FTS search results (characters)
pub const DEFAULT_SNIPPET_LENGTH: usize = 150;
/// FTS snippet length (tokens)
pub const FTS_SNIPPET_LENGTH: usize = 20;
/// Name of the FTS virtual table
pub const TABLE_FTS: &str = "knowledge_fts";

/// Parse specific "tags" string from database model into a vector of strings
pub fn parse_db_tags(tags_str: Option<&String>) -> Vec<String> {
    if let Some(s) = tags_str {
        serde_json::from_str(s).unwrap_or_default()
    } else {
        Vec::new()
    }
}
