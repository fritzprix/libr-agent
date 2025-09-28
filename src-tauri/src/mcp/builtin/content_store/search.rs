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

/// BM25 Search Engine Implementation
#[derive(Debug)]
pub struct ContentSearchEngine {
    chunks: HashMap<String, TextChunk>,
    // BM25 parameters
    k1: f64,                               // Term frequency saturation (typically 1.2-2.0)
    b: f64,                                // Length normalization (typically 0.75)
    avg_doc_len: f64,                      // Average document length
    total_docs: usize,                     // Total number of documents
    term_doc_freq: HashMap<String, usize>, // Document frequency for each term
}

impl ContentSearchEngine {
    /// Create a new BM25 search engine
    pub fn new(_index_dir: std::path::PathBuf) -> Result<Self, String> {
        Ok(Self {
            chunks: HashMap::new(),
            k1: 1.5, // Standard BM25 k1 parameter
            b: 0.75, // Standard BM25 b parameter
            avg_doc_len: 0.0,
            total_docs: 0,
            term_doc_freq: HashMap::new(),
        })
    }

    /// Add text chunks to the index and update BM25 statistics
    pub async fn add_chunks(&mut self, chunks: Vec<TextChunk>) -> Result<(), String> {
        for chunk in chunks {
            // Update document frequency for each term in this chunk
            let terms = self.tokenize(&chunk.text);
            for term in terms {
                *self.term_doc_freq.entry(term).or_insert(0) += 1;
            }

            self.chunks.insert(chunk.id.clone(), chunk);
        }

        // Update total document count and average document length
        self.total_docs = self.chunks.len();
        self.avg_doc_len = if self.total_docs > 0 {
            self.chunks
                .values()
                .map(|c| c.text.len() as f64)
                .sum::<f64>()
                / self.total_docs as f64
        } else {
            0.0
        };

        Ok(())
    }

    /// BM25 search implementation
    pub async fn search_bm25(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let query_terms = self.tokenize(query);
        let mut results = Vec::new();

        for chunk in self.chunks.values() {
            let score = self.bm25_score(&query_terms, chunk);
            if score > 0.0 {
                let matched_text = Self::extract_snippet(&chunk.text, query, 200);
                results.push(SearchResult {
                    content_id: chunk.content_id.clone(),
                    chunk_id: chunk.id.clone(),
                    score,
                    matched_text,
                    line_range: chunk.line_range,
                });
            }
        }

        // Sort by score descending and take top N
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Calculate BM25 score for a document given query terms
    fn bm25_score(&self, query_terms: &[String], chunk: &TextChunk) -> f64 {
        let mut score = 0.0;
        let doc_len = chunk.text.len() as f64;

        for term in query_terms {
            let tf = self.term_frequency(term, chunk) as f64;
            let idf = self.inverse_document_frequency(term);

            if tf > 0.0 && idf > 0.0 {
                let numerator = tf * (self.k1 + 1.0);
                let denominator =
                    tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len));
                score += idf * (numerator / denominator);
            }
        }

        score
    }

    /// Calculate term frequency in a document
    fn term_frequency(&self, term: &str, chunk: &TextChunk) -> usize {
        let term_lower = term.to_lowercase();
        let text_lower = chunk.text.to_lowercase();
        text_lower.matches(&term_lower).count()
    }

    /// Calculate inverse document frequency
    fn inverse_document_frequency(&self, term: &str) -> f64 {
        let df = self.term_doc_freq.get(term).copied().unwrap_or(0) as f64;
        if df == 0.0 {
            return 0.0;
        }

        let n = self.total_docs as f64;
        ((n - df + 0.5) / (df + 0.5)).ln()
    }

    /// Tokenize text into terms (simple whitespace splitting)
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|s| {
                s.to_lowercase()
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect()
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
