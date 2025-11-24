# Refactoring Plan 2: Service Layer Revalidation Event Propagation

**Date**: 2025-11-23 18:35  
**Author**: AI Assistant  
**Status**: Proposal  
**Dependencies**: Plan 1 (RPC Event Notification) must be completed first

---

## 작업의 목적

Service Layer가 데이터를 변경(save/delete)할 때, 해당 변경 사항을 구독 중인 컴포넌트에 자동으로 알려 UI를 갱신하도록 합니다.

**Plan 1의 RPC Notification 메커니즘**을 활용하여, Worker Context에서 실행되는 서비스가 Main Thread의 React Context를 갱신할 수 있게 만듭니다.

---

## 현재의 상태 / 문제점

### 현재 구조

```typescript
// Main Thread Context (MCPServerRegistryContext.tsx)
const saveServer = async (server: MCPServer) => {
  await dbService.mcpServers.upsert(server); // ❌ 직접 DB 접근
  setServers((prev) => [...prev, server]); // ✅ 로컬 상태 갱신
};

// Worker Context (mcp-manager/server.ts)
const createServer = async (config: ServerConfig) => {
  const server = { ...config, id: generateId() };
  await mcpService.save(server); // ✅ 서비스 사용
  // ❌ Main Thread Context가 변경을 감지하지 못함
  return server;
};
```

### 문제점

1. **Service Layer Bypass**: Main Thread Context가 `dbService` 직접 사용
2. **Worker 변경 미반영**: Worker에서 데이터 변경 시 UI 갱신 없음
3. **일관성 부재**: AssistantContext는 서비스 사용, MCPServerRegistryContext는 DB 직접 사용

---

## 관련 코드의 구조 및 동작 방식 Summary

### Bird's Eye View

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Thread                                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ MCPServerRegistryContext.tsx                             │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ ❌ dbService.mcpServers.upsert()             │        │
│  │  │ ❌ No subscription to revalidation events    │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────────────────────────────────────────────┤
│  │ LocalMcpServerService.ts (Main Thread)                   │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ save() → dbService.mcpServers.upsert()       │        │
│  │  │ ❌ No event emission                         │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ ❌ No notification
                           │
┌──────────────────────────┴───────────────────────────────────┐
│                   Web Worker                                 │
│  ┌──────────────────────────────────────────────────────────┤
│  │ McpServerService.ts (Worker)                             │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ save() → localService.save() + remoteService │        │
│  │  │ ❌ No notification after save                │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

### 서비스 레이어 계층 구조

```
IMcpServerService (Interface)
    ├── LocalMcpServerService (Direct DB access)
    ├── RemoteMcpServerService (API client)
    └── McpServerService (Unified = Local + Remote)
```

---

## 변경 이후의 상태 / 해결 판정 기준

### 목표 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Thread                                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ MCPServerRegistryContext.tsx                             │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ ✅ mcpServerService.save()                   │        │
│  │  │ ✅ webMCPProxy.onNotify('service-revalidate')│        │
│  │  │    → refreshAll()                            │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────────────────────────────────────────────┤
│  │ LocalMcpServerService.ts (Main Thread)                   │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ save() → dbService.mcpServers.upsert()       │        │
│  │  │ ✅ emit('service-revalidate', { entity, ... })│       │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ ✅ sendNotification('service-revalidate')
                           │
┌──────────────────────────┴───────────────────────────────────┐
│                   Web Worker                                 │
│  ┌──────────────────────────────────────────────────────────┤
│  │ McpServerService.ts (Worker)                             │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ save() → localService.save() + remoteService │        │
│  │  │ ✅ sendNotification('service-revalidate', ...)│       │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

### 해결 판정 기준

1. **서비스 레이어 일관성**
   - [ ] Main Thread Context도 서비스 레이어 사용 (`LocalMcpServerService`)
   - [ ] Worker Context는 `McpServerService` (Unified) 사용 유지

2. **Revalidation 이벤트 발생**
   - [ ] Main Thread `LocalMcpServerService.save()` → 알림 발생 (자기 자신에게 알림)
   - [ ] Worker `McpServerService.save()` → 알림 발생 (Worker → Main Thread)

