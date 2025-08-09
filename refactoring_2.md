# File Attachment MCP Server Module 구현 계획

## 📋 개요

mcp-plan.md의 파일 첨부 시스템을 Web Worker MCP 서버 모듈로 구현하여, WebMCPContext를 통해 브라우저에서 직접 사용할 수 있는 파일 관리 및 의미적 검색 기능을 제공합니다.

## 🎯 구현 목표

- **Web Worker 기반**: BM25 우선 구현, 향후 transformers.js 임베딩으로 확장
- **표준 MCP 인터페이스**: 기존 calculator, filesystem 모듈과 동일한 구조
- **IndexedDB 저장소**: 기존 db.ts 구조 활용한 영구 저장
- **점진적 검색 향상**: BM25 → 임베딩 → 하이브리드 검색 단계별 확장
- **추상화 설계**: 검색 엔진 교체 가능한 인터페이스 구조

## ⚠️ 주요 주의사항

> **Note: MVP 우선 접근법**
> - 1단계: BM25 기반 텍스트 검색으로 즉시 사용 가능한 MVP 구현
> - 2단계: transformers.js 임베딩을 백그라운드에서 선택적으로 로딩
> - 3단계: 하이브리드 검색(BM25 + 임베딩) 지원

> **Note: 메모리 및 성능 제약**
> - Web Worker 환경에서 50MB 메모리 제한 고려
> - 대용량 파일(10MB+)은 스트리밍 처리 필수
> - transformers.js 모델 로딩 시간(5-30초) 고려한 Progressive Enhancement

> **Note: 브라우저 호환성**
> - IndexedDB, Web Worker, WebAssembly 지원 확인 필요
> - transformers.js의 WebGPU 지원은 제한적 (폴백 전략 필요)
> - Cross-Origin 정책으로 인한 모델 로딩 이슈 대비

> **Note: 보안 고려사항**
> - 파일 크기 제한 (최대 50MB)
> - 악성 스크립트 패턴 검사
> - MIME 타입 검증 및 허용 목록 관리

## 🏗️ 아키텍처

```text
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  React UI       │    │  WebMCPContext   │    │  Web Worker     │
│  File Upload    │───▶│  Provider        │───▶│  file-store.ts  │
│  Search UI      │    │  Hook Integration│    │  + BM25/Embedding│
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                         │
                                                         ▼
                                               ┌─────────────────┐
                                               │  IndexedDB      │
                                               │  - Stores       │
                                               │  - Content      │
                                               │  - Chunks       │
                                               │  - Embeddings   │
                                               └─────────────────┘
```

### 검색 엔진 추상화 구조

```text
┌─────────────────────┐
│   ISearchEngine     │  ← 추상 인터페이스
│   Interface         │
└─────────────────────┘
           ▲
           │
    ┌──────┴──────┐
    ▼             ▼
┌─────────────┐ ┌─────────────┐
│ BM25Engine  │ │EmbeddingEngine│  ← 구현체들
│ (MVP)       │ │ (Enhanced)   │
└─────────────┘ └─────────────┘
```

## 📁 파일 구조 및 코드 배치

### 1. 데이터베이스 확장 (기존 활용)
```text
src/lib/db.ts                 # ✅ 기존 파일 확장
├── FileStore, FileContent, FileChunk 타입 추가
├── Version 5 스키마 추가 (fileStores, fileContents, fileChunks)
├── CRUD 서비스 추가 (기존 패턴 동일)
└── 파일 첨부용 유틸리티 함수 추가
```

### 2. MCP Server Module (새로 생성)
```text
src/lib/web-mcp/modules/
├── calculator.ts             # ✅ 기존
├── filesystem.ts             # ✅ 기존
└── file-store.ts             # 🆕 새로 구현
    ├── ISearchEngine 인터페이스
    ├── BM25SearchEngine (MVP)
    ├── EmbeddingSearchEngine (향후 확장)
    ├── AdaptiveSearchManager
    ├── TextChunker 유틸리티
    └── FileStoreServer MCP 구현
```

### 3. 타입 정의 위치 (기존 활용)
```text
src/models/
├── chat.ts                   # ✅ 기존 - AttachmentReference 이미 정의됨!
└── search-engine.ts          # 🆕 새로 생성 (검색 엔진 전용)
    ├── ISearchEngine 인터페이스
    ├── SearchResult, SearchOptions
    └── BM25/Embedding 관련 타입
```

**✅ 중요 발견**: `AttachmentReference` 타입이 `src/models/chat.ts`에 이미 완벽하게 정의되어 있습니다!
```typescript
// src/models/chat.ts (기존)
export interface AttachmentReference {
  storeId: string;         // MCP 파일 저장소 ID
  contentId: string;       // MCP 컨텐츠 ID
  filename: string;        // 원본 파일명
  mimeType: string;        // MIME 타입
  size: number;            // 파일 크기 (bytes)
  lineCount: number;       // 총 라인 수
  preview: string;         // 첫 10-20줄 미리보기
  uploadedAt: string;      // 업로드 시간 (ISO 8601)
  chunkCount?: number;     // 청크 개수 (검색용)
  lastAccessedAt?: string; // 마지막 접근 시간
}
```

### 4. Hook 및 Context (기존 패턴 활용)

```text
src/hooks/
└── use-file-attachment.ts    # 🆕 새로 생성
    ├── useFileStore (스토어 관리)
    ├── useFileUpload (파일 업로드) 
    └── useFileSearch (검색 기능)
    ※ AttachmentReference 타입은 기존 chat.ts에서 import

src/context/
└── WebMCPContext.tsx         # 🔄 기존 파일 확장
    ├── 기존 기능 유지
    └── 파일 첨부 관련 메서드 추가
```

### 5. UI 컴포넌트 (기존 패턴 활용)

```text
src/features/file-attachment/  # 🆕 새로 생성
├── components/
│   ├── FileUpload.tsx         # AttachmentReference 활용
│   ├── FileList.tsx           # AttachmentReference[] 표시
│   ├── SearchResults.tsx      # 검색 결과 UI
│   └── index.ts
├── hooks/
│   └── use-file-attachment.ts (위 hooks/에서 이동)
└── types/
    └── index.ts (chat.ts와 search-engine.ts에서 re-export)
```

## 🔧 구현 계획

### Phase 1: Core File Store Module

#### 1.1 MCP Server 인터페이스 정의

```typescript
import type { WebMCPServer, MCPTool } from '@/lib/mcp-types';

interface FileStoreServer extends WebMCPServer {
  name: 'file-store';
  version: '1.0.0';
  tools: [
    'createStore',
    'addContent', 
    'listContent',
    'readContent',
    'similaritySearch'
  ];
}
```

#### 1.2 MCP 도구 정의

