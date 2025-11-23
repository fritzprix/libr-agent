# Refactoring Plan 3: Consistent Service Layer Usage Across Codebase

**Date**: 2025-11-23 18:40  
**Author**: AI Assistant  
**Status**: Proposal  
**Dependencies**:

- Plan 1 (RPC Event Notification) - Optional but recommended
- Plan 2 (Service Revalidation) - Optional but recommended

---

## 작업의 목적

코드베이스 전체에서 `dbService` 또는 Dexie를 직접 접근하는 부분을 찾아내고, 일관되게 **Service Layer**를 사용하도록 수정합니다.

이를 통해:

1. **아키텍처 일관성**: 모든 데이터 접근이 서비스 레이어를 경유
2. **유지보수성**: 비즈니스 로직이 서비스에 집중, DB 변경 시 영향 최소화
3. **확장성**: Remote Sync, Caching 등 추가 기능을 서비스에만 추가

---

## 현재의 상태 / 문제점

### Direct DB Access 패턴

```typescript
// ❌ Pattern 1: Context에서 직접 DB 접근
await dbService.mcpServers.upsert(server);
await dbService.assistants.delete(userId, assistantId);

// ❌ Pattern 2: Component에서 직접 DB 접근
const servers = await dbService.mcpServers.getPage(userId, 0, 20);

// ❌ Pattern 3: SWR fetcher에서 직접 DB 접근
useSWR('mcp-servers', () => dbService.mcpServers.getAll(userId));
```

### 발견된 직접 접근 위치 (초기 분석)

1. **`src/context/MCPServerRegistryContext.tsx`**
   - `saveServer()`: `dbService.mcpServers.upsert()`
   - `deleteServer()`: `dbService.mcpServers.delete()`
   - `loadServers()`: `dbService.mcpServers.getAll()`

2. **`src/features/settings/MCPServerManagement.tsx`**
   - SWR fetcher: `dbService.mcpServers.getPage()`

3. **기타 잠재적 위치** (추가 검색 필요)
   - Chat feature 내 direct DB access 가능성
   - Assistant management UI

---

## 관련 코드의 구조 및 동작 방식 Summary

### Bird's Eye View (Before)

```
┌─────────────────────────────────────────────────────────────┐
│                   React Components / Contexts                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ ❌ dbService.mcpServers.upsert()                         │
│  │ ❌ dbService.assistants.getAll()                         │
│  │ ❌ dbService.chats.save()                                │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   DBService (Direct Access)                  │
│  ┌──────────────────────────────────────────────────────────┤
│  │ IndexedDB (Dexie)                                        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

### Bird's Eye View (After)

```
┌─────────────────────────────────────────────────────────────┐
│                   React Components / Contexts                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ ✅ mcpServerService.save()                               │
│  │ ✅ assistantService.getAll()                             │
│  │ ✅ chatService.save()                                    │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   Service Layer                              │
│  ┌──────────────────────────────────────────────────────────┤
│  │ LocalMcpServerService                                    │
│  │ AssistantService                                         │
│  │ ChatService                                              │
│  │ (Business Logic + Event Emission + Remote Sync)         │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   DBService (Infrastructure)                 │
│  ┌──────────────────────────────────────────────────────────┤
│  │ IndexedDB (Dexie)                                        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

---

## 변경 이후의 상태 / 해결 판정 기준

### 해결 판정 기준

1. **Direct DB Access 제거**
   - [ ] `dbService` import가 컴포넌트/컨텍스트에서 제거됨
   - [ ] 모든 데이터 접근이 서비스 레이어를 경유

2. **서비스 인스턴스 주입**
   - [ ] React Context에 서비스 인스턴스 생성 (`useMemo`)
   - [ ] 필요 시 Provider를 통한 서비스 공유

3. **테스트 커버리지**
   - [ ] 기존 기능 동작 확인 (Regression Test)
   - [ ] 서비스 레이어 단위 테스트

4. **코드 검색 결과**
   - [ ] `grep -r "dbService\." src/` → 서비스 레이어 외 결과 없음
   - [ ] `grep -r "from '@/lib/db/db-service'"` → 서비스 파일만 사용

