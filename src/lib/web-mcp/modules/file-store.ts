import type { WebMCPServer, MCPTool } from '@/lib/mcp-types';
import {
  dbService,
  dbUtils,
  FileChunk,
  FileContent,
  FileStore,
} from '@/lib/db';
import { AttachmentReference } from '@/models/chat';
import {
  ISearchEngine,
  SearchOptions,
  SearchResult,
} from '@/models/search-engine';
import BM25 from 'wink-bm25-text-search';
import { getLogger } from '@/lib/logger';

const logger = getLogger('FileStoreMCP');

// FileStore 서버 타입 정의 - 함수 형식에만 집중
export interface FileStoreServer {
  store_file(args: { name: string; content: string; contentType?: string; metadata?: Record<string, unknown> }): Promise<{ id: string; success: boolean; chunks?: number }>;
  retrieve_file(args: { id: string }): Promise<{ content: string; metadata: Record<string, unknown>; contentType: string }>;
  list_files(args?: { limit?: number; offset?: number }): Promise<{ files: Array<{ id: string; name: string; contentType: string; size: number; createdAt: string; metadata: Record<string, unknown> }> }>;
  delete_file(args: { id: string }): Promise<{ success: boolean }>;
  search_files(args: { query: string; limit?: number; contentType?: string }): Promise<{ results: Array<{ id: string; name: string; score: number; snippet: string; metadata: Record<string, unknown> }> }>;
  get_file_info(args: { id: string }): Promise<{ id: string; name: string; contentType: string; size: number; chunks: number; createdAt: string; metadata: Record<string, unknown> }>;
  update_file_metadata(args: { id: string; metadata: Record<string, unknown> }): Promise<{ success: boolean }>;
  get_chunk(args: { id: string; chunkIndex: number }): Promise<{ content: string; chunkIndex: number; totalChunks: number }>;
  list_chunks(args: { id: string }): Promise<{ chunks: Array<{ index: number; size: number; hash: string }> }>;
  clear_store(args?: Record<string, never>): Promise<{ success: boolean; deletedCount: number }>;
  get_store_stats(args?: Record<string, never>): Promise<{ totalFiles: number; totalSize: number; totalChunks: number; avgFileSize: number }>;
}

// Custom error classes for better error handling
class FileStoreError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly context?: Record<string, unknown>,
  ) {
    super(message);
    this.name = 'FileStoreError';
  }
}

class StoreNotFoundError extends FileStoreError {
  constructor(storeId: string) {
    super(`Store not found: ${storeId}`, 'STORE_NOT_FOUND', { storeId });
  }
}

class ContentNotFoundError extends FileStoreError {
  constructor(contentId: string, storeId?: string) {
    super(`Content not found: ${contentId}`, 'CONTENT_NOT_FOUND', {
      contentId,
      storeId,
    });
  }
}

class InvalidRangeError extends FileStoreError {
  constructor(fromLine: number, toLine: number, totalLines: number) {
    super(
      `Invalid line range: ${fromLine}-${toLine} (total: ${totalLines})`,
      'INVALID_LINE_RANGE',
      { fromLine, toLine, totalLines },
    );
  }
}

// BM25 라이브러리의 실제 반환 타입에 맞춘 타입 정의
type BM25SearchResult = { 0: string; 1: number }; // BM25 라이브러리가 실제 반환하는 형태

interface LocalBM25Instance {
  defineConfig: (config: {
    fldWeights: { [key: string]: number };
    ovlpNormFactor: number;
    k1: number;
    b: number;
  }) => void;
  addDoc: (doc: { id: string; text: string }) => void;
  consolidate: () => void;
  search: (query: string) => BM25SearchResult[];
}

class TextChunker {
  private readonly CHUNK_SIZE = 500; // characters
  private readonly OVERLAP_SIZE = 50; // characters
  private readonly MIN_CHUNK_SIZE = 100; // minimum chunk size