3. **React Context 구독**
   - [ ] `MCPServerRegistryContext`가 `service-revalidate` 알림 구독
   - [ ] 알림 수신 시 `refreshAll()` 호출하여 DB에서 최신 데이터 로드

4. **테스트 시나리오**
   - [ ] Main Thread에서 save → Context 자동 갱신
   - [ ] Worker에서 save → Context 자동 갱신
   - [ ] 여러 Context가 동시 구독 가능

---

## 수정이 필요한 코드 및 수정부분의 코드 스니펫

### 1. Service Layer에 이벤트 발생 추가

**파일**: `src/lib/services/mcp-server-service.ts`

**IMcpServerService 인터페이스 확장 (옵션)**:

```typescript
export interface IMcpServerService {
  getAll(userId: string, category?: ServerCategory): Promise<MCPServer[]>;
  getByName(userId: string, name: string): Promise<MCPServer | undefined>;
  save(userId: string, server: MCPServer): Promise<void>;
  delete(userId: string, serverId: string): Promise<void>;

  // ✅ NEW: Optional event listener (only for contexts that need notifications)
  onRevalidate?: (callback: (event: RevalidateEvent) => void) => () => void;
}

export interface RevalidateEvent {
  entity: 'mcpServers' | 'assistants';
  action: 'save' | 'delete';
  entityId?: string;
}
```

**LocalMcpServerService 수정** (Main Thread):

```typescript
export class LocalMcpServerService implements IMcpServerService {
  private revalidateCallbacks = new Set<(event: RevalidateEvent) => void>();

  async save(userId: string, server: MCPServer): Promise<void> {
    await dbService.mcpServers.upsert({
      ...server,
      userId,
      updatedAt: new Date(),
    });

    // ✅ Emit revalidation event
    this.emitRevalidate({
      entity: 'mcpServers',
      action: 'save',
      entityId: server.id,
    });
  }

  async delete(userId: string, serverId: string): Promise<void> {
    await dbService.mcpServers.delete(userId, serverId);

    // ✅ Emit revalidation event
    this.emitRevalidate({
      entity: 'mcpServers',
      action: 'delete',
      entityId: serverId,
    });
  }

  // ✅ NEW: Event subscription
  onRevalidate(callback: (event: RevalidateEvent) => void): () => void {
    this.revalidateCallbacks.add(callback);
    return () => this.revalidateCallbacks.delete(callback);
  }

  // ✅ NEW: Event emission
  private emitRevalidate(event: RevalidateEvent): void {
    for (const callback of this.revalidateCallbacks) {
      try {
        callback(event);
      } catch (error) {
        logger.error('Error in revalidate callback', error);
      }
    }
  }
}
```

**McpServerService 수정** (Worker, Unified):

```typescript
export class McpServerService implements IMcpServerService {
  constructor(
    private localService: LocalMcpServerService,
    private remoteService: RemoteMcpServerService,
  ) {}

  async save(userId: string, server: MCPServer): Promise<void> {
    // Save locally first
    await this.localService.save(userId, server);

    // ✅ Notify Main Thread (via Plan 1's sendNotification)
    if (typeof self !== 'undefined' && 'sendNotification' in self) {
      (
        self as DedicatedWorkerGlobalScope & {
          sendNotification: (type: string, data: unknown) => void;
        }
      ).sendNotification('service-revalidate', {
        entity: 'mcpServers',
        action: 'save',
        entityId: server.id,
      });
    }

    // Try remote sync (non-blocking)
    this.remoteService.save(userId, server).catch((error) => {
      logger.warn('Remote sync failed for save', error);
    });
  }

  async delete(userId: string, serverId: string): Promise<void> {
    await this.localService.delete(userId, serverId);

    // ✅ Notify Main Thread
    if (typeof self !== 'undefined' && 'sendNotification' in self) {
      (
        self as DedicatedWorkerGlobalScope & {
          sendNotification: (type: string, data: unknown) => void;
        }
      ).sendNotification('service-revalidate', {
        entity: 'mcpServers',
        action: 'delete',
        entityId: serverId,
      });
    }

    this.remoteService.delete(userId, serverId).catch((error) => {
      logger.warn('Remote sync failed for delete', error);
    });
  }

  // Forward other methods...
}
```

