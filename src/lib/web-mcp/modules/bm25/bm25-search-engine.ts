import type { FileChunk } from '@/lib/db';
import type {
  ISearchEngine,
  SearchOptions,
  SearchResult,
} from '@/models/search-engine';
import type { BMDocument } from './types';
import * as okapibm25 from 'okapibm25';
const BM25 = okapibm25.default;

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
  private storeChunks = new Map<string, FileChunk[]>();
  private indexLastUsed = new Map<string, number>();
  private isInitialized = false;
  private readonly MAX_INDEXES = 10;

  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    // okapibm25 requires no special initialization
    this.isInitialized = true;
    logger.info('OkapiBM25 search engine initialized');
  }

  async addToIndex(storeId: string, newChunks: FileChunk[]): Promise<void> {
    if (!this.isInitialized) await this.initialize();

    const existingChunks = this.storeChunks.get(storeId) || [];
    const allChunks = [...existingChunks];

    let addedCount = 0;
    newChunks.forEach((newChunk) => {
      if (!allChunks.some((existing) => existing.id === newChunk.id)) {
        allChunks.push(newChunk);
        addedCount++;
      }
    });

    this.storeChunks.set(storeId, allChunks);
    this.indexLastUsed.set(storeId, Date.now());

    logger.info('Added chunks to search index', {
      storeId,
      added: addedCount,
      requested: newChunks.length,
      total: allChunks.length,
    });
  }

  private async rebuildIndex(
    storeId: string,
    chunks: FileChunk[],
  ): Promise<void> {
    await this.cleanupOldIndexes();

    // Simply store the chunks - no complex initialization needed
    const uniqueChunks: FileChunk[] = [];
    const addedIds = new Set<string>();

    chunks.forEach((chunk) => {
      if (!addedIds.has(chunk.id)) {
        uniqueChunks.push(chunk);
        addedIds.add(chunk.id);
      }
    });

    this.storeChunks.set(storeId, uniqueChunks);
    this.indexLastUsed.set(storeId, Date.now());

    logger.info('BM25 index rebuilt', {
      storeId,
      chunks: uniqueChunks.length,
      requested: chunks.length,
    });
  }

  private async cleanupOldIndexes(): Promise<void> {
    if (this.storeChunks.size < this.MAX_INDEXES) return;
    const sortedByAge = Array.from(this.indexLastUsed.entries()).sort(
      ([, a], [, b]) => a - b,
    );
    const toRemove = sortedByAge.slice(
      0,
      sortedByAge.length - this.MAX_INDEXES + 1,
    );
    for (const [storeId] of toRemove) {
      this.storeChunks.delete(storeId);
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
    const startTime = Date.now();
    const chunks = this.storeChunks.get(storeId);

    if (!chunks || chunks.length === 0) {
      const { dbUtils } = await import('@/lib/db');
      const dbChunks = await dbUtils.getFileChunksByStore(storeId);
      if (dbChunks.length === 0) return [];
      await this.rebuildIndex(storeId, dbChunks);
      return this.search(storeId, query, options);
    }

    this.indexLastUsed.set(storeId, Date.now());

    const documents = chunks.map((chunk) => this.preprocessText(chunk.text));
    const queryTerms = this.preprocessText(query).split(/\s+/).filter(Boolean);

    logger.info('Search debug info', {
      storeId,
      chunksCount: chunks.length,
      documentsCount: documents.length,
      queryTerms,
      sampleDocument: documents[0]?.substring(0, 50),
    });

    if (queryTerms.length === 0) return [];

    const results = BM25(
      documents,
      queryTerms,
      { k1: 1.2, b: 0.75 },
      (a: BMDocument, b: BMDocument) => b.score - a.score,
    ) as BMDocument[];

    logger.info('BM25 raw results', {
      storeId,
      rawResultsCount: results.length,
      resultsType: typeof results,
      isArray: Array.isArray(results),
      sampleResult: results[0]
        ? {
            score: results[0].score,
            documentPreview:
              results[0].document?.substring(0, 50) || 'NO DOCUMENT',
            resultKeys: Object.keys(results[0]),
          }
        : null,
    });

    const searchResults = results
      .filter((result) => result.score >= (options.threshold || 0))
      .slice(0, options.topN)
      .map((result): SearchResult | null => {
        const originalIndex = documents.indexOf(result.document);
        logger.debug('Mapping result', {
          resultDocument: result.document.substring(0, 50),
          originalIndex,
          score: result.score,
        });

        if (originalIndex === -1) {
          logger.warn('Could not find original index for result', {
            resultDocument: result.document,
            availableDocuments: documents.map((d) => d.substring(0, 50)),
          });
          return null;
        }

        const chunk = chunks[originalIndex];

        return {
          contentId: chunk.contentId,
          chunkId: chunk.id,
          context: chunk.text,
          lineRange: [chunk.startLine, chunk.endLine],
          score: result.score,
          relevanceType: 'keyword',
        };
      })
      .filter((result): result is SearchResult => result !== null);

    const endTime = Date.now();
    logger.info('Search completed', {
      storeId,
      query,
      resultsCount: searchResults.length,
      processingTime: endTime - startTime,
    });

    return searchResults;
  }

  isReady(): boolean {
    return this.isInitialized;
  }

  async cleanup(): Promise<void> {
    this.storeChunks.clear();
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
