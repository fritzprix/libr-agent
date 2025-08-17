import type { WebMCPServer, MCPTool } from '@/lib/mcp-types';
import type { JSONSchemaObject } from '@/lib/mcp-types'; // ADDED: JSON 스키마 타입을 명시적으로 가져옵니다.
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
import { WebMCPServerProxy } from '@/context/WebMCPContext';
import { ParserFactory } from './parsers/parser-factory';
import { ParserError } from './parsers/index';

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

const logger = createWorkerSafeLogger('content-store');

// File size limits (in bytes)
const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50MB
const MAX_CONTENT_LENGTH = 10 * 1024 * 1024; // 10MB text content

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

// FIX: readContent 함수에서 사용되므로 삭제하지 않습니다.
class ContentNotFoundError extends FileStoreError {
  constructor(contentId: string, storeId?: string) {
    super(`Content not found: ${contentId}`, 'CONTENT_NOT_FOUND', {
      contentId,
      storeId,
    });
  }
}

// FIX: readContent 함수에서 사용되므로 삭제하지 않습니다.
class InvalidRangeError extends FileStoreError {
  constructor(fromLine: number, toLine: number, totalLines: number) {
    super(
      `Invalid line range: ${fromLine}-${toLine} (total: ${totalLines})`,
      'INVALID_LINE_RANGE',
      { fromLine, toLine, totalLines },
    );
  }
}

interface ParseResult {
  content: string;
  filename: string;
  mimeType: string;
  size: number;
}

// Remove old parsing functions and use the new ParserFactory
async function parseRichFile(file: File): Promise<string> {
  try {
    logger.info('Starting file parsing', {
      filename: file.name,
      size: file.size,
      type: file.type,
    });

    const result = await ParserFactory.parseFile(file);

    logger.info('File parsing completed', {
      filename: file.name,
      contentLength: result.length,
      preview: result.substring(0, 200) + (result.length > 200 ? '...' : ''),
    });

    return result;
  } catch (error) {
    logger.error('File parsing failed', {
      filename: file.name,
      error:
        error instanceof Error
          ? {
              name: error.name,
              message: error.message,
              stack: error.stack,
            }
          : error,
    });

    if (error instanceof ParserError) {
      throw error;
    }
    throw error;
  }
}

async function parseFileFromUrl(
  fileUrl: string,
  metadata?: AddContentInput['metadata'],
): Promise<ParseResult> {
  try {
    const response = await fetch(fileUrl);
    if (!response.ok) {
      throw new FileStoreError('Failed to fetch blob URL', 'FETCH_FAILED', {
        fileUrl,
        status: response.status,
      });
    }
    const blob = await response.blob();

    // Validate file size
    if (blob.size > MAX_FILE_SIZE) {
      throw new FileStoreError(
        `File size exceeds limit: ${blob.size} bytes (max: ${MAX_FILE_SIZE})`,
        'FILE_TOO_LARGE',
        { fileSize: blob.size, maxSize: MAX_FILE_SIZE },
      );
    }

    const filename = metadata?.filename || 'unknown_file';
    const file = new File([blob], filename, { type: blob.type });

    const content = await parseRichFile(file);

    // Validate content length
    if (content.length > MAX_CONTENT_LENGTH) {
      throw new FileStoreError(
        `Content too large: ${content.length} characters (max: ${MAX_CONTENT_LENGTH})`,
        'CONTENT_TOO_LARGE',
        { contentLength: content.length, maxLength: MAX_CONTENT_LENGTH },
      );
    }

    return {
      content,
      filename: file.name,
      mimeType: file.type,
      size: file.size,
    };
  } catch (error) {
    logger.error('Failed to parse file from URL', error);
    if (error instanceof FileStoreError) throw error;
    throw new FileStoreError('File parsing failed', 'PARSE_FAILED', {
      fileUrl,
      originalError: error instanceof Error ? error.message : String(error),
    });
  }
}

// FIX: BM25 라이브러리가 반환하는 튜플 형태에 맞게 타입 수정 (TS2488 에러 해결)
type BM25SearchResult = [string, number];

