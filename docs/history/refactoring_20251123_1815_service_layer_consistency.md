# Refactoring Plan: Service Layer Consistency for MCP Server Management

**Date**: 2025-11-23 18:15  
**Author**: AI Assistant  
**Status**: Proposal

---

## 작업의 목적

현재 `MCPServerRegistryContext`와 UI 컴포넌트(`MCPServerManagement`)가 서비스 레이어(`IMcpServerService`)를 우회하고 Dexie DB(`dbService`)에 직접 접근하여, 에이전트가 MCP 도구(`create_server`)로 서버를 등록할 때 UI가 자동 갱신되지 않는 문제가 발생합니다. 이를 해결하기 위해:

1. **서비스 레이어 일관성 확보**: Context와 UI가 `IMcpServerService`를 통해서만 DB에 접근하도록 리팩토링
2. **이벤트 기반 UI 동기화**: 서비스 레이어에서 변경 이벤트를 발행하여 외부 도구에 의한 DB 변경도 UI에 즉시 반영

---

## 현재의 상태 / 문제점

### 아키텍처 불일치

- **`AssistantContext`**: `AssistantService`를 통해 DB 접근 (올바른 패턴)
- **`MCPServerRegistryContext`**: `dbService.mcpServers` 직접 접근 (레이어 우회)
- **`MCPServerManagement`**: SWR fetcher에서 `dbService.mcpServers.getPage()` 직접 호출 (레이어 우회)

### 문제 시나리오

```typescript
// 에이전트가 MCP 도구로 서버 등록
await mcpService.save(newServer); // DB에 저장됨
// ❌ MCPServerRegistryContext는 이를 감지하지 못함 (수동 refreshAll 필요)
// ❌ UI는 구형 데이터 유지 (사용자가 새로고침하거나 다른 작업을 해야 갱신됨)
```

```typescript
// UI에서 직접 등록
await dbService.mcpServers.upsert(server); // DB에 저장
await refreshAll(); // Context가 직접 갱신 호출
// ✅ UI 즉시 갱신됨
```

### 근본 원인

1. **서비스 레이어 우회**: Context가 `IMcpServerService` 대신 `dbService` 직접 사용
2. **이벤트 부재**: 서비스 레이어에 변경 이벤트 발행 메커니즘이 없음
3. **Dexie LiveQuery 미사용**: Context가 DB 변경을 실시간 감지하지 못함

---

## 관련 코드의 구조 및 동작 방식 Summary

### Bird's Eye View

```
┌─────────────────────────────────────────────────────────────┐
│                        UI Layer                              │
│  ┌──────────────────┐          ┌──────────────────┐         │
│  │ AssistantEditor  │          │ MCPServerMgmt    │         │
│  └────────┬─────────┘          └────────┬─────────┘         │
│           │                              │                   │
└───────────┼──────────────────────────────┼───────────────────┘
            │                              │
┌───────────▼──────────────────────────────▼───────────────────┐
│                   Context Layer                              │
│  ┌──────────────────┐          ┌──────────────────┐         │
│  │ AssistantContext │          │ MCPServerRegistry│         │
│  │ (✅ uses Service)│          │ (❌ uses dbService)│       │
│  └────────┬─────────┘          └────────┬─────────┘         │
│           │                              │                   │
└───────────┼──────────────────────────────┼───────────────────┘
            │                              │
┌───────────▼──────────────────────────────▼───────────────────┐
│                   Service Layer                              │
│  ┌──────────────────┐          ┌──────────────────┐         │
│  │ AssistantService │          │ McpServerService │         │
│  │ (Local+Remote)   │          │ (Local+Remote)   │         │
│  └────────┬─────────┘          └────────┬─────────┘         │
│           │                              │                   │
│           │                              │ (Bypassed!)      │
│           │                              │                   │
└───────────┼──────────────────────────────┼───────────────────┘
            │                              │
┌───────────▼──────────────────────────────▼───────────────────┐
│                    Data Layer                                │
│                  ┌──────────────┐                            │
│                  │   dbService  │ ◄─────┐                    │
│                  │   (Dexie)    │       │ Direct Access     │
│                  └──────────────┘       │ from Context!     │
│                                         │                    │
└─────────────────────────────────────────┼────────────────────┘
                                          │
                           ┌──────────────┴──────────────┐
                           │ MCPServerRegistryContext    │
                           │ (Bypassing service layer)   │
                           └─────────────────────────────┘
```

