# Content-Store Web MCP to Rust Backend Migration Plan

**작성일**: 2025-09-21 21:00  
**작성자**: GitHub Copilot  
**버전**: 1.1.0 (Clean)  
**대상**: SynapticFlow Content-Store 모듈

## 🎯 작업의 목적

### 주요 목적

**성능 및 안정성 향상**을 위해 Web Worker 기반 Content-Store MCP 서버를 Rust Native Backend로 마이그레이션하여, 대용량 파일 처리 능력 확보 및 시스템 통합을 달성합니다.

- **성능 최적화**: JavaScript 파싱 → Rust Native 파싱으로 50MB+ 파일 처리 성능 극대화
- **아키텍처 통합**: 기존 Rust MCP Backend 인프라 활용으로 일관된 도구 관리 체계 구축
- **보안 강화**: 브라우저 샌드박스 제약 제거 및 시스템 레벨 파일 접근 권한 확보
- **확장성 확보**: Native 파일 시스템 및 고성능 검색 엔진(tantivy) 활용 기반 마련

### 비즈니스 가치

- AI Agent의 문서 분석 성능 향상 (현재 10MB → 목표 50MB+ 파일 지원)
- 통합된 MCP 도구 환경으로 개발자 UX 일관성 확보
- 고성능 BM25 검색 및 텍스트 청킹으로 지식 검색 품질 향상

## � 마이그레이션 전략 및 접근 방식

### **중요: Legacy 코드 처리 방안**

**본 마이그레이션은 "빅뱅" 방식으로 진행되며, 기존 Web Worker 기반 content-store 코드는 최종 단계에서 완전 삭제될 예정입니다.**

- **공존 코드 최소화**: 기존 코드와의 임시 공존을 위한 브릿지 코드나 호환성 레이어를 만들지 않습니다
- **직접 교체**: Rust 구현이 완료되는 즉시 Frontend에서 Web MCP → Rust MCP로 완전 전환
- **Legacy 정리**: 모든 테스트 통과 및 검증 완료 후, 다음 파일들을 완전 삭제:
  - `src/lib/web-mcp/modules/content-store/` (전체 디렉토리)
  - `src/hooks/use-web-mcp-server.ts` (Web MCP 전용 hook)
  - `src/lib/web-mcp/` 관련 모든 파일
  - `ResourceAttachmentContext.tsx`의 Web MCP 관련 로직

**이점**:

- 코드 복잡도 감소 및 유지보수성 향상
- 혼란스러운 공존 로직 제거
- 최종 아키텍처의 명확성 확보

## �📊 현재의 상태 / 문제점

### 현재 아키텍처 분석 (Web Worker 기반)

```text
Frontend (React)
    ↓ postMessage/Worker API
Web Worker (mcp-worker.ts)
    ↓ Dynamic Import
content-store module (src/lib/web-mcp/modules/content-store/)
    ├── server.ts        - WebMCPServer 구현 (989 lines)
    ├── parser.ts        - parseRichFile wrapper
    ├── chunker.ts       - 텍스트 청킹 (500자 + 50자 오버랩)
    ├── search.ts        - BM25SearchEngine 래퍼
    ├── types.ts         - 타입 정의
    └── index.ts         - 진입점

파싱 의존성:
    ├── mammoth          - DOCX 파싱 (JavaScript)
    ├── xlsx             - XLSX 파싱 (JavaScript)
    └── unpdf            - PDF 파싱 (JavaScript)
```

### 현재 도구 구현 상태 (검증됨)

**실제 5개 도구** (기존 계획서의 13개는 오류):

1. **`createStore`**: 메타데이터 기반 스토어 생성
   - Input: `metadata?: { name, description, sessionId }`
   - Output: `{ storeId: string, createdAt: Date }`

2. **`addContent`**: 파일 파싱 및 인덱싱
   - Input: `{ storeId, fileUrl?, content?, metadata? }`
   - Output: `{ contentId, chunkCount, uploadedAt, ... }`