interface LocalBM25Instance {
  defineConfig: (config: object) => void;
  addDoc: (doc: { id: string; text: string }) => void;
  consolidate: () => void;
  search: (query: string) => BM25SearchResult[];
}

class TextChunker {
  // ... (TextChunker implementation remains the same)
  private readonly CHUNK_SIZE = 500; // characters
  private readonly OVERLAP_SIZE = 50; // characters
  private readonly MIN_CHUNK_SIZE = 100; // minimum chunk size

  chunkText(
    content: string,
  ): { text: string; startLine: number; endLine: number }[] {
    const sentences = this.splitIntoSentences(content);
    if (sentences.length <= 1 && content.length <= this.CHUNK_SIZE) {
      return [
        { text: content, startLine: 1, endLine: this.countLines(content) },
      ];
    }

    const chunks: { text: string; startLine: number; endLine: number }[] = [];
    let currentChunk = '';
    let currentSentences: string[] = [];
    let chunkStartLine = 1;

    for (const sentence of sentences) {
      const sentenceWithSpace = currentChunk ? ` ${sentence}` : sentence;
      const potentialChunk = currentChunk + sentenceWithSpace;

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

        const { overlapText, overlapSentences } =
          this.createOverlap(currentSentences);
        currentChunk = overlapText;
        currentSentences = overlapSentences;
        chunkStartLine = Math.max(
          1,
          endLine - this.countLines(overlapText) + 2,
        );
      } else {
        currentChunk = potentialChunk;
        currentSentences.push(sentence);
      }
    }

    if (currentChunk.trim()) {
      const endLine = chunkStartLine + this.countLines(currentChunk) - 1;
      chunks.push({
        text: currentChunk.trim(),
        startLine: chunkStartLine,
        endLine,
      });
    }

    return chunks;
  }

  private splitIntoSentences(text: string): string[] {
    const sentences = text.match(/[^.!?]+[.!?]+["]?(\s+|$)/g);
    if (sentences) {
      return sentences.map((s) => s.trim()).filter(Boolean);
    }
    return [text];
  }

  private createOverlap(sentences: string[]): {
    overlapText: string;
    overlapSentences: string[];
  } {
    let overlapText = '';
    const overlapSentences: string[] = [];
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
      overlapSentences.unshift(sentence);
      overlapText = potentialOverlap;
    }
    return { overlapText, overlapSentences };
  }

  private countLines(text: string): number {
    return text ? (text.match(/\n/g) || []).length + 1 : 1;
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

  // ADDED: ISearchEngine 인터페이스를 만족시키기 위해 indexStore 메서드 구현 (TS2420 에러 해결)
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
      .replace(/[^\w\s가-힣]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
  }
}

// FIX: JSONSchemaObject에 맞게 스키마 정의 수정 (TS2741, TS2353 에러 해결)
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
    description:
      'Add file content by parsing a file URL or using pre-parsed text.',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: {
          type: 'string',
          description: 'ID of the store to add content to',
        },
        fileUrl: {
          type: 'string',
          description: 'Blob URL of the file to be parsed and added.',
        },
        content: {
          type: 'string',
          description: 'Pre-parsed text content (for backward compatibility).',
        },
        metadata: {
          type: 'object',
          properties: {
            filename: { type: 'string' },
            mimeType: { type: 'string' },
            size: { type: 'number' },
            uploadedAt: { type: 'string', format: 'date-time' },
          },
          description: 'File metadata. Partially optional when using fileUrl.',
        },
      },
      required: ['storeId'],
      oneOf: [{ required: ['fileUrl'] }, { required: ['content', 'metadata'] }],
    } as JSONSchemaObject, // HINT: oneOf를 사용하는 경우 타입 단언이 필요할 수 있습니다.
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
  fileUrl?: string;
  content?: string;
  metadata?: {
    filename?: string;
    mimeType?: string;
    size?: number;
    uploadedAt?: string;
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
  return { storeId: store.id, createdAt: now };
}

