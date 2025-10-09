use bm25::{Embedder, EmbedderBuilder, Language, Scorer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Search result for keyword queries
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub content_id: String,
    pub chunk_id: String,
    pub score: f64,
    pub matched_text: String,
    pub line_range: (usize, usize),
}

/// Text chunk for indexing
#[derive(Debug, Clone)]
pub struct TextChunk {
    pub id: String,
    pub content_id: String,
    pub text: String,
    pub line_range: (usize, usize),
}

/// Index statistics
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub num_docs: usize,
    pub num_segments: usize,
}

/// BM25 Search Engine Implementation using bm25 crate
pub struct ContentSearchEngine {
    // Store original chunks for metadata retrieval
    chunks: HashMap<String, TextChunk>,
    // BM25 embedder for query and document embedding
    embedder: Embedder,
    // BM25 scorer for relevance scoring
    scorer: Scorer<String>,
}

// Manual Debug implementation since Scorer doesn't implement Debug
impl std::fmt::Debug for ContentSearchEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentSearchEngine")
            .field("chunks_count", &self.chunks.len())
            .field("embedder", &"<Embedder>")
            .field("scorer", &"<Scorer>")
            .finish()
    }
}

impl ContentSearchEngine {
    /// Create a new BM25 search engine
    pub fn new(_index_dir: std::path::PathBuf) -> Result<Self, String> {
        // Initialize embedder with English language and standard BM25 parameters
        // Start with avgdl=100 as initial estimate, will be updated on first add_chunks
        let embedder = EmbedderBuilder::with_avgdl(100.0)
            .language_mode(Language::English)
            .k1(1.2) // Standard BM25 k1 parameter
            .b(0.75) // Standard BM25 b parameter
            .build();

        Ok(Self {
            chunks: HashMap::new(),
            embedder,
            scorer: Scorer::new(),
        })
    }

    /// Add text chunks to the index and update BM25 statistics
    pub async fn add_chunks(&mut self, chunks: Vec<TextChunk>) -> Result<(), String> {
        if chunks.is_empty() {
            return Ok(());
        }

        // Refit embedder with updated corpus for accurate avgdl
        let all_texts: Vec<&str> = self
            .chunks
            .values()
            .map(|c| c.text.as_str())
            .chain(chunks.iter().map(|c| c.text.as_str()))
            .collect();

        self.embedder = EmbedderBuilder::with_fit_to_corpus(Language::English, &all_texts)
            .k1(1.2)
            .b(0.75)
            .build();

        // Add new chunks to index
        for chunk in chunks {
            let chunk_id = chunk.id.clone();
            let embedding = self.embedder.embed(&chunk.text);
            self.scorer.upsert(&chunk_id, embedding);
            self.chunks.insert(chunk_id, chunk);
        }

        Ok(())
    }

    /// Remove chunks for a specific content ID from the search index
    pub async fn remove_chunks(&mut self, content_id: &str) -> Result<(), String> {
        // Find all chunks for this content_id
        let chunks_to_remove: Vec<String> = self
            .chunks
            .iter()
            .filter(|(_, chunk)| chunk.content_id == content_id)
            .map(|(id, _)| id.clone())
            .collect();

        // Remove chunks from scorer and storage
        for chunk_id in &chunks_to_remove {
            self.scorer.remove(chunk_id);
            self.chunks.remove(chunk_id);
        }

        // Refit embedder with remaining corpus
        if !self.chunks.is_empty() {
            let all_texts: Vec<&str> = self.chunks.values().map(|c| c.text.as_str()).collect();
            self.embedder = EmbedderBuilder::with_fit_to_corpus(Language::English, &all_texts)
                .k1(1.2)
                .b(0.75)
                .build();
        } else {
            // Reset to default if no documents remain
            self.embedder = EmbedderBuilder::with_avgdl(100.0)
                .language_mode(Language::English)
                .k1(1.2)
                .b(0.75)
                .build();
        }

        Ok(())
    }

    /// BM25 search implementation using bm25 crate
    pub async fn search_bm25(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, String> {
        if self.chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Embed the query
        let query_embedding = self.embedder.embed(query);

        // Get scored matches from the scorer
        let scored_docs = self.scorer.matches(&query_embedding);

        // Convert to SearchResult with metadata
        let mut results: Vec<SearchResult> = scored_docs
            .into_iter()
            .filter_map(|scored_doc| {
                self.chunks.get(&scored_doc.id).map(|chunk| SearchResult {
                    content_id: chunk.content_id.clone(),
                    chunk_id: chunk.id.clone(),
                    score: scored_doc.score as f64,
                    matched_text: Self::extract_snippet(&chunk.text, query, 200),
                    line_range: chunk.line_range,
                })
            })
            .collect();

        // Take top N results (scorer already returns sorted by score descending)
        results.truncate(limit);

        Ok(results)
    }

    /// Extract text snippet around query matches
    fn extract_snippet(text: &str, query: &str, max_length: usize) -> String {
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();

        if let Some(pos) = text_lower.find(&query_lower) {
            let start = pos.saturating_sub(max_length / 2);
            let end = (pos + query.len() + max_length / 2).min(text.len());

            let snippet = &text[start..end];
            if start > 0 {
                format!("...{snippet}")
            } else {
                snippet.to_string()
            }
        } else {
            text.chars().take(max_length).collect()
        }
    }
}
