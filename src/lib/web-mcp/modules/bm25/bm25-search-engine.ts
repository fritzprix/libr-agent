import type { FileChunk } from '@/lib/db';
import type { ISearchEngine, SearchOptions, SearchResult } from '@/models/search-engine';
import type { LocalBM25Instance } from './types';

// Worker-safe logger that falls back to console if Tauri logger is not available
const createWorkerSafeLogger = (context: string) => {
  // Fallback to console logger for Worker environment
  return {
    debug: (message: string, data?: unknown) => {
      console.log(`[${context}][DEBUG] ${message}`, data || '');
    },
    info: (message: string, data?: unknown) => {
      console.log(`[${context}][INFO] ${message}`, data || '');
    },
    warn: (message: string, data?: unknown) => {
      console.warn(`[${context}][WARN] ${message}`, data || '');
    },
    error: (message: string, data?: unknown) => {
      console.error(`[${context}][ERROR] ${message}`, data || '');
    },
  };
};

const logger = createWorkerSafeLogger('bm25-search-engine');

export class BM25SearchEngine implements ISearchEngine {
  private bm25Indexes = new Map<string, LocalBM25Instance>();
  private chunkMappings = new Map<string, Map<string, FileChunk>>();
  private indexLastUsed = new Map<string, number>();
  private isInitialized = false;
  private readonly MAX_INDEXES = 10;

  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    logger.info('Initializing BM25 search engine');
    
