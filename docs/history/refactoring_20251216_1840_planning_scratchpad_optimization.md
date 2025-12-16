# Refactoring Plan: Planning Server Scratchpad Optimization

**Date:** 2025-12-16  
**Author:** AI Assistant  
**Target Module:** `src/lib/web-mcp/modules/planning-server`

---

## 1. 작업의 목적

Planning Server의 Scratchpad 기능을 개선하여 **AI Agent의 컨텍스트 윈도우 소진 문제를 해결**하고, 태그 기반 검색 및 요약 표시를 통해 더 효율적인 메모 관리를 제공한다.

---

## 2. 현재의 상태 / 문제점

### 2.1 문제점

- **컨텍스트 윈도우 소진**: 현재 `getServiceContext`에서 Scratchpad의 전체 내용이 포함되어 AI Agent의 컨텍스트를 빠르게 소진시킴
- **검색 기능 부재**: Scratchpad 아이템이 많아질 경우 원하는 정보를 찾기 어려움
- **구조화 부족**: 제목, 태그 등 메타데이터가 없어 아이템 구분이 어려움

### 2.2 현재 구조

**데이터 모델 (기존):**

```typescript
export interface ScratchpadItem {
  id: number;
  content: string;
  source?: string;
}
```

**DB 스키마 (기존):**

```typescript
scratchpad: '++id, [sessionId+threadId]';
```

**Context 전달 방식 (기존):**

```typescript
// getServiceContext에서 전체 내용 표시
if (scratchpad.length > 0) {
  contextParts.push(`Scratchpad (${scratchpad.length}):`);
  scratchpad.slice(0, 3).forEach((m) => {
    contextParts.push(`- ${m.content}`); // 전체 내용 노출
  });
}
```

---

## 3. 관련 코드의 구조 및 동작 방식 Summary

### 3.1 아키텍처 개요 (Bird's Eye View)

```
ChatContext (사용자 요청)
    ↓
PlanningServer.callTool() (server.ts)
    ↓
SessionStateManager (state.ts)
    ↓
ScratchpadManager (managers/scratchpad-manager.ts)
    ↓
PlanningDatabase (db.ts - Dexie IndexedDB)
```

### 3.2 주요 컴포넌트

1. **`tools.ts`**: 도구 스키마 정의 (addScratchpad, clearScratchpad 등)
2. **`types.ts`**: TypeScript 인터페이스 (ScratchpadItem)
3. **`db.ts`**: Dexie 스키마 및 마이그레이션
4. **`managers/scratchpad-manager.ts`**: 비즈니스 로직 (CRUD)
5. **`state.ts`**: SessionStateManager 래퍼
6. **`server.ts`**: MCP 도구 핸들러 및 getServiceContext

### 3.3 데이터 흐름

```
[사용자] addScratchpad 호출
    → ScratchpadManager.addScratchpad()
    → db.scratchpad.add({ sessionId, threadId, content, source })
    → IndexedDB 저장
    → 용량 체크 (MAX_NOTES=20)
    → 가장 오래된 항목 삭제 (필요시)

[AI Agent] getServiceContext 호출
    → stateManager.getStateForSession()
    → scratchpad 전체 로드
    → contextPrompt에 내용 포함
    → AI에게 전달 (컨텍스트 소진)
```

---

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1 목표 상태

- **메타데이터 추가**: 제목(title), 태그(tags) 필드로 구조화
- **요약 표시**: `getServiceContext`에서 ID, 제목, 태그만 표시하고 내용은 숨김
- **검색 기능**: `readScratchpad` 도구로 ID 또는 태그 기반 검색 지원
- **컨텍스트 절약**: 전체 내용 대신 요약 정보만 전달하여 토큰 사용량 감소

### 4.2 해결 판정 기준

- [ ] `ScratchpadItem`에 `title`, `tags` 필드 추가 완료
- [ ] DB 스키마 v3 마이그레이션 적용 (`*tags` 인덱스)
- [ ] `addScratchpad` 도구가 `title`, `tags` 파라미터 지원
- [ ] `readScratchpad` 도구가 `ids` 및 `tags` 필터링 지원
- [ ] `getServiceContext`에서 Scratchpad 요약 표시 (제목, 태그만)
- [ ] `getCurrentState`에서도 요약 표시 적용
- [ ] 기존 데이터 호환성 유지 (title, tags 없는 아이템도 정상 작동)