3. **`listContent`**: 페이지네이션 지원 컨텐츠 목록
   - Input: `{ storeId, pagination?: { offset, limit } }`
   - Output: `{ contents: ContentSummary[], total, hasMore }`

4. **`readContent`**: 라인 범위 기반 컨텐츠 읽기
   - Input: `{ storeId, contentId, lineRange: { fromLine, toLine? } }`
   - Output: `{ content: string, lineRange: [number, number] }`

5. **`keywordSimilaritySearch`**: BM25 기반 키워드 검색
   - Input: `{ storeId, query, options?: { topN, threshold } }`
   - Output: `{ results: SearchResult[] }`

### 주요 문제점 및 제약사항

1. **성능 한계**
   - 브라우저 메모리 제한으로 대용량 파일(50MB+) 처리 불가
   - JavaScript 파싱 라이브러리의 성능 병목
   - Web Worker 간 postMessage 오버헤드

2. **의존성 복잡도**
   - mammoth(DOCX), xlsx.js(XLSX), unpdf(PDF) 등 JavaScript 라이브러리 의존
   - 번들 크기 증가 및 보안 업데이트 추적 부담

3. **아키텍처 분산**
   - Web MCP와 Rust MCP Backend의 이중 구조로 유지보수 복잡성
   - 도구 네이밍 불일치 (`content-store` vs `builtin_*__*` 패턴)

4. **기능 제약**
   - 브라우저 샌드박스로 인한 직접 파일 시스템 접근 불가
   - IndexedDB 기반 저장소의 용량 및 성능 제한

### 관련 코드 구조 및 동작 방식 (Birdeye View)

```text
현재 흐름:
ResourceAttachmentContext (useWebMCPServer)
    → mcp-worker.ts
    → content-store/server.ts (WebMCPServer)
    → IndexedDB + BM25SearchEngine

목표 흐름:
ResourceAttachmentContext (useRustMCPServer)
    → Tauri IPC
    → src-tauri/src/mcp/builtin/content_store.rs (BuiltinMCPServer)
    → File System + Tantivy Search
```

## 🎯 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

**기능 호환성** (필수):

- [ ] 기존 5개 도구의 입력/출력 스키마 100% 호환성 유지
- [ ] Frontend 코드 변경 < 10% (Hook 변경 및 서버명 업데이트만)
- [ ] 기존 IndexedDB 데이터 → 파일 시스템 마이그레이션 지원

**성능 개선** (필수):

- [ ] 50MB+ 파일 처리 가능 (현재 10MB 제한 해제)
- [ ] DOCX/XLSX/PDF 파싱 속도 2배 이상 향상
- [ ] BM25 검색 응답시간 < 100ms (대용량 컨텐츠 기준)

**아키텍처 통합** (필수):

- [ ] `builtin_contentstore__*` 네이밍으로 기존 Rust MCP 패턴 준수
- [ ] `BuiltinMCPServer` 트레이트 구현으로 기존 인프라 활용
- [ ] Tauri IPC를 통한 Frontend-Backend 통신 구현

**테스트 및 검증** (필수):

- [ ] 기존 Web Worker 기능과의 E2E 테스트 통과
- [ ] 크로스 플랫폼 (Windows/macOS/Linux) 동작 검증
- [ ] 메모리 누수 및 파일 핸들 정리 검증

### 데이터 흐름 개선

```text
Before: React → postMessage → Web Worker → content-store → IndexedDB
After:  React → Tauri IPC → Rust Backend → content_store → File System
```

## 📂 관련 코드의 구조 및 동작 방식 Summary

### 기존 Web MCP 아키텍처

**메인 서버 구현** (`src/lib/web-mcp/modules/content-store/server.ts`):

```typescript
const fileStoreServer: WebMCPServer = {
  name: 'content-store',
  version: '1.1.0',
  tools: [
    /* 5개 도구 정의 */
  ],
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // 도구별 분기 처리
    switch (name) {
      case 'createStore':
        return await createStore(args);
      case 'addContent':
        return await addContent(args);
      // ... 5개 도구 처리
    }
  },
};
```