---

## 수정이 필요한 코드 및 수정부분의 코드 스니펫

### Step 1: 코드베이스 전체 검색

**검색 명령어**:

```bash
# Find direct dbService usage
grep -r "dbService\." src/ \
  --include="*.ts" \
  --include="*.tsx" \
  --exclude-dir="lib/services" \
  --exclude-dir="lib/db"

# Find direct Dexie table access
grep -r "\.mcpServers\." src/ --include="*.ts" --include="*.tsx"
grep -r "\.assistants\." src/ --include="*.ts" --include="*.tsx"
grep -r "\.chats\." src/ --include="*.ts" --include="*.tsx"
```

### Step 2: MCPServerRegistryContext 수정 (이미 Plan 2에서 다룸)

**파일**: `src/context/MCPServerRegistryContext.tsx`

**수정 전**:

```typescript
import { dbService } from '@/lib/db/db-service';

const saveServer = async (server: MCPServer) => {
  await dbService.mcpServers.upsert({
    ...server,
    userId: user?.id || DEFAULT_USER_ID,
  });
  // ...
};
```

**수정 후**:

```typescript
import { LocalMcpServerService } from '@/lib/services/mcp-server-service';

const mcpServerService = useMemo(() => new LocalMcpServerService(), []);

const saveServer = async (server: MCPServer) => {
  const userId = user?.id || DEFAULT_USER_ID;
  await mcpServerService.save(userId, server);
  // Event-driven refresh (if Plan 2 implemented)
};
```

### Step 3: MCPServerManagement UI 수정

**파일**: `src/features/settings/MCPServerManagement.tsx`

**수정 전**:

```typescript
import { dbService } from '@/lib/db/db-service';

const {
  data: servers,
  error,
  isLoading,
} = useSWR(['mcp-servers', currentPage], () =>
  dbService.mcpServers.getPage(userId, currentPage, PAGE_SIZE),
);
```

**수정 후**:

```typescript
import { LocalMcpServerService } from '@/lib/services/mcp-server-service';

const mcpServerService = useMemo(() => new LocalMcpServerService(), []);

const {
  data: servers,
  error,
  isLoading,
} = useSWR(['mcp-servers', currentPage], () =>
  mcpServerService.getPage(userId, currentPage, PAGE_SIZE),
);
```

**필요 시 서비스에 `getPage()` 메서드 추가**:

```typescript
// src/lib/services/mcp-server-service.ts

export class LocalMcpServerService implements IMcpServerService {
  // ... existing methods ...

  async getPage(
    userId: string,
    page: number,
    pageSize: number,
  ): Promise<MCPServer[]> {
    return dbService.mcpServers.getPage(userId, page, pageSize);
  }
}
```

### Step 4: AssistantContext 검증 (이미 서비스 사용 중)

**파일**: `src/context/AssistantContext.tsx`

**현재 상태** (✅ 이미 올바름):

```typescript
import { AssistantService } from '@/lib/services/assistant-service';

const assistantService = useMemo(() => new AssistantService(), []);

const saveAssistant = async (assistant: Assistant) => {
  await assistantService.save(userId, assistant);
  // ...
};
```

**추가 검증 사항**:

- [ ] `loadAssistants()` 메서드도 서비스 사용 확인
- [ ] Direct DB access가 없는지 재확인

### Step 5: Chat Feature 검증

**예상 파일**:

- `src/features/chat/ChatContext.tsx`
- `src/features/chat/components/ChatHistory.tsx`

**검증 사항**:

```typescript
// ❌ If found
await dbService.chats.save(chat);

// ✅ Should be
await chatService.save(userId, chat);
```

**필요 시 ChatService 생성**:

```typescript
// src/lib/services/chat-service.ts

export interface IChatService {
  getAll(userId: string): Promise<Chat[]>;
  save(userId: string, chat: Chat): Promise<void>;
  delete(userId: string, chatId: string): Promise<void>;
}

export class LocalChatService implements IChatService {
  async getAll(userId: string): Promise<Chat[]> {
    return dbService.chats.getAll(userId);
  }

  async save(userId: string, chat: Chat): Promise<void> {
    await dbService.chats.save({ ...chat, userId });
    // Emit revalidation event if Plan 2 implemented
  }

  async delete(userId: string, chatId: string): Promise<void> {
    await dbService.chats.delete(userId, chatId);
    // Emit revalidation event
  }
}
```