---

## 5. 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 5.1 `db.ts` - 스키마 및 마이그레이션

**변경 전:**

```typescript
export interface PlanningScratchpadItem {
  id?: number;
  sessionId: string;
  threadId: string;
  content: string;
  source?: string;
  createdAt: number;
}

// Version 2까지만 존재
this.version(2).stores({
  scratchpad: '++id, [sessionId+threadId]',
});
```

**변경 후:**

```typescript
export interface PlanningScratchpadItem {
  id?: number;
  sessionId: string;
  threadId: string;
  title?: string; // 추가
  content: string;
  tags?: string[]; // 추가
  source?: string;
  createdAt: number;
}

// Version 3 추가
this.version(3).stores({
  goals: '++id, [sessionId+threadId], isActive',
  todos: '++id, [sessionId+threadId], checked',
  scratchpad: '++id, [sessionId+threadId], *tags', // tags 인덱스 추가
});
```

### 5.2 `types.ts` - 인터페이스 업데이트

**변경 전:**

```typescript
export interface ScratchpadItem {
  id: number;
  content: string;
  source?: string;
}
```

**변경 후:**

```typescript
export interface ScratchpadItem {
  id: number;
  title?: string; // 추가
  content: string;
  tags?: string[]; // 추가
  source?: string;
}
```

### 5.3 `tools.ts` - 도구 스키마 확장

**변경 내용:**

1. **`addScratchpad` 도구 업데이트**:

```typescript
{
  name: 'addScratchpad',
  inputSchema: {
    type: 'object',
    properties: {
      title: {
        type: 'string',
        description: 'Optional title for the note.'
      },
      note: { type: 'string', description: '...' },
      tags: {
        type: 'array',
        items: { type: 'string' },
        description: 'Optional tags for categorization.'
      },
      source: { type: 'string', description: '...' }
    },
    required: ['note']
  }
}
```

2. **`readScratchpad` 도구 추가** (신규):

```typescript
{
  name: 'readScratchpad',
  description: 'Read specific scratchpad items by their IDs or filter by tags.',
  inputSchema: {
    type: 'object',
    properties: {
      ids: {
        type: 'array',
        items: { type: 'number', minimum: 0 },
        description: 'List of scratchpad IDs to read.'
      },
      tags: {
        type: 'array',
        items: { type: 'string' },
        description: 'List of tags to filter by.'
      }
    }
  }
}
```

### 5.4 `managers/scratchpad-manager.ts` - 로직 구현

**변경 내용:**

1. **`getScratchpadList()` 메서드 업데이트**:

```typescript
async getScratchpadList(): Promise<ScratchpadItem[]> {
  const items = await db.scratchpad
    .where({ sessionId: this.sessionId, threadId: this.threadId })
    .sortBy('id');

  return items.map((m) => ({
    id: m.id!,
    title: m.title,      // 추가
    content: m.content,
    tags: m.tags,        // 추가
    source: m.source,
  }));
}
```

2. **`addScratchpad()` 시그니처 변경**:

```typescript
async addScratchpad(
  note: string,
  source?: string,
  title?: string,       // 추가
  tags?: string[],      // 추가
): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
  await db.scratchpad.add({
    sessionId: this.sessionId,
    threadId: this.threadId,
    content: note,
    source,
    title,     // 추가
    tags,      // 추가
    createdAt: Date.now(),
  });
  // ...existing capacity check logic
}
```

3. **`readScratchpad()` 메서드 추가** (신규):

```typescript
async readScratchpad(
  ids?: number[],
  tags?: string[],
): Promise<MCPResult<{ scratchpad: ScratchpadItem[] }>> {
  let items: ScratchpadItem[] = [];
  const allItems = await this.getScratchpadList();

  if (ids && ids.length > 0) {
    // ID 기반 검색
    items = allItems.filter((item) => ids.includes(item.id));
  } else if (tags && tags.length > 0) {
    // 태그 기반 검색
    items = allItems.filter(
      (item) => item.tags && item.tags.some((tag) => tags.includes(tag))
    );
  } else {
    // 전체 반환
    items = allItems;
  }

  return createMCPStructuredToolResult(`Read ${items.length} items`, {
    success: true,
    scratchpad: items,
  });
}
```