**핵심 비즈니스 로직**:

- **파싱**: `parseRichFile()` → mammoth/xlsx/unpdf 라이브러리 호출
- **청킹**: `TextChunker` → 500자 + 50자 오버랩 방식
- **검색**: `BM25SearchEngine` → JavaScript 구현체
- **저장**: IndexedDB (`dbService.fileStores`, `dbService.fileContents`)

### 기존 Rust MCP 인프라 활용 포인트

**BuiltinMCPServer 트레이트** (`src-tauri/src/mcp/builtin/mod.rs`):

```rust
pub trait BuiltinMCPServer {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn get_tools(&self) -> Vec<MCPTool>;
    async fn call_tool(&self, name: &str, args: Value) -> Result<MCPResponse, String>;
}
```

**기존 활용 가능 컴포넌트**:

- `MCPTool`, `MCPResponse` 타입 정의
- `SessionManager` Arc 공유로 세션별 격리
- JSONSchema 헬퍼 메서드 (`JSONSchema::string()`, `JSONSchema::object()`)

## 🔧 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. Rust Backend ContentStore 서버 구현

#### `src-tauri/src/mcp/builtin/content_store/mod.rs` (신규)

```rust
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPTool, MCPResponse, JSONSchema};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ContentStoreServer {
    session_manager: Arc<crate::session::SessionManager>,
    // 파서 인스턴스들
}

impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        "contentstore" // Web Worker 'content-store' → Rust 'contentstore'
    }

    fn description(&self) -> &str {
        "File attachment and semantic search system with native performance"
    }

    fn get_tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "create_store".to_string(),
                description: "Create a new content store for file management".to_string(),
                input_schema: JSONSchema::object()
                    .property("metadata", JSONSchema::object()
                        .property("name", JSONSchema::string())
                        .property("description", JSONSchema::string())
                        .property("sessionId", JSONSchema::string())
                    ),
            },
            MCPTool {
                name: "add_content".to_string(),
                description: "Add and parse file content with chunking".to_string(),
                input_schema: JSONSchema::object()
                    .property("storeId", JSONSchema::string())
                    .property("fileUrl", JSONSchema::string())
                    .property("content", JSONSchema::string())
                    .property("metadata", JSONSchema::object()),
            },
            // ... 나머지 3개 도구
        ]
    }

    async fn call_tool(&self, name: &str, args: Value) -> Result<MCPResponse, String> {
        match name {
            "create_store" => self.create_store(args).await,
            "add_content" => self.add_content(args).await,
            "list_content" => self.list_content(args).await,
            "read_content" => self.read_content(args).await,
            "keyword_similarity_search" => self.keyword_similarity_search(args).await,
            _ => Err(format!("Unknown tool: {}", name)),
        }
    }
}
```

### 2. Native 파서 구현

#### `src-tauri/src/mcp/builtin/content_store/parsers.rs` (신규)