### 현재 데이터 흐름

**AssistantContext (올바른 패턴)**:

```
UI → AssistantContext.saveAssistant()
  → AssistantService.save()
    → dbService.assistants.upsert()
      → Dexie DB
  → loadAssistants() (자동 갱신)
    → UI 재렌더링 ✅
```

**MCPServerRegistryContext (문제 패턴)**:

```
UI → MCPServerRegistryContext.saveServer()
  → dbService.mcpServers.upsert() (직접 접근!)
    → Dexie DB
  → refreshAll() (수동 갱신)
    → UI 재렌더링 ✅

Agent Tool → mcpService.save()
  → dbService.mcpServers.upsert()
    → Dexie DB
  → (이벤트 없음!) ❌
    → UI 갱신 안 됨 ❌
```

---

## 변경 이후의 상태 / 해결 판정 기준

### 목표 아키텍처 (RPC 기반 이벤트 확장)

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Thread                                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ UI Layer                                                 │
│  │  ┌──────────────────┐       ┌──────────────────┐        │
│  │  │ AssistantEditor  │       │ MCPServerMgmt    │        │
│  │  └────────┬─────────┘       └────────┬─────────┘        │
│  ├───────────┼──────────────────────────┼──────────────────┤
│  │ Context Layer                        │                  │
│  │  ┌──────────────────┐       ┌────────▼─────────┐        │
│  │  │ AssistantContext │       │ MCPServerRegistry│        │
│  │  │ (uses Service)   │       │ (✅ uses Service)│        │
│  │  └────────┬─────────┘       └────────┬─────────┘        │
│  │           │                           │                  │
│  │           │              ┌────────────▼──────────────┐   │
│  │           │              │ WebMCPProxy.onNotify()   │   │
│  │           │              │ (event handler)          │   │
│  │           │              └───────────────────────────┘   │
│  ├───────────┼──────────────────────────┼──────────────────┤
│  │ Service Layer (Main Thread Instance) │                  │
│  │  ┌────────▼─────────┐       ┌────────▼─────────┐        │
│  │  │ AssistantService │       │ McpServerService │        │
│  │  │                  │       │ (Read-only ops)  │        │
│  │  └──────────────────┘       └──────────────────┘        │
│  ├──────────────────────────────────────────────────────────┤
│  │ WebMCPProxy (RPC Communication)                         │
│  │  ┌──────────────────────────────────────────────┐       │
│  │  │ Request/Response Handler                     │       │
│  │  │ + Notification Event Handler ⬅ NEW!        │       │
│  │  └──────────────────────────────────────────────┘       │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ postMessage (bidirectional)
                           │ Request ➡ | ⬅ Response/Notification
                           │
┌──────────────────────────┴───────────────────────────────────┐
│                   Web Worker                                 │
│  ┌──────────────────────────────────────────────────────────┤
│  │ MCP Manager Module                                       │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ create_server, connect_server, etc.          │        │
│  │  └────────┬─────────────────────────────────────┘        │
│  ├───────────┼──────────────────────────────────────────────┤
│  │ Service Layer (Worker Instance)                          │
│  │  ┌────────▼─────────┐       ┌──────────────────┐        │
│  │  │ AssistantService │       │ McpServerService │        │
│  │  │                  │       │ ┌──────────────┐ │        │
│  │  │                  │       │ │Send notify() │ │        │
│  │  │                  │       │ │on save/delete│ │        │
│  │  └──────────────────┘       └─┴──────┬───────┴─┘        │
│  │                                       │                  │
│  ├───────────────────────────────────────┼──────────────────┤
│  │ Data Layer (Shared IndexedDB)        │                  │
│  │                             ┌─────────▼────────┐         │
│  │                             │   dbService      │         │
│  │                             │   (Dexie)        │         │
│  │                             └──────────────────┘         │
│  ├──────────────────────────────────────────────────────────┤
│  │ Worker Message Handler (mcp-worker.ts)                   │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ RPC Handler + sendNotification() ⬅ NEW!    │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

### 새로운 데이터 흐름 (RPC 기반 Notification)

**Main Thread (UI에서 직접 저장)**:

```typescript
UI → MCPServerRegistryContext.saveServer()
  → McpServerService.save()
    → dbService.mcpServers.upsert()
      → Dexie DB (IndexedDB)
  → (Same context) refreshAll() (즉시 갱신)
  → UI 재렌더링 ✅
```