### 5.5 `state.ts` - 래퍼 메서드 업데이트

**변경 내용:**

```typescript
async addScratchpad(
  note: string,
  source?: string,
  title?: string,       // 추가
  tags?: string[],      // 추가
): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
  return this.scratchpadManager.addScratchpad(note, source, title, tags);
}

async readScratchpad(
  ids?: number[],
  tags?: string[],
): Promise<MCPResult<{ scratchpad: ScratchpadItem[] }>> {
  return this.scratchpadManager.readScratchpad(ids, tags);
}
```

### 5.6 `server.ts` - 도구 핸들러 및 컨텍스트 최적화

**변경 내용:**

1. **`callTool()` 핸들러 추가/수정**:

```typescript
case 'addScratchpad': {
  return await stateManager.addScratchpad(
    typedArgs.note as string,
    typedArgs.source as string | undefined,
    typedArgs.title as string | undefined,    // 추가
    typedArgs.tags as string[] | undefined,   // 추가
  );
}

case 'readScratchpad': {  // 신규 핸들러
  return await stateManager.readScratchpad(
    typedArgs.ids as number[] | undefined,
    typedArgs.tags as string[] | undefined,
  );
}
```

2. **`getServiceContext()` 요약 표시**:

```typescript
if (scratchpad.length > 0) {
  contextParts.push(`Scratchpad (${scratchpad.length}):`);
  scratchpad.slice(0, 5).forEach((m) => {
    const titlePart = m.title ? `[${m.title}] ` : '';
    const tagsPart =
      m.tags && m.tags.length > 0 ? ` (tags: ${m.tags.join(', ')})` : '';
    const contentPreview = m.title ? '' : `${m.content.slice(0, 30)}...`;
    contextParts.push(`- ID:${m.id} ${titlePart}${contentPreview}${tagsPart}`);
  });
  if (scratchpad.length > 5) {
    contextParts.push(
      `...and ${scratchpad.length - 5} more. Use readScratchpad to view details.`,
    );
  }
}
```

3. **`getCurrentState()` 요약 표시**:

```typescript
const scratchpadText =
  includeScratchpad && scratchpad.length > 0
    ? scratchpad
        .map((m) => {
          const titlePart = m.title ? ` [${m.title}]` : '';
          const tagsPart =
            m.tags && m.tags.length > 0 ? ` (tags: ${m.tags.join(', ')})` : '';
          const contentPreview = m.title
            ? ''
            : ` ${m.content.slice(0, 50)}${m.content.length > 50 ? '...' : ''}`;
          return `- [ID: ${m.id}]${titlePart}${contentPreview}${tagsPart}`;
        })
        .join('\n')
    : '(none)';
```

4. **`PlanningServerProxy` 인터페이스 업데이트**:

```typescript
export interface PlanningServerProxy extends WebMCPServerProxy {
  // ...existing methods
  add_scratchpad(args: {
    note: string;
    title?: string; // 추가
    tags?: string[]; // 추가
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;

  read_scratchpad(args: {
    // 신규 메서드
    ids?: number[];
    tags?: string[];
  }): Promise<MCPResult<{ scratchpad: ScratchpadItem[] }>>;

  clear_scratchpad(args: {
    id: number;
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;
}
```

---

## 6. 재사용 가능한 연관 코드

### 6.1 유사 패턴 참고

**Knowledge Server의 태그 검색 패턴** (`src/lib/web-mcp/modules/knowledge-server/`):

```typescript
// db.ts
knowledge: '++id, assistantId, *tags, createdAt, updatedAt';

// search logic
const items = await db.knowledge
  .where('assistantId')
  .equals(assistantId)
  .filter((item) => tags.some((tag) => item.tags.includes(tag)))
  .toArray();
```

### 6.2 IndexedDB 마이그레이션 패턴

**기존 마이그레이션 참고** (v2 마이그레이션):