### Step 6: 기타 발견된 Direct Access 수정

**검색 후 추가 수정**:
각 발견된 위치마다 동일 패턴 적용:

1. 서비스 import
2. `useMemo`로 서비스 인스턴스 생성
3. `dbService` 호출을 서비스 호출로 변경

---

## 재사용 가능한 연관 코드

### 패턴 1: Context에서 서비스 사용

```typescript
import { useMemo } from 'react';
import { SomeService } from '@/lib/services/some-service';

function SomeProvider({ children }) {
  const service = useMemo(() => new SomeService(), []);

  const saveItem = async (item: Item) => {
    await service.save(userId, item);
  };

  const deleteItem = async (itemId: string) => {
    await service.delete(userId, itemId);
  };

  // ...
}
```

### 패턴 2: SWR with Service

```typescript
import useSWR from 'swr';
import { SomeService } from '@/lib/services/some-service';

function SomeComponent() {
  const service = useMemo(() => new SomeService(), []);

  const { data, error, isLoading } = useSWR(['some-key', userId], () =>
    service.getAll(userId),
  );

  // ...
}
```

### 패턴 3: useEffect에서 서비스 호출

```typescript
useEffect(() => {
  const loadData = async () => {
    const items = await service.getAll(userId);
    setItems(items);
  };

  loadData();
}, [service, userId]);
```

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드

### 1. Regression Test: 기존 기능 동작 확인

**파일**: `src/__tests__/regression/db-access.test.tsx`

```typescript
import { describe, it, expect } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';

describe('DB Access Regression Tests', () => {
  it('should save and load MCP servers via service layer', async () => {
    const { result } = renderHook(() => useMCPServerRegistry());

    const newServer = {
      id: 'test-server',
      name: 'Test Server',
      command: 'node',
      args: ['server.js'],
      category: 'development',
    };

    await result.current.saveServer(newServer);

    await waitFor(() => {
      expect(result.current.servers.some((s) => s.id === 'test-server')).toBe(
        true,
      );
    });
  });

  it('should delete MCP servers via service layer', async () => {
    const { result } = renderHook(() => useMCPServerRegistry());

    await result.current.deleteServer('test-server');

    await waitFor(() => {
      expect(result.current.servers.some((s) => s.id === 'test-server')).toBe(
        false,
      );
    });
  });
});
```

### 2. Code Coverage: Direct Access 검출

**파일**: `scripts/check-direct-db-access.sh` (신규)

```bash
#!/bin/bash

# Check for direct dbService usage outside of service layer
RESULT=$(grep -r "dbService\." src/ \
  --include="*.ts" \
  --include="*.tsx" \
  --exclude-dir="lib/services" \
  --exclude-dir="lib/db" \
  --exclude-dir="__tests__" \
  -n)

if [ -n "$RESULT" ]; then
  echo "❌ Direct dbService access found outside service layer:"
  echo "$RESULT"
  exit 1
else
  echo "✅ No direct dbService access found"
  exit 0
fi
```

**CI 통합**:

```yaml
# .github/workflows/ci.yml
- name: Check Service Layer Consistency
  run: bash scripts/check-direct-db-access.sh
```

### 3. Service Layer Unit Tests

**파일**: `src/lib/services/__tests__/chat-service.test.ts` (예시)

```typescript
import { describe, it, expect, vi } from 'vitest';
import { LocalChatService } from '../chat-service';
import { dbService } from '@/lib/db/db-service';

vi.mock('@/lib/db/db-service');

describe('LocalChatService', () => {
  it('should save chat to DB', async () => {
    const service = new LocalChatService();
    const chat = { id: 'chat-1', title: 'Test Chat' };

    await service.save('user-1', chat);

    expect(dbService.chats.save).toHaveBeenCalledWith({
      ...chat,
      userId: 'user-1',
    });
  });

  it('should delete chat from DB', async () => {
    const service = new LocalChatService();

    await service.delete('user-1', 'chat-1');

    expect(dbService.chats.delete).toHaveBeenCalledWith('user-1', 'chat-1');
  });
});
```