**Web Worker (Agent Tool 사용)**:

```typescript
Agent Tool → create_server
  → mcpService.save() (Worker instance)
    → dbService.mcpServers.upsert()
      → Dexie DB (Shared with Main Thread)
    → sendNotification({ type: 'db-changed', entity: 'mcpServers', ... })
      → self.postMessage({ type: 'notification', ... })
        → Main Thread WebMCPProxy.handleWorkerMessage()
          → notificationHandlers['db-changed']()
            → MCPServerRegistryContext.refreshAll()
              → UI 즉시 갱신 ✅
```

### 해결 판정 기준

1. **서비스 레이어 일관성**
   - [ ] `MCPServerRegistryContext`가 `dbService` 대신 `McpServerService` 사용
   - [ ] `MCPServerManagement` SWR fetcher가 서비스 레이어 메서드 호출
   - [ ] `AssistantContext` 패턴과 동일한 구조 확보

2. **실시간 UI 동기화**
   - [ ] 에이전트가 `create_server` 도구로 서버 등록 시 UI 즉시 갱신
   - [ ] Settings 탭에서 서버 추가 시 즉시 반영 (기존 동작 유지)
   - [ ] AssistantEditor 활성 서버 목록 자동 갱신

3. **이벤트 메커니즘**
   - [ ] `McpServerService`에 onChange 콜백 지원 추가
   - [ ] Context가 서비스 이벤트 구독
   - [ ] 메모리 누수 방지 (cleanup on unmount)

---

## 수정이 필요한 코드 및 수정부분의 코드 스니펫

### 1. `IMcpServerService` 인터페이스 확장

**파일**: `src/lib/services/mcp-server-service.ts`

**수정 전**:

```typescript
export interface IMcpServerService {
  getAll(): Promise<MCPServerEntity[]>;
  getPage(page: number, pageSize: number): Promise<Page<MCPServerEntity>>;
  getById(id: string): Promise<MCPServerEntity | undefined>;
  getByName(name: string): Promise<MCPServerEntity | undefined>;
  save(server: MCPServerEntity): Promise<MCPServerEntity>;
  saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]>;
  delete(id: string): Promise<void>;
  count(): Promise<number>;
}
```

**수정 후**:

```typescript
export type MCPServerChangeListener = () => void | Promise<void>;

export interface IMcpServerService {
  getAll(): Promise<MCPServerEntity[]>;
  getPage(page: number, pageSize: number): Promise<Page<MCPServerEntity>>;
  getById(id: string): Promise<MCPServerEntity | undefined>;
  getByName(name: string): Promise<MCPServerEntity | undefined>;
  save(server: MCPServerEntity): Promise<MCPServerEntity>;
  saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]>;
  delete(id: string): Promise<void>;
  count(): Promise<number>;

  // Event subscription for real-time updates
  onChange(listener: MCPServerChangeListener): () => void;
}
```

### 2. `McpServerService` 구현체에 이벤트 발행 추가

**파일**: `src/lib/services/mcp-server-service.ts`

```typescript
export class McpServerService implements IMcpServerService {
  private localService: LocalMcpServerService;
  private remoteService: RemoteMcpServerService | null = null;
  private changeListeners: Set<MCPServerChangeListener> = new Set();

  // ... existing constructor ...

  /**
   * Subscribe to MCP server changes
   * @param listener Callback invoked when servers are modified
   * @returns Unsubscribe function
   */
  onChange(listener: MCPServerChangeListener): () => void {
    this.changeListeners.add(listener);
    return () => {
      this.changeListeners.delete(listener);
    };
  }

  /**
   * Notify all subscribers of data changes
   */
  private async notifyChange(): Promise<void> {
    const promises = Array.from(this.changeListeners).map((listener) =>
      Promise.resolve(listener()).catch((err) =>
        logger.error('Error in change listener', err),
      ),
    );
    await Promise.allSettled(promises);
  }

  async save(server: MCPServerEntity): Promise<MCPServerEntity> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.save(server);
        await this.localService.save(saved);
        await this.notifyChange(); // 🔔 Emit change event
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote', error);
        throw error;
      }
    }
    const result = await this.localService.save(server);
    await this.notifyChange(); // 🔔 Emit change event
    return result;
  }

  async saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.saveAll(servers);
        await this.localService.saveAll(saved);
        await this.notifyChange(); // 🔔 Emit change event
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote', error);
        throw error;
      }
    }
    const result = await this.localService.saveAll(servers);
    await this.notifyChange(); // 🔔 Emit change event
    return result;
  }

  async delete(id: string): Promise<void> {
    if (this.remoteService) {
      try {
        await this.remoteService.delete(id);
      } catch (error) {
        logger.error('Failed to delete from remote', error);
        throw error;
      }

      try {
        await this.localService.delete(id);
        await this.notifyChange(); // 🔔 Emit change event
      } catch (error) {
        logger.error(
          'Remote deletion succeeded, but failed to delete from local',
          error,
        );
        // Do not throw; treat as success, but log for reconciliation
      }
    } else {
      await this.localService.delete(id);
      await this.notifyChange(); // 🔔 Emit change event
    }
  }
}
```