```typescript
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
            sessionId: { type: 'string' }
          }
        }
      }
    }
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
            uploadedAt: { type: 'string' }
          },
          required: ['filename', 'mimeType', 'size', 'uploadedAt']
        }
      },
      required: ['storeId', 'content', 'metadata']
    }
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
          properties: {
            offset: { type: 'number' },
            limit: { type: 'number' }
          }
        }
      },
      required: ['storeId']
    }
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
            toLine: { type: 'number' }
          },
          required: ['fromLine']
        }
      },
      required: ['storeId', 'contentId', 'lineRange']
    }
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
            threshold: { type: 'number', default: 0.5 }
          }
        }
      },
      required: ['storeId', 'query']
    }
  }
];
```

### Phase 2: 검색 엔진 추상화 및 BM25 구현

#### 2.1 검색 엔진 추상화 인터페이스

```typescript
// 검색 엔진 공통 인터페이스
interface ISearchEngine {
  initialize(): Promise<void>;
  indexStore(storeId: string, chunks: ContentChunk[]): Promise<void>;
  search(storeId: string, query: string, options: SearchOptions): Promise<SearchResult[]>;
  isReady(): boolean;
  cleanup(): Promise<void>;
}

interface SearchOptions {
  topN: number;
  threshold?: number;
  searchType?: 'keyword' | 'semantic' | 'hybrid';
}

interface SearchResult {
  contentId: string;
  chunkId: string;
  context: string;
  lineRange: [number, number];
  score: number;
  relevanceType: 'keyword' | 'semantic';
}
```

#### 2.2 BM25 검색 엔진 구현 (MVP)

**파일 위치**: `src/lib/web-mcp/modules/file-store.ts`

```typescript
// 기존 db.ts의 타입들을 import
import { dbService, dbUtils, FileStore, FileContent, FileChunk } from '@/lib/db';
// 기존 chat.ts의 AttachmentReference 활용
import { AttachmentReference } from '@/models/chat';
import BM25 from 'wink-bm25-text-search';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BM25SearchEngine');

// 검색 엔진 공통 인터페이스 (파일 상단에 정의)
interface ISearchEngine {
  initialize(): Promise<void>;
  indexStore(storeId: string, chunks: FileChunk[]): Promise<void>;
  search(storeId: string, query: string, options: SearchOptions): Promise<SearchResult[]>;
  isReady(): boolean;
  cleanup(): Promise<void>;
}

interface SearchOptions {
  topN: number;
  threshold?: number;
  searchType?: 'keyword' | 'semantic' | 'hybrid';
}

interface SearchResult {
  contentId: string;
  chunkId: string;
  context: string;
  lineRange: [number, number];
  score: number;
  relevanceType: 'keyword' | 'semantic' | 'hybrid';
}

// BM25 검색 엔진 구현 (기존 db.ts 활용)
class BM25SearchEngine implements ISearchEngine {
  private bm25Indexes = new Map<string, any>(); // storeId별 BM25 인덱스
  private chunkMappings = new Map<string, Map<string, FileChunk>>(); // 빠른 조회용
  private isInitialized = false;
  
  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    
    logger.info('Initializing BM25 search engine');
    this.isInitialized = true;
  }
  
  async indexStore(storeId: string, chunks: FileChunk[]): Promise<void> {
    if (!this.isInitialized) {
      await this.initialize();
    }
    
    const bm25 = BM25();
    
    // BM25 설정 최적화
    bm25.defineConfig({
      fldWeights: { text: 1 },
      ovlpNormFactor: 0.5, // 문서 길이 정규화
      k1: 1.2,            // 단어 빈도 포화 매개변수
      b: 0.75             // 문서 길이 정규화 강도
    });
    
    // 청크 데이터 준비 및 인덱싱
    const chunkMapping = new Map<string, FileChunk>();
    
    chunks.forEach(chunk => {
      // 텍스트 전처리
      const processedText = this.preprocessText(chunk.text);
      
      bm25.addDoc({
        id: chunk.id, // 기존 db.ts의 id 필드 사용
        text: processedText
      });
      
      chunkMapping.set(chunk.id, chunk);
    });
    
    // 인덱스 통합 및 저장
    bm25.consolidate();
    this.bm25Indexes.set(storeId, bm25);
    this.chunkMappings.set(storeId, chunkMapping);
    
    logger.info('BM25 index created', { 
      storeId, 
      chunkCount: chunks.length 
    });
  }
  
  async search(storeId: string, query: string, options: SearchOptions): Promise<SearchResult[]> {
    const bm25 = this.bm25Indexes.get(storeId);
    const chunkMapping = this.chunkMappings.get(storeId);
    
    if (!bm25 || !chunkMapping) {
      logger.warn('No index found for store', { storeId });
      return [];
    }
    
    // 쿼리 전처리
    const processedQuery = this.preprocessText(query);
    
    // BM25 검색 수행
    const searchResults = bm25.search(processedQuery);
    
    // 결과 변환 및 정렬
    const results: SearchResult[] = searchResults
      .slice(0, options.topN)
      .map((result: any) => {
        const chunk = chunkMapping.get(result.id);
        if (!chunk) return null;
        
        return {
          contentId: chunk.contentId,
          chunkId: chunk.id,
          context: chunk.text,
          lineRange: [chunk.startLine, chunk.endLine] as [number, number],
          score: result.score,
          relevanceType: 'keyword' as const
        };
      })
      .filter(Boolean) as SearchResult[];
    
    logger.debug('BM25 search completed', {
      storeId,
      query: processedQuery,
      resultCount: results.length
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
  
  // 텍스트 전처리 (BM25 성능 향상)
  private preprocessText(text: string): string {
    return text
      .toLowerCase()
      .replace(/[^\w\s가-힣]/g, ' ') // 특수문자 제거, 한글 유지
      .replace(/\s+/g, ' ')          // 연속 공백 정리
      .trim();
  }
}
```

#### 2.3 임베딩 검색 엔진 (향후 확장용)

```typescript
class EmbeddingSearchEngine implements ISearchEngine {
  private embeddingService: EmbeddingService | null = null;
  private vectorSearch: VectorSearch | null = null;
  private isInitialized = false;
  
  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    
    try {
      logger.info('Initializing embedding search engine...');
      
      this.embeddingService = new EmbeddingService();
      await this.embeddingService.initialize();
      
      this.vectorSearch = new VectorSearch();
      
      this.isInitialized = true;
      logger.info('Embedding search engine ready');
    } catch (error) {
      logger.error('Failed to initialize embedding search engine', { error });
      throw error;
    }
  }
  
  async indexStore(storeId: string, chunks: ContentChunk[]): Promise<void> {
    if (!this.isInitialized) {
      await this.initialize();
    }
    
    // 임베딩이 없는 청크들에 대해 임베딩 생성
    const chunksNeedingEmbedding = chunks.filter(chunk => !chunk.embedding);
    
    if (chunksNeedingEmbedding.length > 0) {
      logger.info('Generating embeddings for chunks', { 
        count: chunksNeedingEmbedding.length 
      });
      
      const texts = chunksNeedingEmbedding.map(chunk => chunk.text);
      const embeddings = await this.embeddingService!.generateBatchEmbeddings(texts);
      
      // 임베딩을 청크에 저장 (IndexedDB 업데이트)
      for (let i = 0; i < chunksNeedingEmbedding.length; i++) {
        chunksNeedingEmbedding[i].embedding = embeddings[i];
        // TODO: IndexedDB에 임베딩 업데이트
      }
    }
  }
  
  async search(storeId: string, query: string, options: SearchOptions): Promise<SearchResult[]> {
    if (!this.isInitialized || !this.embeddingService || !this.vectorSearch) {
      throw new Error('Embedding search engine not initialized');
    }
    
    // 쿼리 임베딩 생성
    const queryEmbedding = await this.embeddingService.generateEmbedding(query);
    
    // 벡터 검색 수행
    const results = await this.vectorSearch.search(
      queryEmbedding,
      storeId,
      { 
        topN: options.topN, 
        threshold: options.threshold || 0.5 
      },
      dbInstance! // TODO: DI로 개선
    );
    
    return results.map(result => ({
      ...result,
      relevanceType: 'semantic' as const
    }));
  }
  
  isReady(): boolean {
    return this.isInitialized;
  }
  
  async cleanup(): Promise<void> {
    this.embeddingService = null;
    this.vectorSearch = null;
    this.isInitialized = false;
    logger.info('Embedding search engine cleaned up');
  }
}
```