  chunkText(
    content: string,
  ): { text: string; startLine: number; endLine: number }[] {
    const sentences = this.splitIntoSentences(content);
    const chunks: { text: string; startLine: number; endLine: number }[] = [];

    let currentChunk = '';
    let currentSentences: string[] = [];
    let chunkStartLine = 1;

    for (const sentence of sentences) {
      const sentenceWithSpace = currentChunk ? ` ${sentence}` : sentence;
      const potentialChunk = currentChunk + sentenceWithSpace;

      // 청크 크기 초과 시 현재 청크 완료
      if (
        potentialChunk.length > this.CHUNK_SIZE &&
        currentChunk.length >= this.MIN_CHUNK_SIZE
      ) {
        const endLine = chunkStartLine + this.countLines(currentChunk) - 1;
        chunks.push({
          text: currentChunk.trim(),
          startLine: chunkStartLine,
          endLine,
        });

        // Overlap 적용
        const { overlapText, overlapLines } =
          this.createOverlap(currentSentences);
        currentChunk = overlapText;
        currentSentences = [...overlapLines];
        chunkStartLine = Math.max(
          1,
          endLine - this.countLines(overlapText) + 1,
        );
      }

      currentChunk = potentialChunk;
      currentSentences.push(sentence);
    }

    // 마지막 청크 처리
    if (currentChunk.trim()) {
      const endLine = chunkStartLine + this.countLines(currentChunk) - 1;
      chunks.push({
        text: currentChunk.trim(),
        startLine: chunkStartLine,
        endLine,
      });
    }

    return chunks.length > 0
      ? chunks
      : [{ text: content, startLine: 1, endLine: this.countLines(content) }];
  }

  private splitIntoSentences(text: string): string[] {
    // 문장 구분자: 마침표, 느낌표, 물음표 + 공백/줄바꿈
    const sentencePattern = /([.!?]+)(\s+|$)/g;
    const sentences: string[] = [];
    let lastIndex = 0;

    text.replace(sentencePattern, (match, punctuation, _whitespace, offset) => {
      const sentence = text
        .slice(lastIndex, offset + punctuation.length)
        .trim();
      if (sentence.length > 0) {
        sentences.push(sentence);
      }
      lastIndex = offset + match.length;
      return match;
    });

    // 남은 텍스트 처리
    const remaining = text.slice(lastIndex).trim();
    if (remaining) {
      sentences.push(remaining);
    }

    return sentences.length > 0 ? sentences : [text];
  }

  private createOverlap(sentences: string[]): {
    overlapText: string;
    overlapLines: string[];
  } {
    let overlapText = '';
    const overlapLines: string[] = [];

    // 뒤에서부터 overlap 크기에 맞을 때까지 문장 수집
    for (let i = sentences.length - 1; i >= 0; i--) {
      const sentence = sentences[i];
      const potentialOverlap =
        sentence + (overlapText ? ` ${overlapText}` : '');

      if (
        potentialOverlap.length > this.OVERLAP_SIZE &&
        overlapText.length > 0
      ) {
        break;
      }

      overlapLines.unshift(sentence);
      overlapText = potentialOverlap;
    }

    return { overlapText, overlapLines };
  }

  private countLines(text: string): number {
    return text ? text.split('\n').length : 1;
  }
}