### 3. `MCPServerRegistryContext` 서비스 레이어 사용 전환

**파일**: `src/context/MCPServerRegistryContext.tsx`

**수정 전**:

```typescript
import { dbService, dbUtils } from '@/lib/db/service';

export const MCPServerRegistryProvider = ({ children }: { children: React.ReactNode }) => {
  // ... state declarations ...

  const saveServer = useCallback(async (server: MCPServerEntity) => {
    try {
      await dbService.mcpServers.upsert(server); // ❌ Direct DB access
      await refreshAll();
      await invalidateMCPServerPages();
      logger.info(`MCP server "${server.name}" saved successfully`);
    } catch (err) {
      // ... error handling ...
    }
  }, [refreshAll]);

  const deleteServer = useCallback(async (id: string) => {
    try {
      await dbService.mcpServers.delete(id); // ❌ Direct DB access
      await refreshAll();
      await invalidateMCPServerPages();
      // ...
    }
  }, [refreshAll]);
}
```

**수정 후**:

```typescript
import { McpServerService } from '@/lib/services/mcp-server-service';
import { useSettings } from '@/hooks/use-settings';

export const MCPServerRegistryProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const { value: settings } = useSettings();
  const [allServers, setAllServers] = useState<MCPServerEntity[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const allServersRef = useRef<MCPServerEntity[]>([]);

  // Create service instance
  const mcpService = useMemo(() => {
    return new McpServerService(settings.agentHubUrl);
  }, [settings.agentHubUrl]);

  const refreshAll = useCallback(async () => {
    setLoading(true);
    try {
      const servers = await mcpService.getAll(); // ✅ Use service layer
      setAllServers(servers);
      setError(undefined);
      logger.debug(`Loaded ${servers.length} MCP servers from service`);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      logger.error('Failed to load MCP servers', err);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [mcpService]);

  // Subscribe to service changes
  useEffect(() => {
    const unsubscribe = mcpService.onChange(() => {
      logger.debug('MCP server changed, refreshing...');
      refreshAll(); // Auto-refresh on external changes
    });

    return () => {
      unsubscribe(); // Cleanup on unmount
    };
  }, [mcpService, refreshAll]);

  const saveServer = useCallback(
    async (server: MCPServerEntity) => {
      try {
        await mcpService.save(server); // ✅ Use service layer (auto-triggers onChange)
        await invalidateMCPServerPages();
        logger.info(`MCP server "${server.name}" saved successfully`);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        logger.error('Failed to save MCP server', err);
        throw new Error(`Failed to save server: ${message}`);
      }
    },
    [mcpService],
  );

  const deleteServer = useCallback(
    async (id: string) => {
      try {
        await mcpService.delete(id); // ✅ Use service layer (auto-triggers onChange)
        await invalidateMCPServerPages();
        logger.info(`MCP server with ID "${id}" deleted successfully`);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        logger.error('Failed to delete MCP server', err);
        throw new Error(`Failed to delete server: ${message}`);
      }
    },
    [mcpService],
  );

  // ... rest of implementation ...
};
```

### 4. `MCPServerManagement` SWR fetcher 수정

**파일**: `src/features/settings/MCPServerManagement.tsx`

**수정 전**:

```typescript
import { dbService } from '@/lib/db/service';

export function MCPServerManagement() {
  const { saveServer, deleteServer, toggleActive } = useMCPServerRegistry();

  const {
    data,
    isLoading,
    isValidating,
    setSize,
    mutate: mutateServers,
  } = useSWRInfinite(
    (pageIndex) => ['mcpServers', pageIndex],
    async ([, pageIndex]) => {
      return dbService.mcpServers.getPage(pageIndex + 1, 10); // ❌ Direct access
    },
    // ...
  );

  // ...
}
```

**수정 후**:

```typescript
import { McpServerService } from '@/lib/services/mcp-server-service';
import { useSettings } from '@/hooks/use-settings';

export function MCPServerManagement() {
  const { saveServer, deleteServer, toggleActive } = useMCPServerRegistry();
  const { value: settings } = useSettings();

  const mcpService = useMemo(() => {
    return new McpServerService(settings.agentHubUrl);
  }, [settings.agentHubUrl]);

  const {
    data,
    isLoading,
    isValidating,
    setSize,
    mutate: mutateServers,
  } = useSWRInfinite(
    (pageIndex) => ['mcpServers', pageIndex],
    async ([, pageIndex]) => {
      return mcpService.getPage(pageIndex + 1, 10); // ✅ Use service layer
    },
    // ...
  );

  // ...
}
```

### 5. Worker에서 BroadcastChannel 사용 확인

**파일**: `src/lib/web-mcp/modules/mcp-manager/server.ts`

**참고**: Worker 내부의 `McpServerService` 인스턴스도 동일하게 BroadcastChannel을 사용하므로, `create_server` 도구가 서버를 저장할 때 자동으로 Main Thread에 이벤트를 브로드캐스트합니다.

```typescript
// Worker 내부 (기존 코드 유지, 추가 수정 불필요)
async function getServices() {
  const agentHubUrlObj = await dbService.objects.read('agentHubUrl');
  const agentHubUrl = agentHubUrlObj?.value as string;

  if (cachedServices && cachedServices.agentHubUrl === agentHubUrl) {
    return {
      mcpService: cachedServices.mcpService,
      assistantService: cachedServices.assistantService,
    };
  }

  // ✅ Worker에서도 동일한 McpServerService 사용
  // BroadcastChannel이 자동으로 생성되어 Main Thread와 통신
  const mcpService = new McpServerService(agentHubUrl);
  const assistantService = new AssistantService(agentHubUrl);

  cachedServices = { mcpService, assistantService, agentHubUrl };
  return { mcpService, assistantService };
}

async function createServer(
  mcpService: IMcpServerService,
  args: Record<string, unknown>,
): Promise<MCPResult<CreateServerOutput>> {
  // ...
  await mcpService.save(server); // ✅ 자동으로 BroadcastChannel 이벤트 발행
  // ...
}
```

**동작 방식**:

1. Worker의 `create_server` → `mcpService.save()`
2. `McpServerService.broadcastChange()` 호출
3. BroadcastChannel → Main Thread로 메시지 전송
4. Main Thread의 `MCPServerRegistryContext`가 메시지 수신
5. `refreshAll()` 호출 → UI 갱신 ✅

---

## 재사용 가능한 연관 코드

### 참고: `AssistantContext`의 서비스 레이어 패턴

**파일**: `src/context/AssistantContext.tsx`

```typescript
// ✅ Good Example: Using service layer consistently
const assistantService = useMemo(() => {
  return new AssistantService(settings.agentHubUrl);
}, [settings.agentHubUrl]);

const saveAssistant = useCallback(
  async (assistant: Assistant) => {
    // ...
    await assistantService.save(assistantToSave); // Service layer
    await loadAssistants(); // Refresh after save
    // ...
  },
  [assistantService, loadAssistants],
);

const deleteAssistant = useCallback(
  async (assistantId: string) => {
    // ...
    await assistantService.delete(assistantId); // Service layer
    await loadAssistants(); // Refresh after delete
    // ...
  },
  [assistantService, loadAssistants],
);
```

### 참고: `mcp-manager` 서버가 사용하는 서비스 인스턴스

**파일**: `src/lib/web-mcp/modules/mcp-manager/server.ts`

```typescript
async function getServices(): Promise<{
  mcpService: IMcpServerService;
  assistantService: IAssistantService;
}> {
  const settings = await getSettings();
  return {
    mcpService: new McpServerService(settings.agentHubUrl),
    assistantService: new AssistantService(settings.agentHubUrl),
  };
}

async function createServer(
  mcpService: IMcpServerService,
  args: Record<string, unknown>,
): Promise<MCPResult<CreateServerOutput>> {
  // ...
  await mcpService.save(server); // Service layer usage
  // ...
}
```

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드