    // Enhanced initialization: Actually initialize the BM25 library here
    try {
      // Pre-load the BM25 library to ensure it's available
      await import('wink-bm25-text-search');
      logger.info('BM25 library loaded successfully');
      this.isInitialized = true;
    } catch (error) {
      logger.error('Failed to initialize BM25 library', error);
      throw new Error('BM25 search engine initialization failed');
    }
  }

  async addToIndex(storeId: string, newChunks: FileChunk[]): Promise<void> {
    if (!this.isInitialized) await this.initialize();

    let bm25 = this.bm25Indexes.get(storeId);
    let chunkMapping = this.chunkMappings.get(storeId);

    if (!bm25 || !chunkMapping) {
      // For web-mcp modules, we need to use dbUtils import from parent module
      const { dbUtils } = await import('@/lib/db');
      const allChunks = await dbUtils.getFileChunksByStore(storeId);
      await this.rebuildIndex(storeId, allChunks);
      return;
    }

    // Filter out chunks that are already in the index to prevent duplicates
    const newChunksToAdd = newChunks.filter(
      (chunk) => !chunkMapping.has(chunk.id),
    );

    if (newChunksToAdd.length === 0) {
      logger.warn('All chunks already exist in index, skipping', {
        storeId,
        requested: newChunks.length,
      });
      return;
    }

    // Add new chunks to the index
    let addedCount = 0;
    newChunksToAdd.forEach((chunk) => {
      try {
        if (!chunk.id || typeof chunk.id !== 'string') {
          logger.warn('Invalid chunk ID, skipping', { chunkId: chunk.id });
          return;
        }

        const processedText = this.preprocessText(chunk.text);
        bm25.addDoc({ id: chunk.id, text: processedText });
        chunkMapping.set(chunk.id, chunk);
        addedCount++;
      } catch (chunkError) {
        logger.error('Failed to add chunk to index', {
          chunkId: chunk.id,
          error:
            chunkError instanceof Error
              ? chunkError.message
              : String(chunkError),
        });
      }
    });

    // Consolidate only if we have documents
    if (chunkMapping.size > 0) {
      try {
        bm25.consolidate();
      } catch (consolidateError) {
        logger.warn('BM25 consolidation failed, continuing anyway', {
          storeId,
          error:
            consolidateError instanceof Error
              ? consolidateError.message
              : String(consolidateError),
        });
      }
    }

    this.indexLastUsed.set(storeId, Date.now());

    logger.info('Added chunks to search index', {
      storeId,
      added: addedCount,
      requested: newChunks.length,
      total: chunkMapping.size,
    });
  }

  private async rebuildIndex(
    storeId: string,
    chunks: FileChunk[],
  ): Promise<void> {
    await this.cleanupOldIndexes();
    const BM25Constructor = (await import('wink-bm25-text-search')).default;
    const bm25 = BM25Constructor() as LocalBM25Instance;

    bm25.defineConfig({
      fldWeights: { text: 1 },
      ovlpNormFactor: 0.5,
      k1: 1.2,
      b: 0.75,
    });

    const chunkMapping = new Map<string, FileChunk>();
    const addedIds = new Set<string>();

    // Add chunks to index
    chunks.forEach((chunk) => {
      if (addedIds.has(chunk.id)) return; // Skip duplicates

      try {
        const processedText = this.preprocessText(chunk.text);
        bm25.addDoc({ id: chunk.id, text: processedText });
        chunkMapping.set(chunk.id, chunk);
        addedIds.add(chunk.id);
      } catch (error) {
        logger.warn('Failed to add chunk during rebuild', {
          chunkId: chunk.id,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    });

    // Consolidate if we have documents
    if (addedIds.size > 0) {
      try {
        bm25.consolidate();
      } catch (consolidateError) {
        logger.warn('BM25 rebuild consolidation failed, continuing anyway', {
          storeId,
          error:
            consolidateError instanceof Error
              ? consolidateError.message
              : String(consolidateError),
        });
      }
    }

    this.bm25Indexes.set(storeId, bm25);
    this.chunkMappings.set(storeId, chunkMapping);
    this.indexLastUsed.set(storeId, Date.now());

    logger.info('BM25 index rebuilt', {
      storeId,
      chunks: addedIds.size,
      requested: chunks.length,
    });
  }

  private async cleanupOldIndexes(): Promise<void> {
    if (this.bm25Indexes.size < this.MAX_INDEXES) return;
    const sortedByAge = Array.from(this.indexLastUsed.entries()).sort(
      ([, a], [, b]) => a - b,
    );
    const toRemove = sortedByAge.slice(
      0,
      sortedByAge.length - this.MAX_INDEXES + 1,
    );
    for (const [storeId] of toRemove) {
      this.bm25Indexes.delete(storeId);
      this.chunkMappings.delete(storeId);
      this.indexLastUsed.delete(storeId);
    }
  }

  // ISearchEngine interface implementation
  async indexStore(storeId: string, chunks: FileChunk[]): Promise<void> {
    await this.rebuildIndex(storeId, chunks);
  }

  async search(
    storeId: string,
    query: string,
    options: SearchOptions,
  ): Promise<SearchResult[]> {
    let bm25 = this.bm25Indexes.get(storeId);
    const chunkMapping = this.chunkMappings.get(storeId);

    if (!bm25 || !chunkMapping) {
      const { dbUtils } = await import('@/lib/db');
      const chunks = await dbUtils.getFileChunksByStore(storeId);
      if (chunks.length === 0) return [];
      await this.rebuildIndex(storeId, chunks);
      return this.search(storeId, query, options);
    }

    this.indexLastUsed.set(storeId, Date.now());

    const processedQuery = this.preprocessText(query);
    const searchResults = bm25.search(processedQuery);

    return searchResults
      .filter((result) => result[1] >= (options.threshold || 0))
      .slice(0, options.topN)
      .map((result): SearchResult | null => {
        const chunk = chunkMapping.get(result[0]);
        if (!chunk) return null;
        return {
          contentId: chunk.contentId,
          chunkId: chunk.id,
          context: chunk.text,
          lineRange: [chunk.startLine, chunk.endLine],
          score: result[1],
          relevanceType: 'keyword',
        };
      })
      .filter((r): r is SearchResult => r !== null);
  }

  isReady(): boolean {
    return this.isInitialized;
  }

  async cleanup(): Promise<void> {
    this.bm25Indexes.clear();
    this.chunkMappings.clear();
    this.indexLastUsed.clear();
    this.isInitialized = false;
  }

  private preprocessText(text: string): string {
    return text
      .toLowerCase()
      .replace(/[^\w\s]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
  }
}