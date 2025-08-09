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

  chunkText(
    content: string,
  ): { text: string; startLine: number; endLine: number }[] {
    const lines = content.split('\n');
    const chunks: { text: string; startLine: number; endLine: number }[] = [];

    let currentChunk = '';
    let chunkStartLine = 0;
    let currentLine = 0;

    for (const line of lines) {
      currentChunk += line + '\n';
      currentLine++;

      if (currentChunk.length >= this.CHUNK_SIZE) {
        chunks.push({
          text: currentChunk.trim(),
          startLine: chunkStartLine,
          endLine: currentLine - 1,
        });

        const overlapLines = this.getOverlapLines(currentChunk);
        currentChunk = overlapLines;
        chunkStartLine = Math.max(
          0,
          currentLine - this.getLineCount(overlapLines),
        );
      }
    }

    if (currentChunk.trim()) {
      chunks.push({
        text: currentChunk.trim(),
        startLine: chunkStartLine,
        endLine: currentLine - 1,
      });
    }

    return chunks;
  }

  private getOverlapLines(chunk: string): string {
    const lines = chunk.split('\n');
    // A simple character-based overlap might be better here if lines are long
    let overlap = '';
    for (let i = lines.length - 1; i >= 0; i--) {
      if (overlap.length >= this.OVERLAP_SIZE) break;
      overlap = lines[i] + '\n' + overlap;
    }
    return overlap;
  }

  private getLineCount(text: string): number {
    return text.split('\n').length;
  }
}

class BM25SearchEngine implements ISearchEngine {
  private bm25Indexes = new Map<string, LocalBM25Instance>();
  private chunkMappings = new Map<string, Map<string, FileChunk>>();
  private isInitialized = false;

  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    logger.info('Initializing BM25 search engine');
    this.isInitialized = true;
  }

  async indexStore(storeId: string, chunks: FileChunk[]): Promise<void> {
    if (!this.isInitialized) await this.initialize();

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
    logger.info('BM25 index created', { storeId, chunkCount: chunks.length });
  }

  async search(
    storeId: string,
    query: string,
    options: SearchOptions,
  ): Promise<SearchResult[]> {
    const bm25 = this.bm25Indexes.get(storeId);
    const chunkMapping = this.chunkMappings.get(storeId);

    if (!bm25 || !chunkMapping) {
      logger.warn('No index found for store', { storeId });
      // Attempt to build index on the fly
      const chunks = await dbUtils.getFileChunksByStore(storeId);
      if (chunks.length > 0) {
        logger.info(`Index for store ${storeId} not found, building it now.`);
        await this.indexStore(storeId, chunks);
        return this.search(storeId, query, options);
      } else {
        return [];
      }
    }

    const processedQuery = this.preprocessText(query);
    const searchResults = bm25.search(processedQuery);

    const results: SearchResult[] = searchResults
      .slice(0, options.topN)
      .map((result: BM25SearchResult) => {
        const chunk = chunkMapping.get(result[0]);
        if (!chunk) return null;
        return {
          contentId: chunk.contentId,
          chunkId: chunk.id,
          context: chunk.text,
          lineRange: [chunk.startLine, chunk.endLine] as [number, number],
          score: result[1],
          relevanceType: 'keyword' as const,
        };
      })
      .filter(Boolean) as SearchResult[];

    logger.debug('BM25 search completed', {
      storeId,
      query: processedQuery,
      resultCount: results.length,
    });
    return results;
  }

  isReady(): boolean {
    return this.isInitialized;
  }

  async cleanup(): Promise<void> {
    this.bm25Indexes.clear();
    this.chunkMappings.clear();
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
  const contentId = `content_${Date.now()}`;
  const lines = input.content.split('\n');
  const summary = lines.slice(0, 20).join('\n');
  const now = new Date(input.metadata.uploadedAt);

  const content: FileContent = {
    id: contentId,
    storeId: input.storeId,
    filename: input.metadata.filename,
    mimeType: input.metadata.mimeType,
    size: input.metadata.size,
    uploadedAt: now,
    content: input.content,
    lineCount: lines.length,
    summary,
  };
  await dbService.fileContents.upsert(content);

  const chunksData = textChunker.chunkText(input.content);
  const chunks: FileChunk[] = chunksData.map((chunkData, index) => ({
    id: `${contentId}_chunk_${index}`,
    contentId,
    chunkIndex: index,
    text: chunkData.text,
    startLine: chunkData.startLine,
    endLine: chunkData.endLine,
  }));

  await dbService.fileChunks.upsertMany(chunks);
  await searchEngine.indexStore(input.storeId, chunks);

  logger.info('Content added with chunks', {
    contentId,
    filename: input.metadata.filename,
    chunkCount: chunks.length,
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
    uploadedAt: now,
  };
}

async function listContent(
  input: ListContentInput,
): Promise<{ contents: ContentSummary[] }> {
  const { storeId, pagination } = input;
  const pageNum = pagination?.offset
    ? Math.floor(pagination.offset / (pagination.limit || 50)) + 1
    : 1;
  const pageSize = pagination?.limit || 50;

  const page = await dbService.fileContents.getPage(pageNum, pageSize);
  const contentsInStore = page.items.filter((item) => item.storeId === storeId);

  const summaries: ContentSummary[] = contentsInStore.map((c) => ({
    storeId: c.storeId,
    contentId: c.id,
    filename: c.filename,
    mimeType: c.mimeType,
    size: c.size,
    lineCount: c.lineCount,
    preview: c.summary,
    uploadedAt: c.uploadedAt.toISOString(),
  }));

  return { contents: summaries };
}

async function readContent(
  input: ReadContentInput,
): Promise<{ content: string; lineRange: [number, number] }> {
  const content = await dbService.fileContents.read(input.contentId);
  if (!content || content.storeId !== input.storeId)
    throw new Error(`Content not found: ${input.contentId}`);

  const lines = content.content.split('\n');
  const fromLine = Math.max(0, input.lineRange.fromLine - 1);
  const toLine = Math.min(lines.length, input.lineRange.toLine || lines.length);

  const selectedLines = lines.slice(fromLine, toLine);
  return {
    content: selectedLines.join('\n'),
    lineRange: [fromLine + 1, toLine],
  };
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