class BM25SearchEngine implements ISearchEngine {
  private bm25Indexes = new Map<string, LocalBM25Instance>();
  private chunkMappings = new Map<string, Map<string, FileChunk>>();
  private indexLastUsed = new Map<string, number>();
  private isInitialized = false;
  private readonly MAX_INDEXES = 10;

  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    logger.info('Initializing BM25 search engine');
    this.isInitialized = true;
  }

  async addToIndex(storeId: string, newChunks: FileChunk[]): Promise<void> {
    if (!this.isInitialized) await this.initialize();

    let bm25 = this.bm25Indexes.get(storeId);
    let chunkMapping = this.chunkMappings.get(storeId);

    if (!bm25 || !chunkMapping) {
      // 기존 인덱스가 없으면 전체 인덱스 재구성
      const allChunks = await dbUtils.getFileChunksByStore(storeId);
      await this.rebuildIndex(storeId, allChunks);
      return;
    }

    // 기존 인덱스에 새 chunk들 추가
    newChunks.forEach((chunk) => {
      const processedText = this.preprocessText(chunk.text);
      bm25.addDoc({ id: chunk.id, text: processedText });
      chunkMapping.set(chunk.id, chunk);
    });

    bm25.consolidate();
    this.indexLastUsed.set(storeId, Date.now());

    logger.info('Added chunks to existing index', {
      storeId,
      newChunkCount: newChunks.length,
      totalChunks: chunkMapping.size,
    });
  }

  private async rebuildIndex(
    storeId: string,
    chunks: FileChunk[],
  ): Promise<void> {
    await this.cleanupOldIndexes();

    const bm25 = BM25();
    bm25.defineConfig({
      fldWeights: { text: 1 },
      ovlpNormFactor: 0.5,
      k1: 1.2,
      b: 0.75,
    });

    const chunkMapping = new Map<string, FileChunk>();
    chunks.forEach((chunk) => {
      const processedText = this.preprocessText(chunk.text);
      bm25.addDoc({ id: chunk.id, text: processedText });
      chunkMapping.set(chunk.id, chunk);
    });

    bm25.consolidate();
    this.bm25Indexes.set(storeId, bm25);
    this.chunkMappings.set(storeId, chunkMapping);
    this.indexLastUsed.set(storeId, Date.now());

    logger.info('Index rebuilt', { storeId, chunkCount: chunks.length });
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
      logger.debug('Removed old index from memory', { storeId });
    }
  }

  async indexStore(storeId: string, chunks: FileChunk[]): Promise<void> {
    await this.rebuildIndex(storeId, chunks);
  }

  async search(
    storeId: string,
    query: string,
    options: SearchOptions,
  ): Promise<SearchResult[]> {
    const bm25 = this.bm25Indexes.get(storeId);
    const chunkMapping = this.chunkMappings.get(storeId);

    if (!bm25 || !chunkMapping) {
      const chunks = await dbUtils.getFileChunksByStore(storeId);
      if (chunks.length === 0) {
        logger.warn('No chunks found for store', { storeId });
        return [];
      }

      await this.rebuildIndex(storeId, chunks);
      return this.search(storeId, query, options);
    }

    // 사용 시간 업데이트
    this.indexLastUsed.set(storeId, Date.now());

    const processedQuery = this.preprocessText(query);
    const searchResults = bm25.search(processedQuery);

    const results: SearchResult[] = searchResults
      .filter(
        (result: BM25SearchResult) => result[1] >= (options.threshold || 0),
      ) // threshold 적용
      .slice(0, options.topN)
      .map((result: BM25SearchResult): SearchResult | null => {
        const chunk = chunkMapping.get(result[0]);
        if (!chunk) {
          logger.warn('Chunk not found in mapping', {
            chunkId: result[0],
            storeId,
          });
          return null;
        }
        return {
          contentId: chunk.contentId,
          chunkId: chunk.id,
          context: chunk.text,
          lineRange: [chunk.startLine, chunk.endLine] as [number, number],
          score: result[1],
          relevanceType: 'keyword',
        };
      })
      .filter((result): result is SearchResult => result !== null);

    logger.debug('BM25 search completed', {
      storeId,
      query: processedQuery,
      resultCount: results.length,
      totalResults: searchResults.length,
    });
    return results;
  }

  isReady(): boolean {
    return this.isInitialized;
  }

  async cleanup(): Promise<void> {
    this.bm25Indexes.clear();
    this.chunkMappings.clear();
    this.indexLastUsed.clear();
    this.isInitialized = false;
    logger.info('BM25 search engine cleaned up');
  }

  private preprocessText(text: string): string {
    return text
      .toLowerCase()
      .replace(/[^\w\s가-힣]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
  }
}

