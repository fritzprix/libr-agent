// BM25 (Best Matching 25) ranking implementation for small-to-medium corpora
// Includes in-memory LRU caching for performance optimization

/**
 * Represents a document in the BM25 index
 */
export interface BM25Doc {
  id: string;
  tokens: string[];
  length?: number; // Computed during indexing
}

/**
 * Configuration options for BM25 algorithm
 */
export interface BM25IndexOptions {
  k1?: number; // Term frequency saturation parameter (default: 1.5)
  b?: number; // Length normalization parameter (default: 0.75)
}

/**
 * BM25 search index with ranking capabilities
 */
export class BM25Index {
  private docs: BM25Doc[] = [];
  private N = 0; // Total number of documents
  private avgdl = 0; // Average document length
  private df: Map<string, number> = new Map(); // Document frequency per term
  private tf: Map<string, Map<string, number>> = new Map(); // Term frequency per (term, docId)

  constructor(private readonly options: BM25IndexOptions = {}) {}

  /**
   * Add documents to the index and compute statistics
   */
  addDocuments(docs: BM25Doc[]): void {
    this.docs = docs.map((d) => ({ ...d, length: d.tokens.length }));
    this.N = this.docs.length;
    this.avgdl =
      this.docs.reduce((sum, d) => sum + (d.length || 0), 0) /
      Math.max(1, this.N);

    this.df.clear();
    this.tf.clear();

    // Build inverted index
    for (const doc of this.docs) {
      const tfPerDoc: Map<string, number> = new Map();

      // Count term frequencies in this document
      for (const term of doc.tokens) {
        tfPerDoc.set(term, (tfPerDoc.get(term) || 0) + 1);
      }

      // Update global term frequency and document frequency maps
      for (const [term, freq] of tfPerDoc) {
        if (!this.tf.has(term)) {
          this.tf.set(term, new Map());
        }
        this.tf.get(term)!.set(doc.id, freq);
        this.df.set(term, (this.df.get(term) || 0) + 1);
      }
    }
  }

  /**
   * Compute IDF (Inverse Document Frequency) for a term
   * Uses add-one smoothing for better behavior with rare terms
   */
  private idf(term: string): number {
    const n = this.df.get(term) || 0;
    // BM25 IDF formula with add-one smoothing
    return Math.log(1 + (this.N - n + 0.5) / (n + 0.5));
  }

  /**
   * Score all documents for the given query tokens
   * Returns a map of docId -> BM25 score
   */
  score(queryTokens: string[]): Map<string, number> {
    const k1 = this.options.k1 ?? 1.5;
    const b = this.options.b ?? 0.75;
    const scores: Map<string, number> = new Map();

    // Deduplicate query tokens
    const uniqueTerms = Array.from(new Set(queryTokens));

    for (const term of uniqueTerms) {
      const posting = this.tf.get(term);
      if (!posting) continue;

      const idfScore = this.idf(term);

      // Score each document containing this term
      for (const [docId, termFreq] of posting) {
        const doc = this.docs.find((d) => d.id === docId);
        if (!doc || !doc.length) continue;

        const dl = doc.length;
        const denom = termFreq + k1 * (1 - b + b * (dl / this.avgdl));
        const score =
          idfScore * ((termFreq * (k1 + 1)) / Math.max(1e-9, denom));

        scores.set(docId, (scores.get(docId) || 0) + score);
      }
    }

    return scores;
  }
}

/**
 * Default tokenizer with Unicode normalization
 * Supports multiple languages and character systems
 */
export function defaultTokenizer(text: string): string[] {
  return (text || '')
    .toLowerCase()
    .normalize('NFKC') // Unicode normalization for consistent character representation
    .split(/[\s\p{P}\p{S}]+/u) // Split by whitespace, punctuation, symbols (Unicode-aware)
    .filter((token) => token.length > 1); // Filter out single characters
}

/**
 * LRU Cache entry for BM25 indices
 */
interface CacheEntry {
  index: BM25Index;
  timestamp: number;
  serializedDocs: string; // For cache key comparison
}

/**
 * BM25 Index Cache with LRU eviction
 */
class BM25IndexCache {
  private cache: Map<string, CacheEntry> = new Map();
  private readonly maxEntries: number;

  constructor(maxEntries = 3) {
    this.maxEntries = maxEntries;
  }

  /**
   * Generate cache key from documents
   */
  private generateKey(docs: BM25Doc[]): string {
    // Use sorted doc IDs as cache key (assumes doc content doesn't change)
    return docs
      .map((d) => d.id)
      .sort()
      .join(',');
  }

  /**
   * Get cached index or return null
   */
  get(docs: BM25Doc[]): BM25Index | null {
    const key = this.generateKey(docs);
    const entry = this.cache.get(key);

    if (!entry) return null;

    // Update timestamp for LRU
    entry.timestamp = Date.now();
    return entry.index;
  }

  /**
   * Store index in cache with LRU eviction
   */
  set(docs: BM25Doc[], index: BM25Index): void {
    const key = this.generateKey(docs);

    // Evict oldest entry if cache is full
    if (this.cache.size >= this.maxEntries && !this.cache.has(key)) {
      let oldestKey: string | null = null;
      let oldestTime = Infinity;

      for (const [k, entry] of this.cache) {
        if (entry.timestamp < oldestTime) {
          oldestTime = entry.timestamp;
          oldestKey = k;
        }
      }

      if (oldestKey) {
        this.cache.delete(oldestKey);
      }
    }

    this.cache.set(key, {
      index,
      timestamp: Date.now(),
      serializedDocs: key,
    });
  }

  /**
   * Clear all cached indices
   */
  clear(): void {
    this.cache.clear();
  }

  /**
   * Get cache statistics
   */
  getStats(): { size: number; maxSize: number } {
    return {
      size: this.cache.size,
      maxSize: this.maxEntries,
    };
  }
}

// Global cache instance
const globalCache = new BM25IndexCache(3);

/**
 * Create or retrieve a cached BM25 index
 */
export function createBM25Index(
  docs: BM25Doc[],
  options?: BM25IndexOptions,
): BM25Index {
  // Try to get from cache
  const cached = globalCache.get(docs);
  if (cached) {
    return cached;
  }

  // Create new index
  const index = new BM25Index(options);
  index.addDocuments(docs);

  // Store in cache
  globalCache.set(docs, index);

  return index;
}

/**
 * Clear the BM25 index cache (useful when data changes)
 */
export function clearBM25Cache(): void {
  globalCache.clear();
}

/**
 * Get cache statistics
 */
export function getBM25CacheStats(): { size: number; maxSize: number } {
  return globalCache.getStats();
}
