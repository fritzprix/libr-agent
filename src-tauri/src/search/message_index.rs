/// BM25-based message search engine implementation.
///
/// Provides full-text search over message content with session-level indexing,
/// incremental updates, and configurable index size limits.
use bm25::{Embedder, EmbedderBuilder, Language, Scorer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Search result for message queries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub message_id: String,
    pub session_id: String,
    pub score: f32,
    pub snippet: Option<String>,
    pub created_at: i64,
}

/// Message document for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDocument {
    pub id: String,
    pub session_id: String,
    pub content: String,
    pub created_at: i64,
}

/// BM25 Message Search Engine
///
/// Maintains an in-memory BM25 index for fast full-text search.
/// Supports incremental updates and persistence to disk.
pub struct MessageSearchEngine {
    /// Session ID this engine is indexing
    session_id: String,
    /// Documents indexed by message ID
    documents: HashMap<String, MessageDocument>,
    /// BM25 embedder for query and document embedding
    embedder: Embedder,
    /// BM25 scorer for relevance ranking
    scorer: Scorer<String>,
    /// Maximum number of documents to keep in hot index
    max_docs: usize,
}

// Manual Debug implementation since Scorer doesn't implement Debug
impl std::fmt::Debug for MessageSearchEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageSearchEngine")
            .field("session_id", &self.session_id)
            .field("doc_count", &self.documents.len())
            .field("max_docs", &self.max_docs)
            .finish()
    }
}

// Manual Clone implementation since Embedder and Scorer don't implement Clone
impl Clone for MessageSearchEngine {
    fn clone(&self) -> Self {
        let mut cloned = Self::new(self.session_id.clone(), self.max_docs);
        let docs: Vec<MessageDocument> = self.documents.values().cloned().collect();
        // Rebuild the index with the cloned documents
        cloned.add_documents(docs).expect("Failed to clone index");
        cloned
    }
}

impl MessageSearchEngine {
    /// Creates a new search engine for a session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to index
    /// * `max_docs` - Maximum documents to keep in the hot index (0 = unlimited)
    pub fn new(session_id: String, max_docs: usize) -> Self {
        // Initialize embedder with English language and standard BM25 parameters
        let embedder = EmbedderBuilder::with_avgdl(100.0)
            .language_mode(Language::English)
            .k1(1.2) // Standard BM25 k1 parameter
            .b(0.75) // Standard BM25 b parameter
            .build();

        Self {
            session_id,
            documents: HashMap::new(),
            embedder,
            scorer: Scorer::new(),
            max_docs: if max_docs == 0 { usize::MAX } else { max_docs },
        }
    }

    /// Reads the max documents limit from environment variable.
    ///
    /// Defaults to 10,000 if not set or invalid.
    pub fn max_docs_from_env() -> usize {
        std::env::var("MESSAGE_INDEX_MAX_DOCS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(10_000)
    }

    /// Adds or updates documents in the index.
    ///
    /// If the total document count exceeds `max_docs`, keeps only the most recent messages.
    ///
    /// # Arguments
    /// * `documents` - Messages to add to the index
    pub fn add_documents(&mut self, documents: Vec<MessageDocument>) -> Result<(), String> {
        if documents.is_empty() {
            return Ok(());
        }

        // Add new documents to collection
        for doc in &documents {
            self.documents.insert(doc.id.clone(), doc.clone());
        }

        // If we exceeded max_docs, keep only the most recent messages
        if self.documents.len() > self.max_docs {
            let mut docs_vec: Vec<_> = self.documents.values().cloned().collect();
            docs_vec.sort_by_key(|d| std::cmp::Reverse(d.created_at));
            docs_vec.truncate(self.max_docs);

            self.documents.clear();
            for doc in docs_vec {
                self.documents.insert(doc.id.clone(), doc);
            }
        }

        // Rebuild embedder with updated corpus
        self.rebuild_index()
    }

    /// Rebuilds the BM25 index from scratch using current documents.
    ///
    /// This recalculates avgdl and updates all embeddings.
    fn rebuild_index(&mut self) -> Result<(), String> {
        if self.documents.is_empty() {
            return Ok(());
        }

        // Collect all document texts for corpus statistics
        let all_texts: Vec<&str> = self
            .documents
            .values()
            .map(|d| d.content.as_str())
            .collect();

        // Rebuild embedder with accurate avgdl from corpus
        self.embedder = EmbedderBuilder::with_fit_to_corpus(Language::English, &all_texts)
            .k1(1.2)
            .b(0.75)
            .build();

        // Rebuild scorer with updated embeddings
        self.scorer = Scorer::new();
        for doc in self.documents.values() {
            let embedding = self.embedder.embed(&doc.content);
            self.scorer.upsert(&doc.id, embedding);
        }

        Ok(())
    }

    /// Searches for messages matching the query.
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// Vector of search results sorted by relevance (highest score first)
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        if self.documents.is_empty() {
            return Ok(Vec::new());
        }

        // Embed query
        let query_embedding = self.embedder.embed(query);

        // Get scored matches from scorer (already sorted by score descending)
        let scored_docs = self.scorer.matches(&query_embedding);

        // Convert to SearchResult with metadata
        let mut results: Vec<SearchResult> = scored_docs
            .into_iter()
            .filter_map(|scored_doc| {
                self.documents.get(&scored_doc.id).map(|doc| SearchResult {
                    message_id: doc.id.clone(),
                    session_id: doc.session_id.clone(),
                    score: scored_doc.score,
                    snippet: Self::extract_snippet(&doc.content, query),
                    created_at: doc.created_at,
                })
            })
            .collect();

        // Take top N results
        results.truncate(limit);

        Ok(results)
    }