### 2. React Context에서 서비스 레이어 사용 + 이벤트 구독

**파일**: `src/context/MCPServerRegistryContext.tsx`

**수정 전**:

```typescript
export function MCPServerRegistryProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [servers, setServers] = useState<MCPServer[]>([]);
  const { user } = useAuth();

  const saveServer = async (server: MCPServer) => {
    // ❌ Direct DB access
    await dbService.mcpServers.upsert({
      ...server,
      userId: user?.id || DEFAULT_USER_ID,
    });
    // Local state update
    setServers((prev) => {
      const index = prev.findIndex((s) => s.id === server.id);
      if (index >= 0) {
        const updated = [...prev];
        updated[index] = server;
        return updated;
      }
      return [...prev, server];
    });
  };

  // ...
}
```

**수정 후**:

```typescript
import { LocalMcpServerService } from '@/lib/services/mcp-server-service';
import { webMCPProxy } from '@/lib/web-mcp/mcp-proxy';

export function MCPServerRegistryProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [servers, setServers] = useState<MCPServer[]>([]);
  const { user } = useAuth();

  // ✅ Use service layer
  const mcpServerService = useMemo(() => new LocalMcpServerService(), []);

  // ✅ Refresh all servers from DB
  const refreshAll = useCallback(async () => {
    const userId = user?.id || DEFAULT_USER_ID;
    const allServers = await mcpServerService.getAll(userId);
    setServers(allServers);
  }, [mcpServerService, user?.id]);

  // ✅ Subscribe to revalidation events from Worker
  useEffect(() => {
    const unsubscribe = webMCPProxy.onNotify('service-revalidate', (data) => {
      const event = data as RevalidateEvent;
      if (event.entity === 'mcpServers') {
        logger.debug('Received service-revalidate event, refreshing...', event);
        refreshAll();
      }
    });

    return unsubscribe;
  }, [refreshAll]);

  // ✅ Subscribe to local service events (Main Thread changes)
  useEffect(() => {
    if (mcpServerService.onRevalidate) {
      const unsubscribe = mcpServerService.onRevalidate((event) => {
        logger.debug('Local service changed, refreshing...', event);
        refreshAll();
      });
      return unsubscribe;
    }
  }, [mcpServerService, refreshAll]);

  // ✅ Load initial data
  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  const saveServer = async (server: MCPServer) => {
    const userId = user?.id || DEFAULT_USER_ID;

    // ✅ Use service layer (will trigger revalidation event)
    await mcpServerService.save(userId, server);

    // No need to manually update state - event listener will call refreshAll()
  };

  const deleteServer = async (serverId: string) => {
    const userId = user?.id || DEFAULT_USER_ID;

    // ✅ Use service layer
    await mcpServerService.delete(userId, serverId);

    // Event listener will refresh automatically
  };

  // ... rest of context ...
}
```

### 3. AssistantContext도 동일 패턴 적용

**파일**: `src/context/AssistantContext.tsx`

**현재 상태**:

```typescript
// ✅ Already uses AssistantService correctly
const assistantService = useMemo(() => new AssistantService(), []);
```

**추가 필요**:

```typescript
// ✅ Subscribe to Worker revalidation events
useEffect(() => {
  const unsubscribe = webMCPProxy.onNotify('service-revalidate', (data) => {
    const event = data as RevalidateEvent;
    if (event.entity === 'assistants') {
      logger.debug('Received assistant revalidation event', event);
      loadAssistants(); // Refresh from DB
    }
  });

  return unsubscribe;
}, [loadAssistants]);
```

**AssistantService 수정** (Worker에서 사용 시):

```typescript
// src/lib/services/assistant-service.ts (Worker Context)

async save(userId: string, assistant: Assistant): Promise<void> {
  await this.localService.save(userId, assistant);

  // ✅ Notify Main Thread
  if (typeof self !== 'undefined' && 'sendNotification' in self) {
    (self as DedicatedWorkerGlobalScope & { sendNotification: (type: string, data: unknown) => void })
      .sendNotification('service-revalidate', {
        entity: 'assistants',
        action: 'save',
        entityId: assistant.id,
      });
  }

  // Remote sync...
}
```