async function addContent(input: AddContentInput): Promise<AddContentOutput> {
  try {
    logger.info('Starting addContent', {
      storeId: input.storeId,
      hasFileUrl: !!input.fileUrl,
      hasContent: !!input.content,
      metadata: input.metadata,
    });

    const store = await dbService.fileStores.read(input.storeId);
    if (!store) {
      throw new StoreNotFoundError(input.storeId);
    }

    let finalContent: string;
    let fileMetadata: {
      filename: string;
      mimeType: string;
      size: number;
      uploadedAt: Date;
    };

    if (input.fileUrl) {
      logger.info('Parsing file from URL', { fileUrl: input.fileUrl });
      const parseResult = await parseFileFromUrl(input.fileUrl, input.metadata);
      finalContent = parseResult.content;
      fileMetadata = {
        filename: parseResult.filename,
        mimeType: parseResult.mimeType,
        size: parseResult.size,
        uploadedAt: new Date(),
      };
      logger.info('File URL parsing completed', {
        filename: parseResult.filename,
        contentLength: finalContent.length,
      });
    } else if (input.content && input.metadata?.filename) {
      logger.debug('Using pre-parsed content', {
        filename: input.metadata.filename,
        contentLength: input.content.length,
      });
      finalContent = input.content;
      fileMetadata = {
        filename: input.metadata.filename,
        mimeType: input.metadata.mimeType || 'text/plain',
        size: input.metadata.size || finalContent.length,
        uploadedAt: input.metadata.uploadedAt
          ? new Date(input.metadata.uploadedAt)
          : new Date(),
      };
    } else {
      throw new FileStoreError(
        'Either fileUrl or (content + metadata) must be provided',
        'MISSING_INPUT',
      );
    }

    const contentId = `content_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const lines = finalContent.split('\n');
    const summary = lines.slice(0, 20).join('\n');

    const content: FileContent = {
      id: contentId,
      storeId: input.storeId,
      filename: fileMetadata.filename,
      mimeType: fileMetadata.mimeType,
      size: fileMetadata.size,
      uploadedAt: fileMetadata.uploadedAt,
      content: finalContent,
      lineCount: lines.length,
      summary,
    };

    const chunksData = textChunker.chunkText(finalContent);
    const chunks: FileChunk[] = chunksData.map((chunkData, index) => ({
      id: `${contentId}_chunk_${index}`,
      contentId,
      chunkIndex: index,
      text: chunkData.text,
      startLine: chunkData.startLine,
      endLine: chunkData.endLine,
    }));

    // Save to database
    await dbService.fileContents.upsert(content);
    await dbService.fileChunks.upsertMany(chunks);

    // Add to search index
    await searchEngine.addToIndex(input.storeId, chunks);

    logger.info('Content added successfully', {
      contentId,
      filename: fileMetadata.filename,
      chunks: chunks.length,
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
      uploadedAt: fileMetadata.uploadedAt,
    };
  } catch (error) {
    logger.error('Failed to add content', {
      storeId: input.storeId,
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }
}

// FIX: 스텁(stub) 코드를 완전한 구현으로 대체 (TS6133, TS2355 에러 해결)
async function listContent(
  input: ListContentInput,
): Promise<{ contents: ContentSummary[]; total: number; hasMore: boolean }> {
  const { storeId, pagination } = input;
  const limit = Math.min(pagination?.limit || 50, 100);
  const offset = Math.max(pagination?.offset || 0, 0);

  try {
    const allPages = await dbService.fileContents.getPage(1, 1000);
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
    return { contents: summaries, total, hasMore };
  } catch (error) {
    logger.error('Failed to list content', error);
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
    logger.error('Failed to read content', error);
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
  return { results };
}

const fileStoreServer: WebMCPServer = {
  name: 'content-store',
  version: '1.1.0',
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

export interface ContentStoreServer extends WebMCPServerProxy {
  createStore(input: CreateStoreInput): Promise<CreateStoreOutput>;
  addContent(input: AddContentInput): Promise<AddContentOutput>;
  listContent(
    input: ListContentInput,
  ): Promise<{ contents: ContentSummary[]; total: number; hasMore: boolean }>;
  readContent(
    input: ReadContentInput,
  ): Promise<{ content: string; lineRange: [number, number] }>;
  similaritySearch(
    input: SimilaritySearchInput,
  ): Promise<{ results: SearchResult[] }>;
}

export default fileStoreServer;