---

## 작업 진행 순서 (권장)

1. **Phase 1: 코드베이스 전체 검색**
   - `grep` 명령으로 모든 direct DB access 찾기
   - 발견된 위치 문서화

2. **Phase 2: 우선순위 파일 수정**
   - MCPServerRegistryContext (이미 Plan 2에서 다룸)
   - MCPServerManagement UI
   - AssistantContext 검증

3. **Phase 3: Chat Feature 수정**
   - ChatService 생성 (필요 시)
   - Chat 관련 컴포넌트/컨텍스트 수정

4. **Phase 4: 기타 발견된 위치 수정**
   - 검색 결과에서 남은 파일 처리
   - 패턴별로 그룹화하여 일괄 수정

5. **Phase 5: 검증 및 테스트**
   - Regression test 실행
   - CI 스크립트 추가
   - 전체 기능 동작 확인

---

## 추가 분석 과제

### 1. 서비스 인터페이스 표준화

**현재 상태**:

- `IMcpServerService`: ✅ 정의됨
- `IAssistantService`: ? (확인 필요)
- `IChatService`: ❌ 없음

**작업**:

- 모든 서비스에 대한 인터페이스 정의
- CRUD 메서드 시그니처 표준화

### 2. 서비스 싱글톤 vs 인스턴스

**현재 패턴**:

```typescript
const service = useMemo(() => new SomeService(), []);
```

**대안**:

```typescript
// Singleton pattern
export const mcpServerService = new LocalMcpServerService();

// Usage
import { mcpServerService } from '@/lib/services/mcp-server-service';
```

**장단점**:

- Singleton: 간단, 전역 상태 공유
- Instance: 테스트 격리, DI 가능

**권장**: 현재 패턴 유지 (React Context별 인스턴스)

### 3. Service Provider Pattern

**옵션**: 모든 서비스를 하나의 Provider로 제공

```typescript
// src/context/ServiceProvider.tsx
export function ServiceProvider({ children }) {
  const services = useMemo(() => ({
    mcpServer: new LocalMcpServerService(),
    assistant: new AssistantService(),
    chat: new LocalChatService(),
  }), []);

  return (
    <ServiceContext.Provider value={services}>
      {children}
    </ServiceContext.Provider>
  );
}

// Usage
const { mcpServer, assistant } = useServices();
```

**권장**: 필요 시 Phase 2로 도입 (현재는 개별 인스턴스 유지)

---

## Clarification Q-List

### Q1: 검색 범위

**상황**: `dbService` 사용이 허용되는 범위

**옵션**:

- A) 서비스 레이어 (`src/lib/services/`) 내부만 허용
- B) 유틸리티 함수에서도 허용
- C) 테스트 코드에서도 허용

**권장**: A (서비스 레이어만), C (테스트는 예외)

### Q2: 점진적 마이그레이션

**상황**: 모든 파일을 한 번에 수정할지 여부

**옵션**:

- A) 한 번에 모든 파일 수정 (Breaking Change 가능)
- B) Feature별로 순차 마이그레이션
- C) Deprecation Warning 후 점진적 제거

**권장**: B (Feature별 순차, Regression 최소화)

### Q3: 서비스 레이어 확장

**상황**: 아직 서비스가 없는 엔티티 (e.g., Chat, Settings)

**옵션**:

- A) 즉시 서비스 생성
- B) 별도 계획으로 분리
- C) Direct Access 허용 (표시만)

**권장**: A (이번 Plan에서 함께 생성)

### Q4: SWR Key 전략

**상황**: 서비스 레이어 사용 시 SWR 키 관리

**옵션**:

- A) `['entity-name', userId, ...params]` 형식 표준화
- B) 서비스별 키 생성 함수 제공
- C) 현재 방식 유지

**권장**: A (표준화, 일관성)

---

## End of Refactoring Plan 3