---

## 재사용 가능한 연관 코드

### 참고: AssistantContext의 서비스 사용 패턴

**파일**: `src/context/AssistantContext.tsx`

```typescript
// ✅ Correct pattern to follow
const assistantService = useMemo(() => new AssistantService(), []);

const saveAssistant = async (assistant: Assistant) => {
  const userId = user?.id || DEFAULT_USER_ID;
  await assistantService.save(userId, assistant);
  await loadAssistants(); // Refresh
};
```

MCPServerRegistryContext가 이 패턴을 따라야 함.

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드

### 1. Service Layer 이벤트 발생 테스트

**파일**: `src/lib/services/__tests__/mcp-server-service.test.ts`

```typescript
import { describe, it, expect, vi } from 'vitest';
import { LocalMcpServerService } from '../mcp-server-service';

describe('LocalMcpServerService Revalidation Events', () => {
  it('should emit revalidate event on save', async () => {
    const service = new LocalMcpServerService();
    const callback = vi.fn();

    service.onRevalidate!(callback);

    await service.save('user-1', {
      id: 'server-1',
      name: 'Test Server',
      // ... other fields
    });

    expect(callback).toHaveBeenCalledWith({
      entity: 'mcpServers',
      action: 'save',
      entityId: 'server-1',
    });
  });

  it('should emit revalidate event on delete', async () => {
    const service = new LocalMcpServerService();
    const callback = vi.fn();

    service.onRevalidate!(callback);

    await service.delete('user-1', 'server-1');

    expect(callback).toHaveBeenCalledWith({
      entity: 'mcpServers',
      action: 'delete',
      entityId: 'server-1',
    });
  });

  it('should support multiple callbacks', async () => {
    const service = new LocalMcpServerService();
    const callback1 = vi.fn();
    const callback2 = vi.fn();

    service.onRevalidate!(callback1);
    service.onRevalidate!(callback2);

    await service.save('user-1', { id: 'server-1', name: 'Test' });

    expect(callback1).toHaveBeenCalled();
    expect(callback2).toHaveBeenCalled();
  });

  it('should unsubscribe callback', async () => {
    const service = new LocalMcpServerService();
    const callback = vi.fn();

    const unsubscribe = service.onRevalidate!(callback);
    unsubscribe();

    await service.save('user-1', { id: 'server-1', name: 'Test' });

    expect(callback).not.toHaveBeenCalled();
  });
});
```

### 2. React Context 이벤트 구독 테스트

**파일**: `src/context/__tests__/MCPServerRegistryContext.test.tsx`

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import {
  MCPServerRegistryProvider,
  useMCPServerRegistry,
} from '../MCPServerRegistryContext';
import { webMCPProxy } from '@/lib/web-mcp/mcp-proxy';

describe('MCPServerRegistryContext Revalidation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should refresh on Worker revalidation event', async () => {
    const { result } = renderHook(() => useMCPServerRegistry(), {
      wrapper: MCPServerRegistryProvider,
    });

    // Initial load
    await waitFor(() => expect(result.current.servers).toBeDefined());
    const initialCount = result.current.servers.length;

    // Simulate Worker notification
    const handlers = new Map();
    vi.spyOn(webMCPProxy, 'onNotify').mockImplementation((type, handler) => {
      handlers.set(type, handler);
      return () => handlers.delete(type);
    });

    // Trigger notification
    const handler = handlers.get('service-revalidate');
    handler?.({
      entity: 'mcpServers',
      action: 'save',
      entityId: 'new-server',
    });

    // Wait for refresh
    await waitFor(() => {
      // Assuming new server was added to DB
      expect(result.current.servers.length).toBeGreaterThan(initialCount);
    });
  });

  it('should refresh on local service change', async () => {
    const { result } = renderHook(() => useMCPServerRegistry(), {
      wrapper: MCPServerRegistryProvider,
    });

    // Save server (triggers local event)
    await result.current.saveServer({
      id: 'new-server',
      name: 'New Server',
      // ... other fields
    });

    // Context should auto-refresh
    await waitFor(() => {
      expect(result.current.servers.some((s) => s.id === 'new-server')).toBe(
        true,
      );
    });
  });
});
```

### 3. 통합 테스트: Worker → Main Thread 이벤트 전파

**파일**: `src/__tests__/integration/service-revalidation.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { webMCPProxy } from '@/lib/web-mcp/mcp-proxy';