#### 2.4 적응형 검색 매니저 (Progressive Enhancement)

```typescript
class AdaptiveSearchManager {
  private bm25Engine: BM25SearchEngine;
  private embeddingEngine: EmbeddingSearchEngine | null = null;
  private currentStrategy: 'bm25' | 'embedding' | 'hybrid' = 'bm25';
  
  constructor() {
    this.bm25Engine = new BM25SearchEngine();
    this.initializeEnhancedSearch(); // 백그라운드에서 초기화
  }
  
  async initialize(): Promise<void> {
    await this.bm25Engine.initialize();
    logger.info('Adaptive search manager initialized with BM25');
  }
  
  private async initializeEnhancedSearch(): Promise<void> {
    try {
      this.embeddingEngine = new EmbeddingSearchEngine();
      await this.embeddingEngine.initialize();
      
      // 임베딩 엔진이 준비되면 자동으로 하이브리드 모드로 전환
      this.currentStrategy = 'hybrid';
      logger.info('Enhanced search capabilities enabled');
    } catch (error) {
      logger.warn('Enhanced search not available, using BM25 only', { error });
    }
  }
  
  async indexStore(storeId: string, chunks: ContentChunk[]): Promise<void> {
    // BM25는 항상 인덱싱
    await this.bm25Engine.indexStore(storeId, chunks);
    
    // 임베딩 엔진이 준비되었다면 백그라운드에서 인덱싱
    if (this.embeddingEngine?.isReady()) {
      this.embeddingEngine.indexStore(storeId, chunks).catch(error => {
        logger.warn('Background embedding indexing failed', { error });
      });
    }
  }
  
  async search(storeId: string, query: string, options: SearchOptions): Promise<SearchResult[]> {
    switch (this.currentStrategy) {
      case 'bm25':
        return this.bm25Engine.search(storeId, query, options);
      
      case 'embedding':
        if (!this.embeddingEngine?.isReady()) {
          logger.warn('Embedding engine not ready, falling back to BM25');
          return this.bm25Engine.search(storeId, query, options);
        }
        return this.embeddingEngine.search(storeId, query, options);
      
      case 'hybrid':
        return this.hybridSearch(storeId, query, options);
      
      default:
        return this.bm25Engine.search(storeId, query, options);
    }
  }
  
  private async hybridSearch(storeId: string, query: string, options: SearchOptions): Promise<SearchResult[]> {
    const promises: Promise<SearchResult[]>[] = [
      this.bm25Engine.search(storeId, query, { ...options, topN: options.topN * 2 })
    ];
    
    if (this.embeddingEngine?.isReady()) {
      promises.push(
        this.embeddingEngine.search(storeId, query, { ...options, topN: options.topN * 2 })
      );
    }
    
    const [bm25Results, embeddingResults = []] = await Promise.all(promises);
    
    // 결과 융합 (가중치 기반)
    return this.fuseResults(bm25Results, embeddingResults, options.topN);
  }
  
  private fuseResults(bm25Results: SearchResult[], embeddingResults: SearchResult[], topN: number): SearchResult[] {
    const resultMap = new Map<string, SearchResult>();
    
    // BM25 결과 추가 (가중치: 0.4)
    bm25Results.forEach(result => {
      resultMap.set(result.chunkId, {
        ...result,
        score: result.score * 0.4
      });
    });
    
    // 임베딩 결과 추가 또는 점수 합산 (가중치: 0.6)
    embeddingResults.forEach(result => {
      const existing = resultMap.get(result.chunkId);
      if (existing) {
        existing.score += result.score * 0.6;
        existing.relevanceType = 'hybrid' as any;
      } else {
        resultMap.set(result.chunkId, {
          ...result,
          score: result.score * 0.6
        });
      }
    });
    
    // 점수 기준 정렬 및 상위 N개 반환
    return Array.from(resultMap.values())
      .sort((a, b) => b.score - a.score)
      .slice(0, topN);
  }
  
  getSearchCapabilities(): {
    bm25Available: boolean;
    embeddingAvailable: boolean;
    currentStrategy: string;
  } {
    return {
      bm25Available: this.bm25Engine.isReady(),
      embeddingAvailable: this.embeddingEngine?.isReady() || false,
      currentStrategy: this.currentStrategy
    };
  }
  
  setSearchStrategy(strategy: 'bm25' | 'embedding' | 'hybrid'): void {
    this.currentStrategy = strategy;
    logger.info('Search strategy changed', { strategy });
  }
}
```

#### 2.1 데이터베이스 스키마 설계

```typescript
interface Store {
  storeId: string;
  name: string;
  description?: string;
  sessionId?: string;
  createdAt: string;
  updatedAt: string;
}

interface Content {
  contentId: string;
  storeId: string;
  filename: string;
  mimeType: string;
  size: number;
  uploadedAt: string;
  content: string;
  lineCount: number;
  summary: string; // 첫 10-20줄
}

interface ContentChunk {
  chunkId: string;
  contentId: string;
  chunkIndex: number;
  text: string;
  startLine: number;
  endLine: number;
  embedding?: number[];
}
```

#### 2.2 IndexedDB 래퍼 클래스