```typescript
this.version(2)
  .stores({
    /* new schema */
  })
  .upgrade(async (tx) => {
    // 기존 데이터 변환 로직
    const todos = await tx.table('todos').toArray();
    for (const todo of todos) {
      await tx.table('todos').update(todo.id!, {
        checked: oldTodo.status === 'completed',
      });
    }
  });
```

---

## 7. Test Code 추가 및 수정 필요 부분

### 7.1 Unit Tests

**파일 경로:** `src/lib/web-mcp/modules/planning-server/__tests__/scratchpad.test.ts` (신규 생성)

**테스트 케이스:**

1. **기본 기능 테스트**:
   - `addScratchpad`로 제목/태그 포함 아이템 추가
   - `readScratchpad`로 ID 기반 조회
   - `readScratchpad`로 태그 기반 조회
   - 제목/태그 없는 아이템도 정상 동작 (하위 호환성)

2. **용량 제한 테스트**:
   - MAX_NOTES(20) 초과 시 가장 오래된 항목 삭제 확인

3. **태그 검색 테스트**:
   - 단일 태그 검색
   - 복수 태그 OR 검색 (태그 중 하나라도 일치)
   - 태그가 없는 아이템은 필터링되는지 확인

4. **컨텍스트 최적화 테스트**:
   - `getServiceContext`가 전체 내용이 아닌 요약만 반환하는지 확인
   - 5개 초과 시 "...and X more" 메시지 확인

**샘플 테스트 코드:**

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import planningServer from '../server';

