import type { WebMCPServer, MCPTool, MCPResponse } from '@/lib/mcp-types';
import { extractStructuredContent } from '@/lib/mcp-types';
import { createMCPStructuredResponse } from '@/lib/mcp-response-utils';
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

// Utility functions for better formatting
function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function getFileExtension(filename: string): string {
  const lastDotIndex = filename.lastIndexOf('.');
  return lastDotIndex !== -1
    ? filename.substring(lastDotIndex + 1).toLowerCase()
    : 'unknown';
}

function getRelativeTime(date: Date): string {
  const now = new Date();
  const diffInSeconds = Math.floor((now.getTime() - date.getTime()) / 1000);

  if (diffInSeconds < 60) return 'just now';
  if (diffInSeconds < 3600) return `${Math.floor(diffInSeconds / 60)}m ago`;
  if (diffInSeconds < 86400) return `${Math.floor(diffInSeconds / 3600)}h ago`;
  if (diffInSeconds < 604800)
    return `${Math.floor(diffInSeconds / 60400)}d ago`;
  if (diffInSeconds < 2592000)
    return `${Math.floor(diffInSeconds / 604800)}w ago`;
  return `${Math.floor(diffInSeconds / 2592000)}mo ago`;
}

function getMimeTypeDescription(mimeType: string): string {
  const descriptions: Record<string, string> = {
    'text/plain': 'Plain Text',
    'text/html': 'HTML Document',
    'text/css': 'CSS Stylesheet',
    'text/javascript': 'JavaScript',
    'text/markdown': 'Markdown',
    'application/pdf': 'PDF Document',
    'application/msword': 'Word Document',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document':
      'Word Document',
    'application/vnd.ms-excel': 'Excel Spreadsheet',
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet':
      'Excel Spreadsheet',
    'application/vnd.ms-powerpoint': 'PowerPoint Presentation',
    'application/vnd.openxmlformats-officedocument.presentationml.presentation':
      'PowerPoint Presentation',
    'image/jpeg': 'JPEG Image',
    'image/png': 'PNG Image',
    'image/gif': 'GIF Image',
    'image/webp': 'WebP Image',
    'application/json': 'JSON File',
    'application/xml': 'XML File',
  };
  return descriptions[mimeType] || mimeType;
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
export interface ListContentOutput {
  contents: ContentSummary[];
  total: number;
  hasMore: boolean;
}
export interface ReadContentOutput {
  content: string;
  lineRange: [number, number];
}
export interface KeywordSimilaritySearchOutput {
  results: SearchResult[];
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
    description:
      'Create a new content store for organized file management and search. Each store acts as an isolated container with automatic metadata tracking, session grouping, and unique ID generation.',
    inputSchema: {
      type: 'object',
      properties: {
        metadata: {
          type: 'object',
          properties: {
            name: {
              type: 'string',
              description:
                'Human-readable name for the store (e.g., "Project Documents", "Research Papers")',
            },
            description: {
              type: 'string',
              description:
                "Optional detailed description of the store's purpose",
            },
            sessionId: {
              type: 'string',
              description: 'Session identifier for grouping related stores',
            },
          },
        },
      },
    },
  },
  {
    name: 'addContent',
    description:
      'Add and process file content with automatic parsing, chunking, and indexing. Supports various file formats (PDF, DOCX, TXT, etc.) via URL or direct text input. Features automatic duplicate detection, content hashing, and BM25 search index generation. Files are chunked into ~500 character segments with 50-character overlap for optimal search performance.',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: {
          type: 'string',
          description: 'ID of the target store to add content to',
        },
        fileUrl: {
          type: 'string',
          description:
            'URL of the file to parse and add. Supports blob:, file:, http:, https:, and data: URLs. Maximum file size: 50MB.',
        },
        content: {
          type: 'string',
          description:
            'Pre-parsed text content for direct input (alternative to fileUrl). Maximum length: 10MB. Requires metadata.filename when used.',
        },
        metadata: {
          type: 'object',
          properties: {
            filename: {
              type: 'string',
              description:
                'Original filename with extension. Required when using content parameter.',
            },
            mimeType: {
              type: 'string',
              description:
                'MIME type of the file (e.g., "text/plain", "application/pdf"). Auto-detected if not provided.',
            },
            size: {
              type: 'number',
              description:
                'File size in bytes. Auto-calculated if not provided.',
            },
            uploadedAt: {
              type: 'string',
              format: 'date-time',
              description:
                'Upload timestamp in ISO format. Defaults to current time.',
            },
          },
          description:
            'File metadata. When using fileUrl, most fields are auto-detected. When using content, filename is required.',
        },
      },
      required: ['storeId'],
      oneOf: [{ required: ['fileUrl'] }, { required: ['content', 'metadata'] }],
    } as JSONSchemaObject,
  },
  {
    name: 'listContent',
    description:
      'Retrieve a paginated list of content summaries within a store. Returns metadata including filename, size, line count, preview text, and upload timestamps for efficient content browsing.',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: {
          type: 'string',
          description: 'ID of the store to list content from',
        },
        pagination: {
          type: 'object',
          properties: {
            offset: {
              type: 'number',
              description: 'Number of items to skip (default: 0, min: 0)',
            },
            limit: {
              type: 'number',
              description:
                'Maximum number of items to return (default: 50, max: 100)',
            },
          },
          description: 'Pagination parameters for large content lists',
        },
      },
      required: ['storeId'],
    },
  },
  {
    name: 'readContent',
    description:
      'Read specific line ranges from stored content. Useful for previewing sections, extracting snippets, or reading large files incrementally without loading entire content into memory.',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: {
          type: 'string',
          description: 'ID of the store containing the content',
        },
        contentId: {
          type: 'string',
          description: 'ID of the specific content to read',
        },
        lineRange: {
          type: 'object',
          properties: {
            fromLine: {
              type: 'number',
              description: 'Starting line number (1-based indexing, inclusive)',
            },
            toLine: {
              type: 'number',
              description:
                'Ending line number (1-based indexing, inclusive). If omitted, reads to end of file.',
            },
          },
          required: ['fromLine'],
          description: 'Line range specification for targeted content reading',
        },
      },
      required: ['storeId', 'contentId', 'lineRange'],
    },
  },
  {
    name: 'keywordSimilaritySearch',
    description:
      'Perform advanced keyword-based similarity search using the BM25 (Best Matching 25) algorithm. Searches across all content chunks within a store, ranking results by relevance score based on term frequency and document frequency. Supports multi-keyword queries and returns chunk-level results with context.',
    inputSchema: {
      type: 'object',
      properties: {
        storeId: {
          type: 'string',
          description: 'ID of the store to search within',
        },
        query: {
          type: 'string',
          description:
            'Search query with keywords separated by spaces. Supports multiple terms for complex queries.',
        },
        options: {
          type: 'object',
          properties: {
            topN: {
              type: 'number',
              default: 5,
              description:
                'Maximum number of results to return (default: 5, recommended range: 1-50)',
            },
            threshold: {
              type: 'number',
              default: 0.5,
              description:
                'Minimum relevance score threshold for filtering results (default: 0.5, range: 0.0-1.0)',
            },
          },
          description:
            'Search configuration options for result filtering and ranking',
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
): Promise<MCPResponse<CreateStoreOutput>> {
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
  return createMCPStructuredResponse<CreateStoreOutput>(
    `Store created with ID: ${store.id}`,
    {
      storeId: store.id,
      createdAt: now,
    },
  );
}

async function addContent(
  input: AddContentInput,
): Promise<MCPResponse<AddContentOutput>> {
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
      return createMCPStructuredResponse<AddContentOutput>(
        `Content already exists with ID: ${existingContent.id}`,
        {
          storeId: input.storeId,
          contentId: existingContent.id,
          filename: existingContent.filename,
          mimeType: existingContent.mimeType,
          size: existingContent.size,
          lineCount: existingContent.lineCount,
          preview: existingContent.summary,
          chunkCount: existingChunks.length,
          uploadedAt: existingContent.uploadedAt,
        },
      );
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

    return createMCPStructuredResponse(`Content added with ID: ${contentId}`, {
      storeId: input.storeId,
      contentId,
      filename: content.filename,
      mimeType: content.mimeType,
      size: content.size,
      lineCount: content.lineCount,
      preview: content.summary,
      chunkCount: chunks.length,
      uploadedAt: fileMetadata.uploadedAt,
    });
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
): Promise<MCPResponse<ListContentOutput>> {
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
    const pageInfo =
      offset > 0
        ? ` (showing ${offset + 1}-${offset + summaries.length} of ${total})`
        : ` (total: ${total})`;

    const formattedSummaries = await Promise.all(
      summaries.map(async (s) => {
        const extension = getFileExtension(s.filename);
        const sizeFormatted = formatFileSize(s.size);
        const timeAgo = getRelativeTime(new Date(s.uploadedAt));
        const typeDescription = getMimeTypeDescription(s.mimeType);

        // Get chunk count for this content
        const chunks = await dbUtils.getFileChunksByContent(s.contentId);
        const chunkCount = chunks.length;

        return `📄 ${s.filename}
   • Extension: .${extension}
   • Size: ${sizeFormatted} (${s.size} bytes)
   • Lines: ${s.lineCount}
   • Type: ${typeDescription}
   • Chunks: ${chunkCount}
   • Uploaded: ${timeAgo}
   • Preview: ${s.preview?.substring(0, 100)}${s.preview && s.preview.length > 100 ? '...' : ''}`;
      }),
    );

    return createMCPStructuredResponse<ListContentOutput>(
      `Found ${summaries.length} content items${pageInfo}${hasMore ? ' - more available' : ''}\n\n${formattedSummaries.join('\n\n')}`,
      { contents: summaries, total, hasMore },
    );
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
): Promise<MCPResponse<ReadContentOutput>> {
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

    // Format content with line numbers and range information
    const formattedContent = selectedLines
      .map((line, index) => {
        const lineNumber = fromLine + index;
        const paddedLineNumber = lineNumber.toString().padStart(4, ' ');
        return `${paddedLineNumber}| ${line}`;
      })
      .join('\n');

    const totalLines = lines.length;
    const selectedLineCount = toLine - fromLine + 1;
    const rangeInfo = `Lines ${fromLine}-${toLine} of ${totalLines} (${selectedLineCount} lines)`;

    return createMCPStructuredResponse(
      `Content retrieved: ${rangeInfo}\n\n${formattedContent}`,
      {
        content: selectedLines.join('\n'),
        lineRange: [fromLine, toLine],
      },
    );
  } catch (error) {
    logger.error('Failed to read content', error);
    throw error;
  }
}

async function keywordSimilaritySearch(
  input: KeywordSimilaritySearchInput,
): Promise<MCPResponse<KeywordSimilaritySearchOutput>> {
  const options = {
    topN: input.options?.topN || 5,
    threshold: input.options?.threshold || 0.0,
  };

  const results = await searchEngine.search(
    input.storeId,
    input.query,
    options,
  );

  // Format search results with detailed information
  const formattedResults = results.map((result, index) => {
    const scorePercent = (result.score * 100).toFixed(1);
    const lineInfo = `Lines ${result.lineRange[0]}-${result.lineRange[1]}`;
    const relevanceIcon =
      result.relevanceType === 'keyword'
        ? '🔍'
        : result.relevanceType === 'semantic'
          ? '🧠'
          : '🔗';

    return `📄 **Result ${index + 1}** (${relevanceIcon} ${result.relevanceType})
   • **File**: ${result.filename || 'Unknown'}
   • **Score**: ${scorePercent}% relevance
   • **Location**: ${lineInfo}
   • **Chunk ID**: ${result.chunkId}
   • **Content ID**: ${result.contentId}
   • **Context**:
${result.context
  .split('\n')
  .map((line) => `     ${line}`)
  .join('\n')}`;
  });

  const searchSummary = `🔍 **Search Summary**
   • **Query**: "${input.query}"
   • **Store ID**: ${input.storeId}
   • **Options**: Top ${options.topN} results, threshold ${options.threshold}
   • **Results Found**: ${results.length} matches

📋 **Search Results**`;

  const fullMessage =
    results.length > 0
      ? `${searchSummary}\n\n${formattedResults.join('\n\n---\n\n')}`
      : `${searchSummary}\n\n❌ No results found matching your query.`;

  return createMCPStructuredResponse(fullMessage, { results });
}

const fileStoreServer: WebMCPServer = {
  name: 'content-store',
  version: '1.1.0',
  description: 'File attachment and semantic search system using MCP protocol',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    logger.debug('File store tool called', { name, args });
    if (!searchEngine.isReady()) await searchEngine.initialize();
    switch (name) {
      case 'createStore': {
        const result = await createStore(args as CreateStoreInput);
        return result;
      }
      case 'addContent': {
        const result = await addContent(args as AddContentInput);
        return result;
      }
      case 'listContent': {
        const result = await listContent(args as ListContentInput);
        return result;
      }
      case 'readContent': {
        const result = await readContent(args as ReadContentInput);
        return result;
      }
      case 'keywordSimilaritySearch': {
        const result = await keywordSimilaritySearch(
          args as KeywordSimilaritySearchInput,
        );
        return result;
      }
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
      const result: MCPResponse<ListContentOutput> = await listContent({
        storeId: sessionStore.storeId,
      });

      // Extract structured content using type-safe helper
      const structuredContent = extractStructuredContent(result);
      if (
        !structuredContent?.contents ||
        structuredContent.contents.length === 0
      ) {
        return '# Attached Files\nNo files currently attached to this session.';
      }

      const attachedResources = structuredContent.contents
        .map((c: ContentSummary) =>
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
  listContent(input: ListContentInput): Promise<ListContentOutput>;
  readContent(input: ReadContentInput): Promise<ReadContentOutput>;
  keywordSimilaritySearch(
    input: KeywordSimilaritySearchInput,
  ): Promise<KeywordSimilaritySearchOutput>;
}

export default fileStoreServer;