```typescript
class FileStoreDB {
  private db: IDBDatabase | null = null;
  private readonly DB_NAME = 'FileAttachmentMCP';
  private readonly DB_VERSION = 1;
  
  async initialize(): Promise<void> {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(this.DB_NAME, this.DB_VERSION);
      
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        this.db = request.result;
        resolve();
      };
      
      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;
        
        // Stores 테이블
        if (!db.objectStoreNames.contains('stores')) {
          const storeOS = db.createObjectStore('stores', { keyPath: 'storeId' });
          storeOS.createIndex('sessionId', 'sessionId', { unique: false });
          storeOS.createIndex('createdAt', 'createdAt', { unique: false });
        }
        
        // Contents 테이블
        if (!db.objectStoreNames.contains('contents')) {
          const contentOS = db.createObjectStore('contents', { keyPath: 'contentId' });
          contentOS.createIndex('storeId', 'storeId', { unique: false });
          contentOS.createIndex('filename', 'filename', { unique: false });
          contentOS.createIndex('uploadedAt', 'uploadedAt', { unique: false });
        }
        
        // ContentChunks 테이블
        if (!db.objectStoreNames.contains('contentChunks')) {
          const chunkOS = db.createObjectStore('contentChunks', { keyPath: 'chunkId' });
          chunkOS.createIndex('contentId', 'contentId', { unique: false });
          chunkOS.createIndex('chunkIndex', 'chunkIndex', { unique: false });
        }
      };
    });
  }
  
  async createStore(store: Store): Promise<void> {
    const transaction = this.db!.transaction(['stores'], 'readwrite');
    const objectStore = transaction.objectStore('stores');
    await this.promisifyRequest(objectStore.add(store));
  }
  
  async addContent(content: Content): Promise<void> {
    const transaction = this.db!.transaction(['contents'], 'readwrite');
    const objectStore = transaction.objectStore('contents');
    await this.promisifyRequest(objectStore.add(content));
  }
  
  async addContentChunk(chunk: ContentChunk): Promise<void> {
    const transaction = this.db!.transaction(['contentChunks'], 'readwrite');
    const objectStore = transaction.objectStore('contentChunks');
    await this.promisifyRequest(objectStore.add(chunk));
  }
  
  private promisifyRequest<T>(request: IDBRequest<T>): Promise<T> {
    return new Promise((resolve, reject) => {
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }
}
```

### Phase 3: Embedding Service Integration

#### 3.1 transformers.js 기반 임베딩 서비스

```typescript
class EmbeddingService {
  private pipeline: any = null;
  private isInitialized = false;
  
  async initialize(): Promise<void> {
    if (this.isInitialized) return;
    
    try {
      // transformers.js는 Web Worker에서 동적 import 필요
      const { pipeline } = await import('@huggingface/transformers');
      
      this.pipeline = await pipeline(
        'feature-extraction',
        'Xenova/all-MiniLM-L6-v2',
        {
          quantized: true, // 성능 최적화
          device: 'webgpu', // WebGPU 사용 가능시
          progress_callback: (progress: any) => {
            console.log(`[Embedding] Model loading: ${Math.round(progress.progress * 100)}%`);
          }
        }
      );
      
      this.isInitialized = true;
    } catch (error) {
      console.error('[Embedding] Failed to initialize:', error);
      throw new Error(`Failed to initialize embedding service: ${error.message}`);
    }
  }
  
  async generateEmbedding(text: string): Promise<number[]> {
    if (!this.isInitialized || !this.pipeline) {
      throw new Error('Embedding service not initialized');
    }
    
    try {
      const result = await this.pipeline(text.trim());
      return Array.from(result.data);
    } catch (error) {
      console.error('[Embedding] Failed to generate embedding:', error);
      throw new Error(`Failed to generate embedding: ${error.message}`);
    }
  }
  
  async generateBatchEmbeddings(texts: string[]): Promise<number[][]> {
    const embeddings: number[][] = [];
    
    // 배치 처리로 메모리 사용량 제어
    const batchSize = 10;
    for (let i = 0; i < texts.length; i += batchSize) {
      const batch = texts.slice(i, i + batchSize);
      const batchEmbeddings = await Promise.all(
        batch.map(text => this.generateEmbedding(text))
      );
      embeddings.push(...batchEmbeddings);
    }
    
    return embeddings;
  }
}
```

#### 3.2 텍스트 청킹 전략

```typescript
class TextChunker {
  private readonly CHUNK_SIZE = 500; // 문자 수
  private readonly OVERLAP_SIZE = 50; // 겹치는 부분
  
  chunkText(content: string): { text: string; startLine: number; endLine: number }[] {
    const lines = content.split('\n');
    const chunks: { text: string; startLine: number; endLine: number }[] = [];
    
    let currentChunk = '';
    let chunkStartLine = 0;
    let currentLine = 0;
    
    for (const line of lines) {
      currentChunk += line + '\n';
      currentLine++;
      
      // 청크 크기 초과시 분할
      if (currentChunk.length >= this.CHUNK_SIZE) {
        chunks.push({
          text: currentChunk.trim(),
          startLine: chunkStartLine,
          endLine: currentLine - 1
        });
        
        // 오버랩을 위해 마지막 몇 줄 유지
        const overlapLines = this.getOverlapLines(currentChunk);
        currentChunk = overlapLines;
        chunkStartLine = Math.max(0, currentLine - this.getLineCount(overlapLines));
      }
    }
    
    // 마지막 청크 처리
    if (currentChunk.trim()) {
      chunks.push({
        text: currentChunk.trim(),
        startLine: chunkStartLine,
        endLine: currentLine - 1
      });
    }
    
    return chunks;
  }
  
  private getOverlapLines(chunk: string): string {
    const lines = chunk.split('\n');
    const overlapLineCount = Math.ceil(this.OVERLAP_SIZE / 50); // 대략적인 줄 수
    return lines.slice(-overlapLineCount).join('\n');
  }
  
  private getLineCount(text: string): number {
    return text.split('\n').length;
  }
}
```

### Phase 4: Vector Search Implementation

#### 4.1 브라우저 최적화된 벡터 검색

```typescript
class VectorSearch {
  private readonly SIMILARITY_THRESHOLD = 0.3;
  
  // 코사인 유사도 계산
  private cosineSimilarity(a: number[], b: number[]): number {
    if (a.length !== b.length) {
      throw new Error('Vectors must have the same length');
    }
    
    const dotProduct = a.reduce((sum, val, i) => sum + val * b[i], 0);
    const magnitudeA = Math.sqrt(a.reduce((sum, val) => sum + val * val, 0));
    const magnitudeB = Math.sqrt(b.reduce((sum, val) => sum + val * val, 0));
    
    if (magnitudeA === 0 || magnitudeB === 0) return 0;
    
    return dotProduct / (magnitudeA * magnitudeB);
  }
  
  async search(
    queryEmbedding: number[],
    storeId: string,
    options: { topN: number; threshold: number },
    db: FileStoreDB
  ): Promise<SearchResult[]> {
    // IndexedDB에서 해당 store의 모든 청크 조회
    const chunks = await this.getChunksByStore(storeId, db);
    
    // 임베딩이 없는 청크는 제외
    const chunksWithEmbeddings = chunks.filter(chunk => chunk.embedding);
    
    // 유사도 계산 및 정렬
    const similarities = chunksWithEmbeddings.map(chunk => {
      const similarity = this.cosineSimilarity(queryEmbedding, chunk.embedding!);
      return {
        chunkId: chunk.chunkId,
        contentId: chunk.contentId,
        text: chunk.text,
        startLine: chunk.startLine,
        endLine: chunk.endLine,
        similarity
      };
    })
    .filter(item => item.similarity >= options.threshold)
    .sort((a, b) => b.similarity - a.similarity)
    .slice(0, options.topN);
    
    // 결과 포맷팅
    return similarities.map(item => ({
      contentId: item.contentId,
      chunkId: item.chunkId,
      context: item.text,
      lineRange: [item.startLine, item.endLine],
      similarity: item.similarity
    }));
  }
  
  private async getChunksByStore(storeId: string, db: FileStoreDB): Promise<ContentChunk[]> {
    // 먼저 해당 store의 모든 content ID 조회
    const contents = await db.getContentsByStore(storeId);
    const contentIds = contents.map(content => content.contentId);
    
    // 각 content의 청크들 조회
    const allChunks: ContentChunk[] = [];
    for (const contentId of contentIds) {
      const chunks = await db.getChunksByContent(contentId);
      allChunks.push(...chunks);
    }
    
    return allChunks;
  }
}
```