    /// Extracts a snippet from content containing the query terms.
    ///
    /// Returns a ~200 character snippet centered around the first query match.
    fn extract_snippet(content: &str, query: &str) -> Option<String> {
        const SNIPPET_LENGTH: usize = 200;

        let content_lower = content.to_lowercase();
        let query_lower = query.to_lowercase();

        // Find first query term occurrence
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
        let mut match_pos = None;

        for term in query_terms {
            if let Some(pos) = content_lower.find(term) {
                match_pos = Some(pos);
                break;
            }
        }

        let start_pos = match match_pos {
            Some(pos) => pos.saturating_sub(SNIPPET_LENGTH / 2),
            None => 0, // No match found, take beginning
        };

        let end_pos = (start_pos + SNIPPET_LENGTH).min(content.len());
        let mut snippet = content[start_pos..end_pos].to_string();

        // Add ellipsis if truncated
        if start_pos > 0 {
            snippet = format!("...{snippet}");
        }
        if end_pos < content.len() {
            snippet = format!("{snippet}...");
        }

        Some(snippet)
    }

    /// Returns the number of documents in the index.
    pub fn doc_count(&self) -> usize {
        self.documents.len()
    }

    /// Returns the session ID for this engine.
    #[allow(dead_code)]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Serializes the index to bytes for persistence.
    ///
    /// Note: This serializes the documents; the BM25 index is rebuilt on load.
    pub fn serialize(&self) -> Result<Vec<u8>, String> {
        let docs_vec: Vec<_> = self.documents.values().cloned().collect();
        bincode::serialize(&docs_vec).map_err(|e| format!("Failed to serialize index: {e}"))
    }

    /// Deserializes and rebuilds the index from bytes.
    ///
    /// # Arguments
    /// * `session_id` - Session ID for the engine
    /// * `data` - Serialized document data
    /// * `max_docs` - Maximum document limit
    #[allow(dead_code)]
    pub fn deserialize(session_id: String, data: &[u8], max_docs: usize) -> Result<Self, String> {
        let docs: Vec<MessageDocument> =
            bincode::deserialize(data).map_err(|e| format!("Failed to deserialize index: {e}"))?;

        let mut engine = Self::new(session_id, max_docs);
        engine.add_documents(docs)?;

        Ok(engine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doc(id: &str, content: &str, created_at: i64) -> MessageDocument {
        MessageDocument {
            id: id.to_string(),
            session_id: "test-session".to_string(),
            content: content.to_string(),
            created_at,
        }
    }

    #[test]
    fn test_basic_search() {
        let mut engine = MessageSearchEngine::new("test-session".to_string(), 0);

        let docs = vec![
            create_test_doc("1", "The quick brown fox jumps over the lazy dog", 100),
            create_test_doc("2", "A fast brown animal leaps gracefully", 200),
            create_test_doc("3", "The weather is nice today", 300),
        ];

        engine.add_documents(docs).unwrap();

        let results = engine.search("brown fox", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].message_id, "1");
    }

    #[test]
    fn test_max_docs_limit() {
        let mut engine = MessageSearchEngine::new("test-session".to_string(), 2);

        let docs = vec![
            create_test_doc("1", "oldest message", 100),
            create_test_doc("2", "middle message", 200),
            create_test_doc("3", "newest message", 300),
        ];

        engine.add_documents(docs).unwrap();

        // Should keep only 2 most recent
        assert_eq!(engine.doc_count(), 2);
        assert!(engine.documents.contains_key("2"));
        assert!(engine.documents.contains_key("3"));
        assert!(!engine.documents.contains_key("1"));
    }

    #[test]
    fn test_snippet_extraction() {
        let content = "This is a very long message that contains important information about the quick brown fox jumping over lazy dogs in the forest.";
        let snippet = MessageSearchEngine::extract_snippet(content, "brown fox");

        assert!(snippet.is_some());
        let s = snippet.unwrap();
        assert!(s.contains("brown fox"));
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut engine = MessageSearchEngine::new("test-session".to_string(), 0);

        let docs = vec![
            create_test_doc("1", "first message", 100),
            create_test_doc("2", "second message", 200),
        ];

        engine.add_documents(docs).unwrap();

        // Serialize
        let data = engine.serialize().unwrap();

        // Deserialize
        let restored =
            MessageSearchEngine::deserialize("test-session".to_string(), &data, 0).unwrap();

        assert_eq!(restored.doc_count(), 2);
        assert_eq!(restored.session_id(), "test-session");

        // Search should work
        let results = restored.search("first", 10).unwrap();
        assert!(!results.is_empty());
    }
}
