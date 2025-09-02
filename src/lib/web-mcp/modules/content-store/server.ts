import type { WebMCPServer, MCPTool, MCPResponse } from '@/lib/mcp-types';
import { normalizeToolResult } from '@/lib/mcp-types';
import type { JSONSchemaObject } from '@/lib/mcp-types';
import type { ServiceContextOptions } from '@/features/tools';
import {
  dbService,
  dbUtils,
  FileChunk,
  FileContent,
  FileStore,
} from '@/lib/db';
import { AttachmentReference } from '@/models/chat';
import type { SearchResult } from '@/models/search-engine';
import { WebMCPServerProxy } from '@/hooks/use-web-mcp-server';
import { computeContentHash } from '@/lib/content-hash';
import { BM25SearchEngine } from '../bm25/bm25-search-engine';

// Import from submodules
import { logger } from './logger';
import { parseRichFile } from './parser';

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

interface ParseResult {
  content: string;
  filename: string;
  mimeType: string;
  size: number;
}

// Tool input/output interfaces
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
export interface KeywordSimilaritySearchInput {
  storeId: string;
  query: string;
  options?: { topN?: number; threshold?: number };
}
export type ContentSummary = AttachmentReference;

class TextChunker {
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
    const sentences = text.match(/[^.!?]+[.!?]+["']?(\s+|$)/g);
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

// Tool schema definitions
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
    } as JSONSchemaObject,
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
    name: 'keywordSimilaritySearch',
    description:
      'Keyword-based similarity search across content using BM25 algorithm',
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

const searchEngine = new BM25SearchEngine();
const textChunker = new TextChunker();

// Tool implementation functions
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
      storeIdType: typeof input.storeId,
      hasFileUrl: !!input.fileUrl,
      hasContent: !!input.content,
      metadata: input.metadata,
      inputKeys: Object.keys(input),
    });

    // Validate storeId
    if (!input.storeId || typeof input.storeId !== 'string') {
      throw new FileStoreError(
        `Invalid storeId: expected string, got ${typeof input.storeId} (${input.storeId})`,
        'INVALID_STORE_ID',
        { storeId: input.storeId, storeIdType: typeof input.storeId },
      );
    }

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

    // Calculate content hash for duplicate detection
    const contentHash = await computeContentHash(finalContent);
    logger.info('Content hash calculated', {
      contentHash,
      contentLength: finalContent.length,
    });

    // Check for duplicate content in the same store
    const existingContent = await dbService.fileContents.findByHashAndStore(
      contentHash,
      input.storeId,
    );
    if (existingContent) {
      logger.info('Duplicate content found, returning existing', {
        existingContentId: existingContent.id,
        filename: existingContent.filename,
        contentHash,
      });

      // Return existing content information without creating new entries
      const existingChunks = await dbUtils.getFileChunksByContent(
        existingContent.id,
      );
      return {
        storeId: input.storeId,
        contentId: existingContent.id,
        filename: existingContent.filename,
        mimeType: existingContent.mimeType,
        size: existingContent.size,
        lineCount: existingContent.lineCount,
        preview: existingContent.summary,
        chunkCount: existingChunks.length,
        uploadedAt: existingContent.uploadedAt,
      };
    }

    const contentId = `content_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`;
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
      contentHash,
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
  logger.info('Reading content', { input });
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

async function keywordSimilaritySearch(
  input: KeywordSimilaritySearchInput,
): Promise<{ results: SearchResult[] }> {
  const options = {
    topN: input.options?.topN || 5,
    threshold: input.options?.threshold || 0.0, // 임시로 0으로 변경하여 테스트
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
  async callTool(name: string, args: unknown): Promise<MCPResponse> {
    logger.debug('File store tool called', { name, args });
    if (!searchEngine.isReady()) await searchEngine.initialize();
    switch (name) {
      case 'createStore':
        return normalizeToolResult(
          await createStore(args as CreateStoreInput),
          'createStore',
        );
      case 'addContent':
        return normalizeToolResult(
          await addContent(args as AddContentInput),
          'addContent',
        );
      case 'listContent':
        return normalizeToolResult(
          await listContent(args as ListContentInput),
          'listContent',
        );
      case 'readContent':
        return normalizeToolResult(
          await readContent(args as ReadContentInput),
          'readContent',
        );
      case 'keywordSimilaritySearch':
        return normalizeToolResult(
          await keywordSimilaritySearch(args as KeywordSimilaritySearchInput),
          'keywordSimilaritySearch',
        );
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  },
  async getServiceContext(options?: ServiceContextOptions): Promise<string> {
    try {
      const { sessionId } = options || {};

      if (!sessionId || typeof sessionId !== 'string') {
        logger.debug(
          'No valid sessionId in getServiceContext - session may be transitioning',
          {
            sessionId,
            sessionIdType: typeof sessionId,
          },
        );
        return '# Attached Files\nNo active session available.';
      }

      const sessionStore = await dbService.sessions.read(sessionId);
      if (!sessionStore?.storeId) {
        return '# Attached Files\nNo files currently attached to this session.';
      }

      // Get contents for this specific session's store
      const result = await listContent({ storeId: sessionStore.storeId });

      if (!result.contents || result.contents.length === 0) {
        return '# Attached Files\nNo files currently attached to this session.';
      }

      const attachedResources = result.contents
        .map((c) =>
          JSON.stringify({
            storeId: c.storeId,
            contentId: c.contentId,
            preview: c.preview,
            filename: c.filename,
            type: c.mimeType,
            size: c.size,
          }),
        )
        .join('\n');

      return `# Attached Files\n${attachedResources}`;
    } catch (error) {
      logger.error('Failed to build content-store service context', {
        sessionId: options?.sessionId,
        error,
      });
      return '# Attached Files\nError loading attached files for this session.';
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
  keywordSimilaritySearch(
    input: KeywordSimilaritySearchInput,
  ): Promise<{ results: SearchResult[] }>;
}

export default fileStoreServer;