```rust
use docx_rs::*;  // 수정: docx-rust → docx-rs (올바른 크레이트명)
use calamine::{Reader, Xlsx, open_workbook};
use lopdf::Document;
use std::path::Path;

pub enum ParseResult {
    Text(String),
    Error(String),
}

pub struct DocumentParser;

impl DocumentParser {
    pub async fn parse_file(file_path: &Path, mime_type: &str) -> ParseResult {
        match mime_type {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Self::parse_docx(file_path).await
            }
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                Self::parse_xlsx(file_path).await
            }
            "application/pdf" => {
                Self::parse_pdf(file_path).await
            }
            "text/plain" => {
                Self::parse_text(file_path).await
            }
            _ => ParseResult::Error(format!("Unsupported MIME type: {}", mime_type)),
        }
    }

    async fn parse_docx(file_path: &Path) -> ParseResult {
        // docx-rs 크레이트 활용
        // 제한: 텍스트 추출만 지원, 필요시 zip/xml 파싱으로 보완
        match std::fs::read(file_path) {
            Ok(data) => {
                // docx-rs 구현
                ParseResult::Text("DOCX content extracted".to_string())
            }
            Err(e) => ParseResult::Error(e.to_string()),
        }
    }

    async fn parse_xlsx(file_path: &Path) -> ParseResult {
        // calamine 크레이트: XLSX 읽기/파싱에 매우 적합
        match open_workbook::<Xlsx<_>, _>(file_path) {
            Ok(mut workbook) => {
                let mut content = String::new();
                // 시트별 셀 데이터 추출
                ParseResult::Text(content)
            }
            Err(e) => ParseResult::Error(e.to_string()),
        }
    }

    async fn parse_pdf(file_path: &Path) -> ParseResult {
        // lopdf/pdf-extract: PDF 텍스트 추출에 적합
        match Document::load(file_path) {
            Ok(doc) => {
                // PDF 텍스트 추출 로직
                ParseResult::Text("PDF content extracted".to_string())
            }
            Err(e) => ParseResult::Error(e.to_string()),
        }
    }
}
```

### 3. 검색 엔진 구현

#### `src-tauri/src/mcp/builtin/content_store/search.rs` (신규)

```rust
use tantivy::*;  // BM25 및 고성능 텍스트 검색 지원
use std::path::PathBuf;

pub struct ContentSearchEngine {
    index: Index,
    schema: Schema,
}

impl ContentSearchEngine {
    pub fn new(index_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.add_text_field("chunk_id", STRING | STORED);
        let schema = schema_builder.build();

        let index = Index::create_in_dir(&index_dir, schema.clone())?;

        Ok(Self { index, schema })
    }

    pub async fn add_chunks(&self, store_id: &str, chunks: Vec<TextChunk>) -> Result<(), String> {
        let mut index_writer = self.index.writer(50_000_000)?;

        for chunk in chunks {
            let mut doc = Document::new();
            doc.add_text(self.schema.get_field("content")?, &chunk.text);
            doc.add_text(self.schema.get_field("chunk_id")?, &chunk.id);
            index_writer.add_document(doc)?;
        }

        index_writer.commit()?;
        Ok(())
    }

    pub async fn search_bm25(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(&self.index, vec![
            self.schema.get_field("content")?
        ]);
        let query = query_parser.parse_query(query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        // BM25 점수 기반 결과 반환
        Ok(vec![])
    }
}
```

### 4. Frontend Hook 마이그레이션

**중요**: 본 단계에서는 Web MCP와의 공존 로직을 만들지 않고, Rust MCP로 완전 교체합니다.

#### `src/hooks/use-rust-mcp-server.ts` (신규)

```typescript
import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { MCPResponse } from '@/lib/mcp-types';

export function useRustMCPServer<T>(serverName: string) {
  const [isReady, setIsReady] = useState(false);

  const callTool = useCallback(
    async (toolName: string, args: any): Promise<MCPResponse> => {
      const fullToolName = `builtin_${serverName}__${toolName}`;
      return await invoke('call_builtin_mcp_tool', {
        serverName,
        toolName,
        args: JSON.stringify(args),
      });
    },
    [serverName],
  );

  const server = {
    callTool,
    isReady: () => isReady,
  } as T;

  useEffect(() => {
    setIsReady(true);
  }, []);

  return { server, isReady };
}
```

#### `src/context/ResourceAttachmentContext.tsx` (수정)

**변경 전 (현재 코드)**:

```typescript
// 기존 Web MCP 우선 사용 + Rust MCP fallback 로직
const { server: rustServer } = useRustMCPServer<...>('content-store');
const { server: webServer } = useWebMCPServer<ContentStoreServer>('content-store');
const server = (preferRust && rustServer ? rustServer : webServer) as unknown as ContentStoreServer;
```

**변경 후 (직접 교체)**:

```typescript
// Rust MCP로 완전 교체
import { useRustMCPServer } from '@/hooks/use-rust-mcp-server';
const { server } = useRustMCPServer<ContentStoreServer>('contentstore');
```

**삭제될 코드**:

- `preferRust` 상태 및 관련 useEffect 로직
- `useWebMCPServer` import 및 사용
- 서버 선택 로직 `(preferRust && rustServer ? rustServer : webServer)`

### 5. Tauri Command 등록

#### `src-tauri/src/commands/mcp.rs` (수정)

```rust
#[tauri::command]
pub async fn call_builtin_mcp_tool(
    server_name: String,
    tool_name: String,
    args: String,
    state: tauri::State<'_, AppState>,
) -> Result<MCPResponse, String> {
    let session_manager = &state.session_manager;

    // ContentStore 서버 등록 확인
    match server_name.as_str() {
        "contentstore" => {
            let content_store = ContentStoreServer::new(session_manager.clone());
            let args_value: serde_json::Value = serde_json::from_str(&args)
                .map_err(|e| format!("Invalid JSON args: {}", e))?;
            content_store.call_tool(&tool_name, args_value).await
        }
        _ => Err(format!("Unknown builtin server: {}", server_name)),
    }
}
```

## 🧩 재사용 가능한 연관 코드

### 기존 Rust MCP 인프라

**파일 경로**: `src-tauri/src/mcp/builtin/`

- **주요 기능**: `BuiltinMCPServer` 트레이트, `MCPResponse` 타입
- **인터페이스**: 기존 workspace 서버와 동일한 패턴 활용
- **재사용 가능**: JSONSchema 헬퍼, 에러 처리, 세션 관리

**파일 경로**: `src-tauri/src/session.rs`

- **주요 기능**: 세션별 격리, 작업 디렉토리 관리
- **인터페이스**: `SessionManager` Arc 공유로 멀티 스레드 안전성
- **재사용 가능**: 기존 MCP 서버들과 동일한 세션 격리 방식

### Frontend MCP 통합 시스템

**파일 경로**: `src/features/tools/index.tsx`

- **주요 기능**: `builtin_*__*` 네이밍 스키마, 도구 라우팅
- **인터페이스**: `executeTool` 메서드, 통합 에러 처리
- **재사용 가능**: 기존 builtin tools와 동일한 호출 패턴

**파일 경로**: `src/lib/mcp-types.ts`

- **주요 기능**: `MCPTool`, `MCPResponse`, `MCPError` 타입 정의
- **인터페이스**: JSON-RPC 표준 준수, TypeScript 타입 안전성
- **재사용 가능**: Web MCP와 Rust MCP 간 타입 호환성

### 데이터베이스 및 파일 시스템

**활용 예정**: SQLite (sqlx 크레이트)

- **주요 기능**: 메타데이터 저장, 트랜잭션 지원
- **인터페이스**: 비동기 쿼리, 마이그레이션 지원
- **적합성**: 경량화된 로컬 DB, 크로스 플랫폼 지원

**활용 예정**: File System API (Rust std)

- **주요 기능**: 네이티브 파일 I/O, 경로 처리
- **인터페이스**: tokio::fs 비동기 작업
- **적합성**: 대용량 파일 처리, 시스템 권한 활용

## 🧪 Test Code 추가 및 수정 필요 부분에 대한 가이드

### 1. Rust Backend 단위 테스트

#### `src-tauri/src/mcp/builtin/content_store/tests/parser_tests.rs`

```rust
#[cfg(test)]
mod parser_tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_text_file_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!\nSecond line.").unwrap();

        let result = DocumentParser::parse_file(&file_path, "text/plain").await;
        match result {
            ParseResult::Text(content) => {
                assert_eq!(content, "Hello, World!\nSecond line.");
            }
            ParseResult::Error(e) => panic!("Parsing failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_docx_parsing() {
        // docx-rs를 이용한 DOCX 파싱 테스트
        // 실제 DOCX 파일 생성 후 파싱 검증
    }

    #[tokio::test]
    async fn test_xlsx_parsing() {
        // calamine을 이용한 XLSX 파싱 테스트
        // 셀 데이터 및 수식 추출 검증
    }
}
```

