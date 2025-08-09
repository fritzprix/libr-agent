# 파일 첨부 시스템 통합 작업 현황 및 계획

## 📋 현재 완료된 작업 (refactoring_2_summary.md 기반)

### ✅ Backend Infrastructure (완료)
1. **Database Extension (`src/lib/db.ts`)**
   - IndexedDB schema v5 확장
   - FileStore, FileContent, FileChunk 테이블 추가
   - CRUD 서비스 및 유틸리티 함수 구현

2. **MCP Server Module (`src/lib/web-mcp/modules/file-store.ts`)**
   - BM25SearchEngine 클래스 구현
   - 5개 MCP 도구 구현 (createStore, addContent, listContent, readContent, similaritySearch)
   - wink-bm25-text-search 라이브러리 통합

3. **Type Definitions (`src/models/search-engine.ts`)**
   - ISearchEngine, SearchOptions, SearchResult 인터페이스 정의
   - 기존 AttachmentReference (chat.ts) 타입 활용

4. **Dependencies & Integration**
   - wink-bm25-text-search 패키지 추가
   - WebMCPProvider에 file-store 서버 등록
   - 통합 테스트 구현

5. **Basic Hooks & Components**
   - use-file-attachment.ts 기본 구조
   - 플레이스홀더 UI 컴포넌트들 (FileUpload, FileList, SearchResults)

## 🚧 진행해야 할 작업

### Phase 1: UI/UX 개선 및 통합 (우선순위: 높음)

#### 1.1 Chat.tsx 파일 첨부 UI 통합
**현재 상태**: FileAttachment 컴포넌트가 있지만 실제 file-store MCP와 연동되지 않음
**필요 작업**:
```typescript
// src/features/chat/Chat.tsx 개선 필요
function ChatInput() {
  const { executeCall } = useWebMCPTools();
  const [attachments, setAttachments] = useState<AttachmentReference[]>([]);
  
  const handleFileAttachment = async (files: File[]) => {
    // 1. createStore (세션별로 한 번만)
    // 2. addContent로 파일들 업로드
    // 3. AttachmentReference[] 형태로 상태 관리
    // 4. 메시지 전송 시 attachments 필드에 포함
  };
}
```

#### 1.2 AttachmentBubble.tsx 개선
**현재 상태**: 기본적인 파일 표시만 가능
**필요 작업**:
- 파일 미리보기 (preview 필드 활용)
- 파일 검색 결과 하이라이팅
- 파일 내용 읽기 (readContent MCP 도구 활용)
- 삭제/관리 기능

#### 1.3 FileAttachment.tsx 개선
**현재 상태**: 로컬 파일 선택만 가능
**필요 작업**:
- Drag & Drop 지원
- 파일 업로드 진행률 표시
- 파일 크기/타입 제한 처리
- file-store MCP와 실제 연동

### Phase 2: Built-in MCP 통합 (우선순위: 높음)

#### 2.1 Built-in MCP Context Provider 추가
**목적**: Web Worker MCP (file-store 등)를 기존 MCP 시스템과 통합
**필요 파일**: `src/context/BuiltinMCPContext.tsx`
```typescript
interface BuiltinMCPContextValue {
  fileStore: {
    createStore: (metadata?: any) => Promise<string>;
    uploadFile: (file: File, storeId: string) => Promise<AttachmentReference>;
    searchFiles: (storeId: string, query: string) => Promise<SearchResult[]>;
    // ... 기타 file-store 도구들
  };
  isReady: boolean;
  error: Error | null;
}
```

#### 2.2 use-ai-service.ts 통합
**현재 상태**: unifiedMCPTools, localTools만 사용
**필요 작업**:
```typescript
export const useAIService = () => {
  const { availableTools: unifiedMCPTools } = useUnifiedMCP();
  const { getAvailableTools: getAvailableLocalTools } = useLocalTools();
  const { fileStore } = useBuiltinMCP(); // 🆕 추가 필요
  
  const submit = async (messages: Message[]) => {
    // 파일 첨부가 있는 메시지 처리
    const filesContext = await processAttachments(messages);
    
    const availableTools = [
      ...unifiedMCPTools,
      ...getAvailableLocalTools(),
      ...getBuiltinMCPTools(), // 🆕 file-store 도구들 포함
    ];
  };
};
```

### Phase 3: 파일 첨부 워크플로우 구현 (우선순위: 중간)

#### 3.1 파일 첨부 → 메시지 전송 플로우
```typescript
// 사용자 워크플로우:
// 1. 파일 선택/드래그 → FileAttachment 컴포넌트
// 2. 파일 업로드 → file-store MCP (createStore + addContent)
// 3. AttachmentReference 생성 → Chat 상태 관리
// 4. 메시지 작성 및 전송 → Message.attachments 필드 포함
// 5. AI 응답 시 파일 내용 자동 참조 → similaritySearch 활용
```

#### 3.2 파일 검색 기능
**목적**: 채팅 중 과거 첨부 파일들을 검색할 수 있는 기능
**필요 컴포넌트**: `src/features/file-attachment/components/FileSearchModal.tsx`

#### 3.3 파일 관리 UI
**목적**: 세션별 첨부 파일 관리
**필요 컴포넌트**: `src/features/file-attachment/components/FileManager.tsx`

### Phase 4: 성능 및 사용성 개선 (우선순위: 낮음)

#### 4.1 파일 미리보기 및 하이라이팅
- PDF, 이미지 등 다양한 파일 형식 지원
- 검색 결과에서 관련 부분 하이라이팅
- 파일 내용 스니펫 표시

#### 4.2 Progressive Enhancement
- 임베딩 검색 엔진 추가 (transformers.js)
- 하이브리드 검색 (BM25 + 임베딩)
- 검색 성능 모니터링

## 🎯 즉시 시작할 작업 우선순위

### 1순위: Built-in MCP Context 구현
**파일**: `src/context/BuiltinMCPContext.tsx`
**이유**: 다른 모든 작업의 기반이 됨

### 2순위: Chat.tsx 파일 첨부 연동
**파일**: `src/features/chat/Chat.tsx`
**이유**: 사용자가 직접 체감할 수 있는 핵심 기능

### 3순위: use-ai-service.ts 통합
**파일**: `src/hooks/use-ai-service.ts`
**이유**: AI가 파일 내용을 실제로 활용할 수 있게 함

### 4순위: UI 컴포넌트 개선
**파일들**: `FileAttachment.tsx`, `AttachmentBubble.tsx`
**이유**: 사용자 경험 향상

## 🔧 기술적 고려사항

### 세션 관리
- 세션별 파일 스토어 관리 (sessionId 기반)
- 세션 종료 시 파일 정리 정책

### 성능 최적화
- 파일 업로드 시 청킹 및 인덱싱 백그라운드 처리
- 검색 결과 캐싱
- 메모리 사용량 모니터링

### 에러 처리
- 파일 업로드 실패 시 재시도 로직
- 네트워크 오류 시 로컬 저장
- 사용자 친화적인 에러 메시지

## 📝 다음 작업 체크리스트

- [ ] BuiltinMCPContext 구현
- [ ] Chat.tsx 파일 첨부 UI 연동
- [ ] use-ai-service.ts Built-in MCP 통합
- [ ] FileAttachment 컴포넌트 개선 (Drag & Drop)
- [ ] AttachmentBubble 파일 미리보기 기능
- [ ] 파일 검색 모달 구현
- [ ] 세션별 파일 관리 시스템
- [ ] 에러 처리 및 사용자 피드백
- [ ] 성능 모니터링 및 최적화
- [ ] E2E 테스트 작성