const tools: MCPTool[] = [
  {
    name: 'createStore',
    description: 'Create a new content store for file management',
    inputSchema: {
      type: 'object',
      properties: {
        metadata: {
          type: 'object',
          properties: {
            name: { type: 'string' },
            description: { type: 'string' },
            sessionId: { type: 'string' },
          },
        },
      },
    },
  },
  {
    name: 'addContent',
    description: 'Add file content with chunking and indexing',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: { type: 'string' },
        content: { type: 'string' },
        metadata: {
          type: 'object',
          properties: {
            filename: { type: 'string' },
            mimeType: { type: 'string' },
            size: { type: 'number' },
            uploadedAt: { type: 'string' },
          },
          required: ['filename', 'mimeType', 'size', 'uploadedAt'],
        },
      },
      required: ['storeId', 'content', 'metadata'],
    },
  },
  {
    name: 'listContent',
    description: 'List content summaries in a store',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: { type: 'string' },
        pagination: {
          type: 'object',
          properties: { offset: { type: 'number' }, limit: { type: 'number' } },
        },
      },
      required: ['storeId'],
    },
  },
  {
    name: 'readContent',
    description: 'Read specific line ranges from content',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: { type: 'string' },
        contentId: { type: 'string' },
        lineRange: {
          type: 'object',
          properties: {
            fromLine: { type: 'number' },
            toLine: { type: 'number' },
          },
          required: ['fromLine'],
        },
      },
      required: ['storeId', 'contentId', 'lineRange'],
    },
  },
  {
    name: 'similaritySearch',
    description: 'Semantic search across content',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: { type: 'string' },
        query: { type: 'string' },
        options: {
          type: 'object',
          properties: {
            topN: { type: 'number', default: 5 },
            threshold: { type: 'number', default: 0.5 },
          },
        },
      },
      required: ['storeId', 'query'],
    },
  },
];

export interface CreateStoreInput {
  metadata?: { name?: string; description?: string; sessionId?: string };
}
export interface CreateStoreOutput {
  storeId: string;
  createdAt: Date;
}
export interface AddContentInput {
  storeId: string;
  content: string;
  metadata: {
    filename: string;
    mimeType: string;
    size: number;
    uploadedAt: string;
  };
}
export interface AddContentOutput
  extends Omit<AttachmentReference, 'storeId' | 'contentId' | 'uploadedAt'> {
  storeId: string;
  contentId: string;
  chunkCount: number;
  uploadedAt: Date;
}
export interface ListContentInput {
  storeId: string;
  pagination?: { offset?: number; limit?: number };
}
export interface ReadContentInput {
  storeId: string;
  contentId: string;
  lineRange: { fromLine: number; toLine?: number };
}
export interface SimilaritySearchInput {
  storeId: string;
  query: string;
  options?: { topN?: number; threshold?: number };
}
export type ContentSummary = AttachmentReference;

const searchEngine = new BM25SearchEngine();
const textChunker = new TextChunker();

async function createStore(
  input: CreateStoreInput,
): Promise<CreateStoreOutput> {
  const now = new Date();
  const store: FileStore = {
    id: `store_${Date.now()}`,
    name: input.metadata?.name || 'Unnamed Store',
    description: input.metadata?.description,
    sessionId: input.metadata?.sessionId,
    createdAt: now,
    updatedAt: now,
  };
  await dbService.fileStores.upsert(store);
  logger.info('Store created', { storeId: store.id, name: store.name });
  return { storeId: store.id, createdAt: now };
}

