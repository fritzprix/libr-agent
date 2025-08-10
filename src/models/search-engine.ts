import type { FileChunk } from '@/lib/db';

// 검색 엔진 공통 인터페이스
export interface ISearchEngine {
  initialize(): Promise<void>;
  indexStore(storeId: string, chunks: FileChunk[]): Promise<void>;
  search(
    storeId: string,
    query: string,
    options: SearchOptions,
  ): Promise<SearchResult[]>;
  isReady(): boolean;
  cleanup(): Promise<void>;
}

export interface SearchOptions {
  topN: number;
  threshold?: number;
  searchType?: 'keyword' | 'semantic' | 'hybrid';
}

export interface SearchResult {
  contentId: string;
  chunkId: string;
  context: string;
  lineRange: [number, number];
  score: number;
  relevanceType: 'keyword' | 'semantic' | 'hybrid';
  filename?: string; // For better display
  highlightedContext?: string; // For search highlighting
  mimeType?: string; // For file type indication
}