### Phase 5: Complete Module Implementation

#### 5.1 메인 파일 구현 (file-store.ts)

```typescript
import type { WebMCPServer, MCPTool } from '@/lib/mcp-types';
import { AttachmentReference } from '@/models/chat'; // 기존 타입 활용
import { getLogger } from '@/lib/logger';

const logger = getLogger('FileStoreMCP');

// 타입 정의 (기존 AttachmentReference 활용)
interface CreateStoreInput {
  metadata?: {
    name?: string;
    description?: string;
    sessionId?: string;
  };
}

interface CreateStoreOutput {
  storeId: string;
  createdAt: string;
}

interface AddContentInput {
  storeId: string;
  content: string;
  metadata: {
    filename: string;
    mimeType: string;
    size: number;
    uploadedAt: string;
  };
}

// AttachmentReference를 활용한 출력 타입
interface AddContentOutput extends Omit<AttachmentReference, 'storeId' | 'contentId'> {
  storeId: string;
  contentId: string;
  chunkCount: number; // 추가 정보
}

// AttachmentReference를 기반으로 한 컨텐츠 요약
type ContentSummary = AttachmentReference;

interface SearchResult {
  contentId: string;
  chunkId?: string;
  context: string;
  lineRange?: [number, number];
  similarity?: number;
}

// 전역 인스턴스들
let dbInstance: FileStoreDB | null = null;
let embeddingService: EmbeddingService | null = null;
let textChunker: TextChunker | null = null;
let vectorSearch: VectorSearch | null = null;

const fileStoreServer: WebMCPServer = {
  name: 'file-store',
  version: '1.0.0',
  description: 'File attachment and semantic search system using MCP protocol',
  
  async listTools(): Promise<MCPTool[]> {
    return tools;
  },
  
  async callTool(name: string, args: unknown): Promise<unknown> {
    logger.debug('File store tool called', { name, args });
    
    try {
      // 필요한 서비스들 초기화
      await this.initializeServices();
      
      switch (name) {
        case 'createStore':
          return await this.createStore(args as CreateStoreInput);
        
        case 'addContent':
          return await this.addContent(args as AddContentInput);
        
        case 'listContent':
          return await this.listContent(args as any);
        
        case 'readContent':
          return await this.readContent(args as any);
        
        case 'similaritySearch':
          return await this.similaritySearch(args as any);
        
        default:
          throw new Error(`Unknown tool: ${name}`);
      }
    } catch (error) {
      logger.error('File store tool execution failed', { name, error });
      throw error;
    }
  },
  
  async initializeServices(): Promise<void> {
    if (!dbInstance) {
      dbInstance = new FileStoreDB();
      await dbInstance.initialize();
    }
    
    if (!embeddingService) {
      embeddingService = new EmbeddingService();
      await embeddingService.initialize();
    }
    
    if (!textChunker) {
      textChunker = new TextChunker();
    }
    
    if (!vectorSearch) {
      vectorSearch = new VectorSearch();
    }
  },
  
  async createStore(input: CreateStoreInput): Promise<CreateStoreOutput> {
    const storeId = `store_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const now = new Date().toISOString();
    
    const store: Store = {
      storeId,
      name: input.metadata?.name || 'Unnamed Store',
      description: input.metadata?.description,
      sessionId: input.metadata?.sessionId,
      createdAt: now,
      updatedAt: now
    };
    
    await dbInstance!.createStore(store);
    
    logger.info('Store created', { storeId, name: store.name });
    
    return {
      storeId,
      createdAt: now
    };
  },
  
  async addContent(input: AddContentInput): Promise<AddContentOutput> {
    const contentId = `content_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const lines = input.content.split('\n');
    const summary = lines.slice(0, 20).join('\n'); // 첫 20줄을 요약으로 사용
    
    // Content 저장
    const content: Content = {
      contentId,
      storeId: input.storeId,
      filename: input.metadata.filename,
      mimeType: input.metadata.mimeType,
      size: input.metadata.size,
      uploadedAt: input.metadata.uploadedAt,
      content: input.content,
      lineCount: lines.length,
      summary
    };
    
    await dbInstance!.addContent(content);
    
    // 텍스트 청킹
    const chunks = textChunker!.chunkText(input.content);
    
    // 각 청크에 대해 임베딩 생성 및 저장
    const chunkPromises = chunks.map(async (chunkData, index) => {
      const chunkId = `${contentId}_chunk_${index}`;
      
      try {
        const embedding = await embeddingService!.generateEmbedding(chunkData.text);
        
        const chunk: ContentChunk = {
          chunkId,
          contentId,
          chunkIndex: index,
          text: chunkData.text,
          startLine: chunkData.startLine,
          endLine: chunkData.endLine,
          embedding
        };
        
        await dbInstance!.addContentChunk(chunk);
      } catch (error) {
        logger.warn('Failed to generate embedding for chunk', { chunkId, error });
        
        // 임베딩 생성 실패해도 텍스트는 저장
        const chunk: ContentChunk = {
          chunkId,
          contentId,
          chunkIndex: index,
          text: chunkData.text,
          startLine: chunkData.startLine,
          endLine: chunkData.endLine
        };
        
        await dbInstance!.addContentChunk(chunk);
      }
    });
    
    await Promise.all(chunkPromises);
    
    logger.info('Content added with chunks', { 
      contentId, 
      filename: input.metadata.filename,
      chunkCount: chunks.length 
    });
    
    return {
      contentId,
      chunkCount: chunks.length
    };
  },
  
  async listContent(input: { storeId: string; pagination?: { offset?: number; limit?: number } }): Promise<{ contents: ContentSummary[] }> {
    const contents = await dbInstance!.getContentsByStore(input.storeId);
    
    // 페이지네이션 적용
    const offset = input.pagination?.offset || 0;
    const limit = input.pagination?.limit || 50;
    const paginatedContents = contents.slice(offset, offset + limit);
    
    const summaries: ContentSummary[] = paginatedContents.map(content => ({
      contentId: content.contentId,
      filename: content.filename,
      mimeType: content.mimeType,
      size: content.size,
      uploadedAt: content.uploadedAt,
      lineCount: content.lineCount,
      summary: content.summary
    }));
    
    return { contents: summaries };
  },
  
  async readContent(input: { 
    storeId: string; 
    contentId: string; 
    lineRange: { fromLine: number; toLine?: number } 
  }): Promise<{ content: string; lineRange: [number, number] }> {
    const content = await dbInstance!.getContentById(input.contentId);
    
    if (!content || content.storeId !== input.storeId) {
      throw new Error(`Content not found: ${input.contentId}`);
    }
    
    const lines = content.content.split('\n');
    const fromLine = Math.max(0, input.lineRange.fromLine - 1); // 1-based to 0-based
    const toLine = Math.min(lines.length - 1, (input.lineRange.toLine || lines.length) - 1);
    
    const selectedLines = lines.slice(fromLine, toLine + 1);
    
    return {
      content: selectedLines.join('\n'),
      lineRange: [fromLine + 1, toLine + 1] // 0-based to 1-based
    };
  },
  
  async similaritySearch(input: {
    storeId: string;
    query: string;
    options?: { topN?: number; threshold?: number }
  }): Promise<{ results: SearchResult[] }> {
    const options = {
      topN: input.options?.topN || 5,
      threshold: input.options?.threshold || 0.5
    };
    
    // 쿼리 임베딩 생성
    const queryEmbedding = await embeddingService!.generateEmbedding(input.query);
    
    // 벡터 검색 수행
    const results = await vectorSearch!.search(
      queryEmbedding,
      input.storeId,
      options,
      dbInstance!
    );
    
    logger.info('Similarity search completed', {
      storeId: input.storeId,
      query: input.query,
      resultCount: results.length
    });
    
    return { results };
  }
};

export default fileStoreServer;
```

## 🔌 WebMCPContext 통합

### Provider 설정 업데이트

```typescript
// App.tsx 업데이트
function App() {
  return (
    <WebMCPProvider
      workerPath="/workers/mcp-worker.js"
      servers={['calculator', 'filesystem', 'file-store']} // 🆕 추가
      autoLoad={true}
    >
      <UnifiedMCPProvider>
        <YourApp />
      </UnifiedMCPProvider>
    </WebMCPProvider>
  );
}
```

### Hook 사용 예시 (기존 AttachmentReference 활용)

```typescript
// 파일 첨부 컴포넌트에서 사용
import { useWebMCPTools } from '@/hooks/use-web-mcp';
import { AttachmentReference } from '@/models/chat'; // 기존 타입 활용

function FileAttachmentComponent() {
  const { executeCall } = useWebMCPTools();
  
  const uploadFile = async (file: File): Promise<AttachmentReference> => {
    try {
      // 1. 스토어 생성 (세션당 한 번)
      const { storeId } = await executeCall('file-store', 'createStore', {
        metadata: { sessionId: 'current-session', name: 'Session Files' }
      });
      
      // 2. 파일 내용 추가 (AttachmentReference 형식으로 반환)
      const content = await file.text();
      const result = await executeCall('file-store', 'addContent', {
        storeId,
        content,
        metadata: {
          filename: file.name,
          mimeType: file.type,
          size: file.size,
          uploadedAt: new Date().toISOString()
        }
      }) as AddContentOutput;
      
      // AttachmentReference 형식으로 변환
      const attachment: AttachmentReference = {
        storeId: result.storeId,
        contentId: result.contentId,
        filename: result.filename,
        mimeType: result.mimeType,
        size: result.size,
        lineCount: result.lineCount,
        preview: result.preview,
        uploadedAt: result.uploadedAt,
        chunkCount: result.chunkCount,
        lastAccessedAt: new Date().toISOString()
      };
      
      logger.info('File uploaded successfully', { 
        contentId: attachment.contentId, 
        filename: attachment.filename, 
        chunkCount: attachment.chunkCount 
      });
      
      return attachment;
    } catch (error) {
      logger.error('File upload failed', { filename: file.name, error });
      throw error;
    }
  };
  
  const searchContent = async (query: string, storeId: string) => {
    try {
      const { results } = await executeCall('file-store', 'similaritySearch', {
        storeId,
        query,
        options: { topN: 5, threshold: 0.5 }
      });
      
      return results;
    } catch (error) {
      logger.error('Content search failed', { query, error });
      throw error;
    }
  };
}
```

## 📦 Dependencies 및 설정

### package.json 업데이트 (BM25 우선)

```json
{
  "dependencies": {
    "wink-bm25-text-search": "^2.0.7"
  },
  "optionalDependencies": {
    "@huggingface/transformers": "^3.0.0"
  },
  "devDependencies": {
    "@types/web": "^0.0.99"
  }
}
```

### vite.config.ts 업데이트

```typescript
export default defineConfig({
  // ...existing config
  optimizeDeps: {
    include: ['wink-bm25-text-search'],
    exclude: ['@huggingface/transformers'] // 옵션으로 제외
  },
  worker: {
    format: 'es',
    plugins: () => [
      // transformers.js WASM 지원 (향후 확장용)
    ]
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'search-engines': ['wink-bm25-text-search'],
          'transformers': ['@huggingface/transformers'] // 옵션
        }
      }
    }
  },
  server: {
    headers: {
      // transformers.js 사용 시 필요한 헤더 (향후 확장용)
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Opener-Policy': 'same-origin'
    }
  }
});
```

### 설정 가능한 아키텍처

```typescript
interface FileStoreConfig {
  searchEngine: 'bm25' | 'embedding' | 'hybrid' | 'auto';
  enableProgressTracking: boolean;
  maxFileSize: number;
  chunkSize: number;
  memoryLimit: number;
  bm25Config?: {
    k1: number;
    b: number;
    ovlpNormFactor: number;
  };
  embeddingConfig?: {
    model: string;
    quantized: boolean;
    device: 'cpu' | 'webgpu';
  };
}

const DEFAULT_CONFIG: FileStoreConfig = {
  searchEngine: 'auto', // BM25로 시작, 임베딩 준비되면 하이브리드로 전환
  enableProgressTracking: true,
  maxFileSize: 50 * 1024 * 1024, // 50MB
  chunkSize: 500,
  memoryLimit: 50 * 1024 * 1024, // 50MB
  bm25Config: {
    k1: 1.2,
    b: 0.75,
    ovlpNormFactor: 0.5
  },
  embeddingConfig: {
    model: 'Xenova/all-MiniLM-L6-v2',
    quantized: true,
    device: 'webgpu'
  }
};
```

## 🧪 테스트 및 검증

### BM25 기반 통합 테스트

```typescript
export async function testFileStoreModuleBM25(): Promise<boolean> {
  const logger = getLogger('FileStoreTest');
  
  try {
    const proxy = createWebMCPProxy({ workerPath: '/workers/mcp-worker.js' });
    await proxy.initialize();
    
    // 서버 로드
    await proxy.loadServer('file-store');
    
    // 1. 스토어 생성 테스트
    const { storeId } = await proxy.callTool('file-store', 'createStore', {
      metadata: { 
        name: 'BM25 Test Store', 
        description: 'Testing BM25 search functionality' 
      }
    });
    
    logger.info('Store created successfully', { storeId });
    
    // 2. 다양한 컨텐츠 추가 (BM25 테스트용)
    const testDocuments = [
      {
        filename: 'ml_basics.txt',
        content: `
          Machine learning is a subset of artificial intelligence (AI).
          It involves training algorithms on data to make predictions.
          Supervised learning uses labeled data for training.
          Unsupervised learning finds patterns in unlabeled data.
        `
      },
      {
        filename: 'deep_learning.txt', 
        content: `
          Deep learning uses neural networks with multiple layers.
          Convolutional neural networks are great for image processing.
          Recurrent neural networks handle sequential data well.
          Transformers have revolutionized natural language processing.
        `
      },
      {
        filename: 'nlp_guide.txt',
        content: `
          Natural language processing helps computers understand text.
          Tokenization breaks text into words or subwords.
          Named entity recognition identifies important entities.
          Sentiment analysis determines emotional tone of text.
        `
      }
    ];
    
    const contentIds: string[] = [];
    
    for (const doc of testDocuments) {
      const { contentId, chunkCount } = await proxy.callTool('file-store', 'addContent', {
        storeId,
        content: doc.content,
        metadata: {
          filename: doc.filename,
          mimeType: 'text/plain',
          size: doc.content.length,
          uploadedAt: new Date().toISOString()
        }
      });
      
      contentIds.push(contentId);
      logger.info('Content added', { filename: doc.filename, contentId, chunkCount });
    }
    
    // 3. BM25 검색 테스트 (다양한 쿼리)
    const searchQueries = [
      { query: 'neural networks', expectedTerms: ['neural', 'networks', 'deep'] },
      { query: 'machine learning algorithms', expectedTerms: ['machine', 'learning', 'algorithms'] },
      { query: 'text processing NLP', expectedTerms: ['text', 'processing', 'nlp'] },
      { query: 'supervised unsupervised', expectedTerms: ['supervised', 'unsupervised'] }
    ];
    
    for (const { query, expectedTerms } of searchQueries) {
      const { results } = await proxy.callTool('file-store', 'similaritySearch', {
        storeId,
        query,
        options: { topN: 3, searchType: 'keyword' }
      });
      
      logger.info('BM25 search completed', { 
        query, 
        resultCount: results.length,
        topScore: results[0]?.score || 0
      });
      
      // 결과 검증: 관련 용어가 포함된 청크가 상위에 있는지 확인
      const hasRelevantResults = results.some(result => 
        expectedTerms.some(term => 
          result.context.toLowerCase().includes(term.toLowerCase())
        )
      );
      
      if (!hasRelevantResults && results.length > 0) {
        logger.warn('Search may not be working correctly', { query, results });
      }
    }
    
    // 4. 엣지 케이스 테스트
    const { results: emptyResults } = await proxy.callTool('file-store', 'similaritySearch', {
      storeId,
      query: 'nonexistent random terms xyz123',
      options: { topN: 5 }
    });
    
    logger.info('Empty query test', { 
      query: 'nonexistent terms',
      resultCount: emptyResults.length 
    });
    
    // 5. 성능 테스트 (검색 속도)
    const startTime = Date.now();
    
    await Promise.all([
      proxy.callTool('file-store', 'similaritySearch', {
        storeId, query: 'machine learning', options: { topN: 5 }
      }),
      proxy.callTool('file-store', 'similaritySearch', {
        storeId, query: 'deep networks', options: { topN: 5 }
      }),
      proxy.callTool('file-store', 'similaritySearch', {
        storeId, query: 'text processing', options: { topN: 5 }
      })
    ]);
    
    const searchTime = Date.now() - startTime;
    logger.info('Concurrent search performance', { searchTime, searchCount: 3 });
    
    // 검증: 기본 기능이 모두 동작하는지 확인
    const allTestsPassed = 
      storeId && 
      contentIds.length === testDocuments.length && 
      searchTime < 1000; // 3개 검색이 1초 이내
    
    logger.info('BM25 tests completed', { 
      success: allTestsPassed,
      searchTime,
      contentCount: contentIds.length
    });
    
    return allTestsPassed;
  } catch (error) {
    logger.error('BM25 file store test failed', error);
    return false;
  }
}

// Progressive Enhancement 테스트
export async function testProgressiveEnhancement(): Promise<boolean> {
  const logger = getLogger('ProgressiveTest');
  
  try {
    const proxy = createWebMCPProxy({ workerPath: '/workers/mcp-worker.js' });
    await proxy.initialize();
    await proxy.loadServer('file-store');
    
    // 검색 엔진 상태 확인
    const capabilities = await proxy.callTool('file-store', 'getSearchCapabilities', {});
    
    logger.info('Search capabilities', capabilities);
    
    // BM25는 항상 사용 가능해야 함
    if (!capabilities.bm25Available) {
      throw new Error('BM25 search engine should always be available');
    }
    
    // 임베딩 엔진은 선택적
    if (capabilities.embeddingAvailable) {
      logger.info('Enhanced search features available');
      
      // 하이브리드 검색 테스트
      const { results } = await proxy.callTool('file-store', 'similaritySearch', {
        storeId: 'test-store',
        query: 'test query',
        options: { searchType: 'hybrid' }
      });
      
      logger.info('Hybrid search test completed', { resultCount: results.length });
    } else {
      logger.info('Using BM25-only mode (normal for initial load)');
    }
    
    return true;
  } catch (error) {
    logger.error('Progressive enhancement test failed', error);
    return false;
  }
}
```

## 🚀 성능 최적화 및 메모리 관리

### 메모리 사용량 모니터링

```typescript
class MemoryManager {
  private readonly MAX_CHUNKS_IN_MEMORY = 1000;
  private readonly MAX_EMBEDDINGS_IN_MEMORY = 500;
  private chunkCache = new Map<string, ContentChunk>();
  private embeddingCache = new Map<string, number[]>();
  
  manageMemory(): void {
    if (this.chunkCache.size > this.MAX_CHUNKS_IN_MEMORY) {
      this.evictOldestChunks();
    }
    
    if (this.embeddingCache.size > this.MAX_EMBEDDINGS_IN_MEMORY) {
      this.evictOldestEmbeddings();
    }
  }
  
  private evictOldestChunks(): void {
    const entries = Array.from(this.chunkCache.entries());
    const toEvict = entries.slice(0, entries.length - this.MAX_CHUNKS_IN_MEMORY + 100);
    
    for (const [key] of toEvict) {
      this.chunkCache.delete(key);
    }
  }
  
  private evictOldestEmbeddings(): void {
    const entries = Array.from(this.embeddingCache.entries());
    const toEvict = entries.slice(0, entries.length - this.MAX_EMBEDDINGS_IN_MEMORY + 50);
    
    for (const [key] of toEvict) {
      this.embeddingCache.delete(key);
    }
  }
  
  getMemoryUsage(): { chunkCount: number; embeddingCount: number } {
    return {
      chunkCount: this.chunkCache.size,
      embeddingCount: this.embeddingCache.size
    };
  }
}
```

### 배치 처리 최적화

```typescript
class BatchProcessor {
  private readonly BATCH_SIZE = 10;
  private readonly BATCH_DELAY = 100; // ms
  
  async processBatch<T, R>(
    items: T[],
    processor: (item: T) => Promise<R>
  ): Promise<R[]> {
    const results: R[] = [];
    
    for (let i = 0; i < items.length; i += this.BATCH_SIZE) {
      const batch = items.slice(i, i + this.BATCH_SIZE);
      const batchResults = await Promise.all(
        batch.map(item => processor(item))
      );
      
      results.push(...batchResults);
      
      // 다음 배치 처리 전 지연
      if (i + this.BATCH_SIZE < items.length) {
        await this.delay(this.BATCH_DELAY);
      }
    }
    
    return results;
  }
  
  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}
```

## 📈 성공 메트릭스 및 모니터링

### 성능 메트릭스

- **✅ 파일 처리**: 10MB 파일을 5초 이내 처리
- **✅ 검색 성능**: 1000개 청크에서 100ms 이내 검색
- **✅ 메모리 사용**: 브라우저 100MB 이내 유지
- **✅ 정확도**: 의미적 검색 정확도 80% 이상

### 모니터링 대시보드

```typescript
interface PerformanceMetrics {
  totalStores: number;
  totalContents: number;
  totalChunks: number;
  averageSearchTime: number;
  memoryUsage: number;
  embeddingGenerationTime: number;
  indexingTime: number;
}

class MetricsCollector {
  private metrics: PerformanceMetrics = {
    totalStores: 0,
    totalContents: 0,
    totalChunks: 0,
    averageSearchTime: 0,
    memoryUsage: 0,
    embeddingGenerationTime: 0,
    indexingTime: 0
  };
  
  updateMetrics(update: Partial<PerformanceMetrics>): void {
    this.metrics = { ...this.metrics, ...update };
  }
  
  getMetrics(): PerformanceMetrics {
    return { ...this.metrics };
  }
  
  logMetrics(): void {
    console.table(this.metrics);
  }
}
```

## 🎯 결론 및 다음 단계 (기존 타입 활용)

이 구현 계획을 통해 Web Worker MCP 모듈로 완전한 파일 첨부 및 의미적 검색 시스템을 구현할 수 있습니다.

### 주요 장점

1. **기존 타입 재사용**: `AttachmentReference`가 이미 완벽하게 정의되어 일관성 확보
2. **브라우저 네이티브**: 외부 서버 의존성 없이 브라우저에서 완전 동작
3. **표준 MCP 인터페이스**: 기존 시스템과 완벽 통합
4. **확장 가능**: 새로운 검색 알고리즘이나 임베딩 모델 쉽게 교체
5. **성능 최적화**: 메모리 관리 및 배치 처리로 안정적 동작

### 🔗 기존 타입 통합 이점

```typescript
// src/models/chat.ts의 AttachmentReference를 그대로 활용
export interface Message {
  // ...기존 필드들
  attachments?: AttachmentReference[]; // ✅ 이미 정의됨!
}

// 새로운 파일 첨부 기능이 기존 채팅 시스템과 완벽 호환
const message: Message = {
  id: 'msg-1',
  content: '파일을 첨부했습니다.',
  attachments: [uploadedAttachment], // AttachmentReference 그대로 사용
  // ...
};
```

### MVP 단계별 구현 우선순위

```text
🎯 MVP Phase 1a: BM25 기반 검색 (즉시 사용 가능)
├── ✅ 기본 파일 저장 (IndexedDB)
├── ✅ 텍스트 청킹
├── ✅ BM25 인덱싱 및 검색
├── ✅ 키워드 기반 검색
└── ✅ 기본 에러 처리

🚀 MVP Phase 1b: UI 통합 및 사용성 개선
├── 📁 파일 업로드 컴포넌트
├── 🔍 검색 결과 표시 UI
├── 📊 진행 상황 모니터링
├── ⚠️  에러 처리 및 사용자 피드백
└── 🧪 기본 테스트 완료

🔬 Enhanced Phase 2a: 임베딩 검색 (선택적 로딩)
├── 🤖 transformers.js 백그라운드 로딩
├── 🧠 임베딩 생성 및 저장
├── 🔄 BM25 → 임베딩 점진적 전환
└── 📈 성능 모니터링

⚡ Enhanced Phase 2b: 하이브리드 검색 최적화
├── 🔀 BM25 + 임베딩 결과 융합
├── ⚙️  사용자 설정 검색 전략
├── 🎛️  가중치 조정 및 튜닝
└── 🏆 최적 검색 결과 제공

🌟 Advanced Phase 3: 확장 기능
├── 📄 다양한 파일 형식 지원
├── 🏷️  메타데이터 기반 필터링
├── 💾 검색 결과 캐싱
└── 🔗 검색 결과 연관성 분석
```

### 다음 단계 (기존 타입 기반)

1. **Phase 1a 구현**: 검색 엔진 타입 정의 및 BM25 Core Module
   - `src/models/search-engine.ts` 생성 (ISearchEngine, SearchOptions, SearchResult)
   - `wink-bm25-text-search` 통합 및 BM25SearchEngine 구현
   - 기존 `AttachmentReference` 타입과 연동하는 MCP 도구 구현

2. **Phase 1b 구현**: UI 통합 및 기존 채팅 시스템과 연결
   - WebMCPContext에 파일 첨부 기능 추가
   - `AttachmentReference[]`를 활용한 파일 업로드/검색 컴포넌트
   - `Message.attachments` 필드와 완전 호환되는 UI 구현

3. **Phase 2a 구현**: 임베딩 기능 점진적 추가
   - transformers.js 선택적 로딩
   - 백그라운드 임베딩 생성 및 기존 `FileChunk` 테이블과 연동
   - Progressive Enhancement 적용

4. **Phase 2b 최적화**: 하이브리드 검색 및 채팅 통합
   - BM25 + 임베딩 결과 융합 알고리즘
   - 채팅 컨텍스트에 파일 첨부 내용 자동 포함
   - `AttachmentReference` 기반 파일 관리 및 검색

5. **Phase 3 확장**: 기존 시스템 완전 통합
   - PDF, DOCX 등 다양한 파일 형식 지원
   - 세션별 파일 관리 (Session 테이블과 연동)
   - Assistant별 파일 접근 권한 관리

### 🚀 즉시 활용 가능한 기존 자산

- ✅ `AttachmentReference` 인터페이스 (완벽하게 정의됨)
- ✅ `Message.attachments` 필드 (채팅과 즉시 연동 가능)
- ✅ 기존 db.ts의 CRUD 패턴 (파일 저장 로직 확장됨)
- ✅ WebMCPContext 구조 (calculator/filesystem 패턴 참고)
- ✅ 로깅 시스템 (`@/lib/logger`) 및 shadcn/ui 컴포넌트