async function addContent(input: AddContentInput): Promise<AddContentOutput> {
  try {
    // Store 존재 여부 확인
    const store = await dbService.fileStores.read(input.storeId);
    if (!store) {
      throw new StoreNotFoundError(input.storeId);
    }

    const contentId = `content_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const lines = input.content.split('\n');
    const summary = lines.slice(0, 20).join('\n');
    const uploadedAt = new Date(input.metadata.uploadedAt);

    // Content 생성
    const content: FileContent = {
      id: contentId,
      storeId: input.storeId,
      filename: input.metadata.filename,
      mimeType: input.metadata.mimeType,
      size: input.metadata.size,
      uploadedAt,
      content: input.content,
      lineCount: lines.length,
      summary,
    };

    // Chunking
    const chunksData = textChunker.chunkText(input.content);
    const chunks: FileChunk[] = chunksData.map((chunkData, index) => ({
      id: `${contentId}_chunk_${index}`,
      contentId,
      chunkIndex: index,
      text: chunkData.text,
      startLine: chunkData.startLine,
      endLine: chunkData.endLine,
    }));

    // DB 저장
    await dbService.fileContents.upsert(content);
    await dbService.fileChunks.upsertMany(chunks);

    // 기존 인덱스에 새 chunk만 추가 (전체 재인덱싱 방지)
    await searchEngine.addToIndex(input.storeId, chunks);

    logger.info('Content added successfully', {
      contentId,
      filename: input.metadata.filename,
      chunkCount: chunks.length,
      storeId: input.storeId,
    });

    return {
      storeId: input.storeId,
      contentId,
      filename: content.filename,
      mimeType: content.mimeType,
      size: content.size,
      lineCount: content.lineCount,
      preview: content.summary,
      chunkCount: chunks.length,
      uploadedAt,
    };
  } catch (error) {
    logger.error('Failed to add content', error, {
      storeId: input.storeId,
      filename: input.metadata.filename,
    });
    throw error;
  }
}

async function listContent(
  input: ListContentInput,
): Promise<{ contents: ContentSummary[]; total: number; hasMore: boolean }> {
  const { storeId, pagination } = input;
  const limit = Math.min(pagination?.limit || 50, 100); // 최대 100개 제한
  const offset = Math.max(pagination?.offset || 0, 0);

  try {
    // 전체 페이지를 가져와서 storeId로 필터링
    // TODO: DB에 findByStoreId 메서드 추가 필요
    const allPages = await dbService.fileContents.getPage(1, 1000); // 임시로 큰 페이지 사이즈 사용
    const allContentsInStore = allPages.items.filter(
      (item: FileContent) => item.storeId === storeId,
    );
    const total = allContentsInStore.length;

    const paginatedContents = allContentsInStore.slice(offset, offset + limit);

    const summaries: ContentSummary[] = paginatedContents.map(
      (c: FileContent) => ({
        storeId: c.storeId,
        contentId: c.id,
        filename: c.filename,
        mimeType: c.mimeType,
        size: c.size,
        lineCount: c.lineCount,
        preview: c.summary,
        uploadedAt: c.uploadedAt.toISOString(),
      }),
    );

    const hasMore = offset + limit < total;

    logger.debug('Listed content', {
      storeId,
      offset,
      limit,
      total,
      returned: summaries.length,
      hasMore,
    });

    return { contents: summaries, total, hasMore };
  } catch (error) {
    logger.error('Failed to list content', error, { input });
    throw new FileStoreError(
      'Failed to retrieve content list',
      'LIST_CONTENT_FAILED',
      { storeId, pagination },
    );
  }
}

async function readContent(
  input: ReadContentInput,
): Promise<{ content: string; lineRange: [number, number] }> {
  try {
    const content = await dbService.fileContents.read(input.contentId);

    if (!content) {
      throw new ContentNotFoundError(input.contentId, input.storeId);
    }

    if (content.storeId !== input.storeId) {
      throw new FileStoreError(
        'Content belongs to different store',
        'STORE_MISMATCH',
        {
          contentId: input.contentId,
          expectedStore: input.storeId,
          actualStore: content.storeId,
        },
      );
    }

    const lines = content.content.split('\n');
    const fromLine = Math.max(1, input.lineRange.fromLine);
    const toLine = Math.min(
      lines.length,
      input.lineRange.toLine || lines.length,
    );

    if (fromLine > lines.length) {
      throw new InvalidRangeError(fromLine, toLine, lines.length);
    }

    const selectedLines = lines.slice(fromLine - 1, toLine);
    return {
      content: selectedLines.join('\n'),
      lineRange: [fromLine, toLine],
    };
  } catch (error) {
    logger.error('Failed to read content', error, { input });
    throw error;
  }
}

async function similaritySearch(
  input: SimilaritySearchInput,
): Promise<{ results: SearchResult[] }> {
  const options = {
    topN: input.options?.topN || 5,
    threshold: input.options?.threshold || 0.1,
  };
  const results = await searchEngine.search(
    input.storeId,
    input.query,
    options,
  );
  logger.info('Similarity search completed', {
    storeId: input.storeId,
    query: input.query,
    resultCount: results.length,
  });
  return { results };
}

const fileStoreServer: WebMCPServer = {
  name: 'file-store',
  version: '1.0.0',
  description: 'File attachment and semantic search system using MCP protocol',
  tools,
  async callTool(name: string, args: unknown): Promise<unknown> {
    logger.debug('File store tool called', { name, args });
    if (!searchEngine.isReady()) await searchEngine.initialize();

    switch (name) {
      case 'createStore':
        return createStore(args as CreateStoreInput);
      case 'addContent':
        return addContent(args as AddContentInput);
      case 'listContent':
        return listContent(args as ListContentInput);
      case 'readContent':
        return readContent(args as ReadContentInput);
      case 'similaritySearch':
        return similaritySearch(args as SimilaritySearchInput);
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  },
};

export default fileStoreServer;
