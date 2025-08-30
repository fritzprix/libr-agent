/**
 * Content Store Search Module
 *
 * Handles search functionality using BM25 search engine
 */

import { BM25SearchEngine } from '../bm25';
import type { SearchResult } from '@/models/search-engine';
import type { FileChunk } from '@/lib/db';
import { logger } from './logger';
import type { SearchOptions } from './types';

export interface SearchDocument {
  id: string;
  content: string;
  metadata?: Record<string, unknown>;
}

export class ContentSearchEngine {
  private searchEngine: BM25SearchEngine = new BM25SearchEngine();
  private initialized = false;

  /**
   * Initialize search engine
   */
  async initialize(): Promise<void> {
    if (this.initialized) return;

    logger.debug('Initializing content search engine');
    await this.searchEngine.initialize();
    this.initialized = true;

    logger.info('Content search engine initialized');
  }

  /**
   * Add chunks to the search index for a store
   */
  async addToIndex(storeId: string, chunks: FileChunk[]): Promise<void> {
    if (!this.initialized) {
      await this.initialize();
    }

    logger.debug('Adding chunks to search index', {
      storeId,
      chunkCount: chunks.length,
    });

    await this.searchEngine.addToIndex(storeId, chunks);

    logger.debug('Chunks added to search index', {
      storeId,
      chunkCount: chunks.length,
    });
  }

  /**
   * Search documents in a store
   */
  async search(
    storeId: string,
    query: string,
    options: SearchOptions = {},
  ): Promise<SearchResult[]> {
    if (!this.initialized) {
      await this.initialize();
    }

    const { limit = 20, scoreThreshold = 0.1 } = options;

    logger.debug('Performing search', {
      storeId,
      query,
      limit,
      scoreThreshold,
    });

    const results = await this.searchEngine.search(storeId, query, {
      topN: limit,
      threshold: scoreThreshold,
    });

    logger.debug('Search completed', {
      storeId,
      query,
      resultsCount: results.length,
    });

    return results;
  }

  /**
   * Remove search index for a store
   */
  async removeStore(storeId: string): Promise<void> {
    logger.debug('Removing search index for store', { storeId });
    // BM25SearchEngine doesn't have removeIndex, cleanup is done automatically
    logger.debug('Search index cleanup handled automatically for store', {
      storeId,
    });
  }

  /**
   * Check if search engine is ready
   */
  get isReady(): boolean {
    return this.initialized;
  }

  /**
   * Clear all search indices
   */
  async clear(): Promise<void> {
    await this.searchEngine.cleanup();
    logger.debug('All search indices cleared');
  }
}

// Export singleton instance
export const contentSearchEngine = new ContentSearchEngine();