describe('Scratchpad with Title and Tags', () => {
  beforeEach(async () => {
    await planningServer.switchContext({ sessionId: 'test-session' });
    await planningServer.callTool('clearSession', {});
  });

  it('should add scratchpad with title and tags', async () => {
    const result = await planningServer.callTool('addScratchpad', {
      title: 'Bug Report',
      note: 'User authentication fails on mobile',
      tags: ['bug', 'mobile', 'urgent'],
    });

    expect(result.isError).toBe(false);
    expect(result.content[0].text).toContain('Bug Report');
  });

  it('should read scratchpad by tags', async () => {
    await planningServer.callTool('addScratchpad', {
      title: 'Note 1',
      note: 'Content 1',
      tags: ['bug'],
    });
    await planningServer.callTool('addScratchpad', {
      title: 'Note 2',
      note: 'Content 2',
      tags: ['feature'],
    });

    const result = await planningServer.callTool('readScratchpad', {
      tags: ['bug'],
    });

    expect(result.isError).toBe(false);
    const data = JSON.parse(result.content[0].text);
    expect(data.scratchpad).toHaveLength(1);
    expect(data.scratchpad[0].title).toBe('Note 1');
  });

  it('should display summary in context', async () => {
    await planningServer.callTool('addScratchpad', {
      title: 'Important Finding',
      note: 'Very long content that should not appear in context preview...',
      tags: ['important'],
    });

    const context = await planningServer.getServiceContext({
      sessionId: 'test-session',
    });

    expect(context.contextPrompt).toContain('Important Finding');
    expect(context.contextPrompt).toContain('tags: important');
    expect(context.contextPrompt).not.toContain('Very long content');
  });
});
```

### 7.2 Integration Tests

**파일 경로:** `src/lib/web-mcp/modules/planning-server/__tests__/integration.test.ts` (기존 파일 수정)

**추가 시나리오:**

- 여러 세션 간 Scratchpad 격리 확인
- 태그 기반 검색 후 `readScratchpad`로 전체 내용 조회하는 워크플로우

---

## 8. 추가 분석 과제

### 8.1 ThreadId 의존성 검토

**현재 상태:**

- Planning Server는 `[sessionId+threadId]` 복합 키로 데이터를 격리
- 대부분의 경우 `threadId === sessionId` (Top thread)
- 타 모듈(knowledge-server)은 `assistantId`만 사용

**검토 필요 사항:**

- Scratchpad가 세션 내 모든 스레드에서 공유되어야 하는가?
- 현재 구조가 사용자 경험(UX)에 부정적 영향을 미치는가?
- Session-scoped Planning으로 전환 시 마이그레이션 전략

**권장 사항:**

- 별도 Refactoring Plan으로 분리하여 진행
- 사용자 피드백 및 실제 사용 패턴 분석 후 결정

### 8.2 Scratchpad UI 개선

**현재 상태:**

- Scratchpad는 텍스트 기반 컨텍스트로만 제공
- UI에서 직접 확인/관리하는 기능 없음

**검토 필요 사항:**

- Planning State를 시각화하는 전용 UI 패널 추가 필요성
- 태그 기반 필터링 UI 제공 여부

---

## 9. Clarification Q-list

### Q1. ThreadId 격리 방식 유지 여부

**질문:** 현재 Scratchpad는 `(sessionId, threadId)` 조합으로 격리됩니다. 사용자가 세션 내에서 새로운 스레드를 열면 기존 Scratchpad를 볼 수 없습니다. 이를 Session-scoped로 변경하여 모든 스레드가 공유하도록 할까요?

**선택지:**

1. 현재 구조 유지 (스레드별 격리)
2. Session-scoped로 변경 (세션 내 공유)
3. 사용자 설정으로 선택 가능하게 구현

### Q2. MAX_NOTES 용량 제한

**질문:** 현재 Scratchpad는 20개로 제한되어 있습니다. 태그/제목 기반 구조화로 인해 더 많은 아이템을 관리할 수 있게 되었는데, 용량 제한을 늘릴 필요가 있나요?

**권장:** 30~50개로 증가 고려 (컨텍스트 요약으로 토큰 사용량 감소)

### Q3. 태그 검색 로직 (OR vs AND)

**질문:** 현재 구현은 여러 태그 중 하나라도 일치하면 반환(OR 검색)합니다. 모든 태그가 일치해야 하는 AND 검색도 필요한가요?

**현재 구현:**

```typescript
item.tags.some((tag) => tags.includes(tag)); // OR 검색
```

**대안:**

```typescript
tags.every((tag) => item.tags.includes(tag)); // AND 검색
```

### Q4. 기존 데이터 마이그레이션

**질문:** DB 스키마 v3 업그레이드 시 기존 Scratchpad 아이템(title, tags 없음)에 대한 기본값 설정이 필요한가요?

**제안:**

- `title`: 없으면 `null` 유지 (옵셔널)
- `tags`: 빈 배열 `[]` 또는 `null` (옵셔널)

**현재 구현:** 별도 마이그레이션 로직 없음 (필드 추가만, 기존 데이터는 undefined)

---

## 10. 작업 우선순위

1. **Phase 1 - Core Implementation** (완료됨):
   - [x] DB 스키마 업데이트 (v3)
   - [x] 인터페이스 업데이트 (types.ts)
   - [x] 도구 스키마 확장 (tools.ts)
   - [x] ScratchpadManager 로직 구현
   - [x] Server 핸들러 및 컨텍스트 최적화

2. **Phase 2 - Testing**:
   - [ ] Unit Test 작성
   - [ ] Integration Test 업데이트
   - [ ] 기존 데이터 호환성 테스트

3. **Phase 3 - Documentation**:
   - [ ] API 문서 업데이트
   - [ ] 사용 가이드 추가
   - [ ] CHANGELOG 업데이트

4. **Phase 4 - Future Enhancements** (선택):
   - [ ] ThreadId 의존성 제거 (별도 Refactoring)
   - [ ] Scratchpad UI 패널 추가
   - [ ] 태그 자동 완성 기능

---

## 11. 참고 자료

- **현재 구현 파일:**
  - `src/lib/web-mcp/modules/planning-server/db.ts`
  - `src/lib/web-mcp/modules/planning-server/types.ts`
  - `src/lib/web-mcp/modules/planning-server/tools.ts`
  - `src/lib/web-mcp/modules/planning-server/managers/scratchpad-manager.ts`
  - `src/lib/web-mcp/modules/planning-server/state.ts`
  - `src/lib/web-mcp/modules/planning-server/server.ts`

- **관련 문서:**
  - `.github/copilot-instructions.md` - 코딩 스타일 및 아키텍처 가이드
  - `docs/architecture/chat-feature-architecture.md` - Chat Feature 아키텍처

- **유사 구현 참고:**
  - `src/lib/web-mcp/modules/knowledge-server/` - 태그 기반 검색 패턴