describe('Service Revalidation Integration', () => {
  it('should propagate Worker save to Main Thread Context', async () => {
    await webMCPProxy.initialize();

    const notifications: RevalidateEvent[] = [];
    webMCPProxy.onNotify('service-revalidate', (data) => {
      notifications.push(data as RevalidateEvent);
    });

    // Call Worker tool to create server
    await webMCPProxy.callTool('mcp_manager', 'create_server', {
      name: 'Test Server',
      command: 'node',
      args: ['server.js'],
    });

    // Wait for notification
    await new Promise((resolve) => setTimeout(resolve, 200));

    expect(notifications).toContainEqual({
      entity: 'mcpServers',
      action: 'save',
      entityId: expect.any(String),
    });

    webMCPProxy.cleanup();
  });
});
```

---

## 작업 진행 순서 (권장)

1. **Phase 1: Service Layer 이벤트 추가** (Main Thread)
   - `LocalMcpServerService`에 `onRevalidate()` 메서드 추가
   - `save()/delete()` 시 이벤트 발생
   - 단위 테스트 작성

2. **Phase 2: Worker Service 알림 추가**
   - `McpServerService` (Unified)에서 `sendNotification()` 호출
   - `AssistantService` (Worker)에서도 동일 적용

3. **Phase 3: React Context 구독**
   - `MCPServerRegistryContext`에서 Worker 알림 구독
   - 로컬 서비스 이벤트 구독
   - `refreshAll()` 구현

4. **Phase 4: AssistantContext 동기화**
   - `AssistantContext`에서 Worker 알림 구독 추가
   - 동일 패턴 적용

5. **Phase 5: 통합 테스트**
   - Worker → Main Thread 이벤트 전파 검증
   - UI 자동 갱신 확인

---

## 추가 분석 과제

1. **최적화: Debounce/Throttle**
   - 짧은 시간에 여러 변경 발생 시 `refreshAll()` 호출 최적화
   - SWR의 `mutate()`와 유사한 메커니즘 고려

2. **선택적 갱신 (Optimistic Update)**
   - 전체 `refreshAll()` 대신 변경된 항목만 업데이트
   - `event.entityId`를 활용한 부분 갱신

3. **에러 처리**
   - `refreshAll()` 실패 시 재시도 로직
   - 사용자에게 에러 알림 표시

---

## Clarification Q-List

### Q1: 전체 갱신 vs 부분 갱신

**상황**: `service-revalidate` 이벤트 수신 시 갱신 전략

**옵션**:

- A) `refreshAll()` - 전체 목록 다시 로드 (단순, 안전)
- B) `event.entityId`로 해당 항목만 갱신 (최적화)
- C) Optimistic Update + 백그라운드 검증

**권장**: A (Phase 1), B (Phase 2 최적화)

### Q2: 로컬 이벤트 구독 필요성

**상황**: Main Thread에서 `LocalMcpServerService` 사용 시 자체 이벤트 발생

**옵션**:

- A) 로컬 이벤트도 구독 (이중 갱신 가능성)
- B) 로컬 변경 시 직접 상태 업데이트, Worker 변경만 이벤트로 처리
- C) 모든 변경을 이벤트로 통일

**권장**: C (일관성, 단순성)

### Q3: AssistantContext 동기화 우선순위

**상황**: AssistantContext도 동일 패턴 적용 필요

**옵션**:

- A) MCPServerRegistryContext와 동시 진행
- B) MCPServerRegistryContext 완료 후 순차 진행
- C) AssistantContext는 별도 계획으로 분리

**권장**: A (동일 패턴, 병렬 가능)

---

## End of Refactoring Plan 2