#### `src-tauri/src/mcp/builtin/content_store/tests/search_tests.rs`

```rust
#[cfg(test)]
mod search_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bm25_search_accuracy() {
        let temp_dir = TempDir::new().unwrap();
        let search_engine = ContentSearchEngine::new(temp_dir.path().to_path_buf()).unwrap();

        // 테스트 청크 추가
        let chunks = vec![
            TextChunk {
                id: "chunk1".to_string(),
                text: "Rust programming language is fast and safe".to_string(),
                line_range: (1, 5),
            },
            TextChunk {
                id: "chunk2".to_string(),
                text: "JavaScript is widely used for web development".to_string(),
                line_range: (6, 10),
            },
        ];

        search_engine.add_chunks("store1", chunks).await.unwrap();

        // BM25 검색 테스트
        let results = search_engine.search_bm25("Rust programming", 5).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].chunk_id, "chunk1");
    }
}
```

### 2. Frontend 통합 테스트

#### `src/test/integration/rust-mcp-migration.test.ts`

```typescript
import { render, screen, waitFor } from '@testing-library/react';
import { describe, test, expect, beforeEach } from 'vitest';
import { ResourceAttachmentProvider } from '@/context/ResourceAttachmentContext';

describe('Rust MCP Migration', () => {
  beforeEach(() => {
    // Mock Tauri invoke
    global.__TAURI__ = {
      invoke: vi.fn(),
    };
  });

  test('should maintain createStore API compatibility', async () => {
    const mockResponse = {
      jsonrpc: '2.0',
      result: {
        storeId: 'store_123',
        createdAt: '2025-09-21T21:00:00Z',
      },
    };

    vi.mocked(global.__TAURI__.invoke).mockResolvedValue(mockResponse);

    // useRustMCPServer hook 테스트
    const { result } = renderHook(() => useRustMCPServer('contentstore'));

    const response = await result.current.server.callTool('create_store', {
      metadata: { name: 'Test Store' },
    });

    expect(response).toMatchObject({
      jsonrpc: '2.0',
      result: {
        storeId: expect.any(String),
        createdAt: expect.any(String),
      },
    });
  });

  test('should handle file parsing with same interface', async () => {
    // addContent 도구 호환성 테스트
    const fileData = 'Test file content';
    const mockResponse = {
      jsonrpc: '2.0',
      result: {
        contentId: 'content_123',
        chunkCount: 1,
        uploadedAt: '2025-09-21T21:00:00Z',
      },
    };

    vi.mocked(global.__TAURI__.invoke).mockResolvedValue(mockResponse);

    const { result } = renderHook(() => useRustMCPServer('contentstore'));

    const response = await result.current.server.callTool('add_content', {
      storeId: 'store_123',
      content: fileData,
      metadata: { filename: 'test.txt', mimeType: 'text/plain' },
    });

    expect(response.result.contentId).toBeDefined();
    expect(response.result.chunkCount).toBeGreaterThan(0);
  });
});
```

### 3. 크로스 플랫폼 테스트

#### `src-tauri/src/tests/cross_platform_tests.rs`

```rust
#[cfg(test)]
mod cross_platform_tests {
    use super::*;

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_windows_file_paths() {
        // Windows 경로 처리 테스트
        let file_path = PathBuf::from(r"C:\temp\test.docx");
        // 경로 정규화 및 파싱 테스트
    }

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn test_macos_file_paths() {
        // macOS 경로 처리 테스트
        let file_path = PathBuf::from("/tmp/test.docx");
        // HFS+ 파일명 인코딩 테스트
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_linux_file_paths() {
        // Linux 경로 처리 테스트
        let file_path = PathBuf::from("/tmp/test.docx");
        // UTF-8 파일명 처리 테스트
    }
}
```

