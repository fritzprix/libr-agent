# MCP File Attachment System Development Plan

## Overview

파일 첨부 시스템을 MCP (Model Context Protocol) 기반으로 구현하여 효율적인 컨텍스트 관리와 의미적 검색을 제공합니다.

## Core Concept

- **Store 기반 관리**: 세션별로 독립적인 파일 저장소 생성
- **Chunking + Embedding**: 대용량 파일의 의미적 검색 지원
- **Context 최적화**: 전체 파일이 아닌 필요한 부분만 LLM 컨텍스트에 포함
- **MCP Protocol**: 표준 MCP 도구로 파일 관리 기능 제공

## Architecture

```text
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   React UI      │────│  MCP Client      │────│  MCP Server     │
│                 │    │  (Frontend)      │    │  (Rust)         │
├─────────────────┤    ├──────────────────┤    ├─────────────────┤
│ File Upload     │    │ Tool Calls       │    │ Content Store   │
│ Chat Interface  │    │ Context Manager  │    │ Embedding Gen   │
│ Search Results  │    │ Session Mapping  │    │ HNSW Index      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## MCP Server API Design

### Tools

#### 1. `createStore`

```typescript
// Creates a new content store
Input: { metadata?: { name, description, sessionId } }
Output: { storeId: string, createdAt: string }
```

#### 2. `addContent`

```typescript
// Adds file content with chunking and indexing
Input: {
  storeId: string,
  content: string,
  metadata: { filename, mimeType, size, uploadedAt }
}
Output: { contentId: string, chunkCount: number }
```

#### 3. `listContent`

```typescript
// Lists content summaries (first 10-20 lines)
Input: { storeId: string, pagination?, filters? }
Output: { contents: ContentSummary[] }
```

#### 4. `readContent`

```typescript
// Reads specific line ranges
Input: {
  storeId: string,
  contentId: string,
  lineRange: { fromLine: number, toLine?: number }
}
Output: { content: string, lineRange: [number, number] }
```

#### 5. `similaritySearch`

```typescript
// Semantic search across content
Input: {
  storeId: string,
  query: string,
  options?: { topN: number, threshold: number }
}
Output: { results: SearchResult[] }
```

## Data Structures

### ContentSummary

```typescript
interface ContentSummary {
  contentId: string;
  filename: string;
  mimeType: string;
  lineCount: number;
  preview: string; // First 10-20 lines
  chunkCount: number;
  size: number;
  uploadedAt: string;
}
```

### SearchResult

```typescript
interface SearchResult {
  contentId: string;
  filename: string;
  similarity: number;
  lineRange: [number, number];
  snippet: string;
  context?: string;
}
```

## Implementation Plan

### Phase 1: Core MCP Server (Rust)

- [ ] Basic store creation and management
- [ ] Content addition with line counting
- [ ] Simple content reading by line range
- [ ] File-based persistence

### Phase 2: Embedding & Search

- [ ] Content chunking algorithm (500-1000 chars per chunk)
- [ ] Embedding generation (all-MiniLM-L6-v2)
- [ ] HNSW index for similarity search
- [ ] Search result ranking and filtering

### Phase 3: Frontend Integration

- [ ] FileAttachmentManagerContext
- [ ] React components for file upload
- [ ] MCP client integration
- [ ] Chat context enhancement

### Phase 4: UI/UX Enhancement

- [ ] File preview components
- [ ] Search result display
- [ ] Attachment management interface
- [ ] Error handling and loading states

## Frontend Integration Points

### 1. Context Hierarchy

```text
MCPServerContext
├── FileAttachmentManagerProvider
    ├── ChatProvider
        ├── Chat Components
```

### 2. Chat Message Enhancement

```typescript
interface Message {
  // ... existing fields
  attachmentStoreId?: string;
  attachmentSummaries?: ContentSummary[];
}
```

### 3. System Prompt Integration

```typescript
const attachmentContext = `
## Available Files
${summaries.map((s) => `- ${s.filename} (${s.lineCount} lines): ${s.preview}`).join('\n')}

Use similarity search to find relevant content when needed.
`;
```

## Usage Workflow

### 1. Session Start

```typescript
// Auto-create store for new session
const storeId = await createStore({
  metadata: { sessionId, name: 'Session Files' },
});
```

### 2. File Upload

```typescript
// Add file content to store
const { contentId } = await addContent({
  storeId,
  content: fileText,
  metadata: { filename, mimeType, size },
});
```

### 3. Context Enhancement

```typescript
// Include file summaries in message context
const summaries = await listContent({ storeId });
message.attachmentSummaries = summaries.contents;
```

### 4. LLM Tool Usage

```typescript
// LLM searches when needed
const results = await similaritySearch({
  storeId,
  query: 'API configuration',
  options: { topN: 3 },
});

// LLM reads specific content
const content = await readContent({
  storeId,
  contentId: 'file_123',
  lineRange: { fromLine: 45, toLine: 60 },
});
```

## Benefits

- **Efficient Context**: Only relevant file portions in LLM context
- **Semantic Search**: Find relevant content intelligently
- **Scalable**: Handle large files without context window issues
- **Modular**: MCP protocol enables easy extension
- **Session Isolation**: Files scoped to specific conversations

## Technical Requirements

### Rust Dependencies

- `serde` - JSON serialization
- `uuid` - ID generation
- `candle-core` - ML embeddings
- `hnsw` - Vector similarity search
- `tokio` - Async runtime

### Frontend Dependencies

- Existing MCP client infrastructure
- File upload utilities
- React context providers

## File Structure

```text
src-tauri/src/mcp/
├── file_attachment_server.rs   # Main MCP server
├── content_store.rs           # Store management
├── embedding_service.rs       # Embedding generation
├── search_index.rs           # HNSW search index
└── chunking.rs               # Content chunking

src/context/
├── FileAttachmentManagerContext.tsx
└── FileAttachmentMCP.tsx

src/features/attachments/
├── FileUpload.tsx
├── AttachmentList.tsx
└── SearchResults.tsx
```

## Success Metrics

- [ ] Handle files up to 10MB efficiently
- [ ] Sub-second search response times
- [ ] Context window usage < 20% for file metadata
- [ ] Support 100+ files per session
- [ ] Accurate semantic search results (>80% relevance)

## Future Extensions

- Support for non-text files (images, PDFs)
- Multi-store search across sessions
- File version management
- Collaborative file sharing
- Advanced chunking strategies