### 1. 서비스 레이어 이벤트 테스트

**파일**: `src/lib/services/__tests__/mcp-server-service.test.ts` (신규)

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { McpServerService } from '../mcp-server-service';
import { MCPServerEntity } from '@/models/chat';

describe('McpServerService', () => {
  let service: McpServerService;

  beforeEach(() => {
    service = new McpServerService();
  });

  it('should emit onChange event when saving a server', async () => {
    const listener = vi.fn();
    const unsubscribe = service.onChange(listener);

    const server: MCPServerEntity = {
      id: 'test-1',
      name: 'Test Server',
      isActive: true,
      createdAt: new Date(),
      updatedAt: new Date(),
      transport: { type: 'stdio', command: 'node', args: [] },
    };

    await service.save(server);

    expect(listener).toHaveBeenCalledTimes(1);

    unsubscribe();
  });

  it('should emit onChange event when deleting a server', async () => {
    const listener = vi.fn();
    service.onChange(listener);

    await service.delete('test-1');

    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('should unsubscribe listeners correctly', async () => {
    const listener = vi.fn();
    const unsubscribe = service.onChange(listener);

    unsubscribe();

    await service.save({
      id: 'test-2',
      name: 'Test',
      isActive: true,
      createdAt: new Date(),
      updatedAt: new Date(),
      transport: { type: 'stdio', command: 'test', args: [] },
    });

    expect(listener).not.toHaveBeenCalled();
  });
});
```

### 2. Context 이벤트 구독 테스트

**파일**: `src/context/__tests__/MCPServerRegistryContext.test.tsx` (신규)

```typescript
import { describe, it, expect, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import {
  MCPServerRegistryProvider,
  useMCPServerRegistry,
} from '../MCPServerRegistryContext';
import { McpServerService } from '@/lib/services/mcp-server-service';

vi.mock('@/lib/services/mcp-server-service');

describe('MCPServerRegistryContext', () => {
  it('should refresh when service emits change event', async () => {
    const mockOnChange = vi.fn((listener) => {
      // Simulate immediate trigger
      setTimeout(() => listener(), 100);
      return vi.fn(); // Return unsubscribe function
    });

    vi.mocked(McpServerService).mockImplementation(() => ({
      onChange: mockOnChange,
      getAll: vi.fn().mockResolvedValue([]),
      // ... other methods ...
    }));

    const { result } = renderHook(() => useMCPServerRegistry(), {
      wrapper: MCPServerRegistryProvider,
    });

    await waitFor(() => {
      expect(mockOnChange).toHaveBeenCalled();
    });

    // Verify refreshAll was triggered by onChange
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
  });
});
```

### 3. 통합 테스트: Agent Tool → UI 갱신

**파일**: `src/__tests__/integration/mcp-server-sync.test.tsx` (신규)

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MCPServerRegistryProvider } from '@/context/MCPServerRegistryContext';
import { mcpManagerServer } from '@/lib/web-mcp/modules/mcp-manager/server';

describe('MCP Server Synchronization', () => {
  it('should update UI when agent creates server via tool', async () => {
    render(
      <MCPServerRegistryProvider>
        <TestComponent />
      </MCPServerRegistryProvider>
    );

    // Agent calls create_server tool
    await mcpManagerServer.callTool('create_server', {
      name: 'New Agent Server',
      transport: { type: 'stdio', command: 'test', args: [] },
    });

    // Verify UI shows new server without manual refresh
    await waitFor(() => {
      expect(screen.getByText('New Agent Server')).toBeInTheDocument();
    });
  });
});
```

### 4. 기존 테스트 수정 포인트

- **`MCPServerManagement` 컴포넌트 테스트**: SWR fetcher가 서비스 레이어를 사용하도록 수정 후 mock 업데이트 필요
- **`mcp-manager` 도구 테스트**: `save()` 호출 후 onChange 이벤트 발생 검증 추가

---

## 작업 진행 순서 (권장)

1. **Phase 1: 서비스 레이어 이벤트 추가** (Breaking Change 없음)
   - `IMcpServerService`에 `onChange()` 메서드 추가
   - `LocalMcpServerService`, `McpServerService` 구현
   - 단위 테스트 작성 및 검증

2. **Phase 2: Context 전환** (UI 영향 최소화)
   - `MCPServerRegistryContext`에서 `McpServerService` 인스턴스 생성
   - `saveServer`, `deleteServer` 메서드를 서비스 레이어 호출로 변경
   - `useEffect`에서 `onChange` 이벤트 구독
   - 기능 테스트 (Settings 탭에서 서버 추가/삭제 정상 동작 확인)

3. **Phase 3: UI 컴포넌트 수정**
   - `MCPServerManagement` SWR fetcher를 서비스 레이어 사용으로 변경
   - 통합 테스트 작성 (Agent Tool → UI 갱신 시나리오)

4. **Phase 4: 검증 및 정리**
   - E2E 시나리오 테스트 (사용자 등록 + 에이전트 등록 혼합)
   - 성능 테스트 (이벤트 리스너 메모리 누수 검증)
   - 문서 업데이트 (`docs/architecture/data-flow.md`)

---

## 추가 분석 과제

1. **Remote Service 동기화 전략**
   - Remote API에서 변경 사항을 받아올 때 onChange 이벤트를 언제 발행할지 결정 필요
   - Polling vs WebSocket vs Server-Sent Events 검토

2. **이벤트 중복 방지**
   - Context의 `saveServer`가 서비스를 호출하고, 서비스가 onChange를 발행하면 Context의 `refreshAll`이 호출됨
   - 이로 인해 `refreshAll` → DB 조회가 중복 발생할 가능성
   - 해결책: `saveServer` 내부에서 `refreshAll` 호출 제거하고 onChange 이벤트에만 의존

3. **성능 최적화**
   - 다수의 서버를 일괄 저장(`saveAll`)할 때 이벤트를 한 번만 발행하도록 개선
   - Debounce/Throttle 적용 검토

4. **Error Boundary**
   - onChange 리스너에서 예외 발생 시 다른 리스너에 영향 없도록 격리 (현재 `Promise.allSettled` 사용으로 해결)

---

## Clarification Q-List (사용자 의사 결정 필요)

### Q1: 이벤트 발행 시점 선택

**상황**: Context의 `saveServer`가 서비스 레이어를 호출하면, 서비스가 onChange 이벤트를 발행하여 Context의 구독자가 `refreshAll`을 호출합니다. 이 경우 Context 내부에서 추가로 `refreshAll`을 호출하면 중복됩니다.

**옵션**:

- A) Context의 `saveServer`에서 명시적 `refreshAll` 제거, onChange 이벤트에만 의존
- B) 서비스 레이어에서 "silent save" 옵션 제공 (이벤트 발행 여부 제어)
- C) 현재처럼 중복 호출 허용 (최신 데이터 보장 우선)