### 4. 성능 벤치마크 테스트

#### `src-tauri/benches/content_store_benchmarks.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn benchmark_docx_parsing(c: &mut Criterion) {
    c.bench_function("docx_parsing_10mb", |b| {
        b.iter(|| {
            // 10MB DOCX 파일 파싱 벤치마크
            // JavaScript mammoth vs Rust docx-rs 성능 비교
        });
    });
}

fn benchmark_bm25_search(c: &mut Criterion) {
    c.bench_function("bm25_search_1000_chunks", |b| {
        b.iter(|| {
            // 1000개 청크에서 BM25 검색 성능 측정
            // JavaScript vs Rust tantivy 성능 비교
        });
    });
}

criterion_group!(benches, benchmark_docx_parsing, benchmark_bm25_search);
criterion_main!(benches);
```

## 📋 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 필수 수정 파일 목록

1. **`src-tauri/Cargo.toml`** - 의존성 추가
2. **`src-tauri/src/mcp/builtin/content_store/`** - 전체 모듈 신규 생성
3. **`src/hooks/use-rust-mcp-server.ts`** - 신규 Hook 구현
4. **`src/context/ResourceAttachmentContext.tsx`** - Hook 교체
5. **`src-tauri/src/commands/mcp.rs`** - ContentStore 서버 등록

### 주요 의존성 추가 (src-tauri/Cargo.toml)

```toml
[dependencies]
# 기존 의존성...

# 파일 파싱
docx-rs = "0.4"           # DOCX 파싱 (정정: docx-rust가 아님)
calamine = "0.22"         # XLSX 파싱
lopdf = "0.31"            # PDF 파싱
# pdf-extract = "0.7"     # 대안 PDF 파서

# 검색 엔진
tantivy = "0.21"          # BM25 텍스트 검색

# 데이터베이스
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite"] }

# 비동기 처리
tokio = { version = "1.0", features = ["full"] }

[dev-dependencies]
tempfile = "3.8"          # 테스트용 임시 파일
criterion = "0.5"         # 성능 벤치마크
```

---

## 🗑️ Phase 4: Legacy 코드 정리 (최종 단계)

### 모든 테스트 통과 및 검증 완료 후에 실행

#### Web MCP 관련 파일들

- `src/lib/web-mcp/modules/content-store/` (전체 디렉토리)
- `src/hooks/use-web-mcp-server.ts`
- `src/lib/web-mcp/` (전체 디렉토리)

#### Context 파일 내 불필요 코드

- `src/context/ResourceAttachmentContext.tsx` 내 다음 코드 제거:
  - `preferRust` 상태 및 관련 useEffect
  - `useWebMCPServer` import
  - `listBuiltinServers` import 및 호출
  - 서버 선택 로직 `(preferRust && rustServer ? rustServer : webServer)`

#### 기타 관련 파일들

- `src/lib/web-mcp/` 관련 모든 파일
- 테스트 파일에서 Web MCP 관련 테스트 케이스 제거

### 정리 작업 체크리스트

- [ ] 모든 단위 테스트 통과
- [ ] E2E 테스트 통과 (파일 업로드/검색/삭제)
- [ ] 크로스 플랫폼 테스트 완료 (Windows/macOS/Linux)
- [ ] 성능 벤치마크 검증 (50MB+ 파일 처리)
- [ ] 메모리 누수 및 파일 핸들 정리 검증
- [ ] **Legacy 코드 완전 삭제**
- [ ] **의존성 정리** (mammoth, xlsx, unpdf 등 JavaScript 라이브러리 제거)
- [ ] 문서 업데이트 (Web MCP 관련 내용 제거)

---

**📋 다음 단계**: 본 계획 검토 및 승인 후, Phase 1부터 단계적 구현을 시작합니다. 기존 Web Worker 기능과의 병렬 운영을 통해 안정성을 확보하면서 점진적으로 마이그레이션을 진행합니다.