**권장**: A (이벤트 중심 아키텍처 일관성)

### Q2: Remote Service onChange 발행 시점

**상황**: Remote API에서 데이터를 가져와 Local DB에 동기화할 때 onChange를 발행하면, UI가 불필요하게 갱신될 수 있습니다.

**옵션**:

- A) Remote fetch 후 항상 onChange 발행 (일관성 우선)
- B) Local DB 변경 시에만 onChange 발행 (성능 우선)
- C) 별도 onRemoteSync 이벤트 추가 (세밀한 제어)

**권장**: B (초기 로드/백그라운드 동기화 시 불필요한 렌더링 방지)

### Q3: 기존 `dbService` 직접 접근 코드 정리 범위

**상황**: Assistant/Session 등 다른 엔티티도 일부 Context에서 dbService를 직접 사용합니다.

**옵션**:

- A) MCP Server만 우선 리팩토링, 다른 엔티티는 추후 별도 작업
- B) 모든 엔티티를 한 번에 서비스 레이어로 전환
- C) 공통 Base Service/Context 패턴 수립 후 일괄 적용

**권장**: A (점진적 개선, 리스크 최소화)

### Q4: 테스트 커버리지 목표

**상황**: 새로운 이벤트 메커니즘에 대한 테스트 범위 결정

**옵션**:

- A) 단위 테스트만 (서비스 레이어 BroadcastChannel 동작)
- B) 단위 + 통합 테스트 (Context 구독 + UI 갱신)
- C) 단위 + 통합 + E2E (실제 Agent Tool 시나리오)

**권장**: B (핵심 동작 검증, E2E는 수동 테스트로 보완)

---

## End of Refactoring Plan
