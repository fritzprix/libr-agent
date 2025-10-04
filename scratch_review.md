# scratch.md 개선 계획 리뷰 및 현재 코드 정합성 분석

## 📋 요약

scratch.md에서 제안한 Built-In Tools 관리 구조 개선 계획을 현재 코드베이스와 비교 분석한 결과입니다.

---

## 1️⃣ 제안: `switchContext` 인터페이스 추가

### 📝 제안 내용 · 1️⃣

```typescript
export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
  getServiceContext?: (options?: ServiceContextOptions) => Promise<string>;
  switchContext: ({ sessionId, assistantId }) => Promise<void>; // 새로 추가
}
```

### ✅ 현재 상태 · 1️⃣

**파일**: `src/features/tools/index.tsx` (Line 37-43)

```typescript
export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
  getServiceContext?: (options?: ServiceContextOptions) => Promise<string>;
  // switchContext가 존재하지 않음
}
```

### 🔍 분석 · 1️⃣

-- ❌ **`switchContext` 메서드가 현재 인터페이스에 존재하지 않음**

- ✅ `ServiceContextOptions`에 `sessionId`가 이미 포함되어 있음 (Line 22-28)
- ✅ `getServiceContext`는 이미 `options?: ServiceContextOptions`를 받아서 sessionId를 전달할 수 있음

### 💡 권장사항 · 1️⃣

**세션/어시스턴트 컨텍스트 전환 시 내부 상태를 선제적으로 맞추기 위해 `switchContext`를 인터페이스에 추가하는 편이 안전합니다.**

**핵심 포인트:**

1. `switchContext`는 외부 도구 계약이 아니라 `BuiltInToolProvider` ↔︎ 서비스 간 협업용 내부 훅으로만 사용합니다.
2. 세션 전환 시 필요한 프리로드(예: IndexedDB 캐시 로드, WebSocket 재구독)를 한 번에 처리할 수 있어, 후속 `getServiceContext` 호출이 더 가볍고 실패 가능성이 낮아집니다.
3. 모든 서비스가 세션 의존적이지는 않으므로 **선택적(optional)** 메서드로 선언하되, 세션 상태를 유지하는 서비스에는 구현을 강제하는 가이드가 필요합니다.

```typescript
export interface BuiltInService {
  // ...existing fields...
  switchContext?: (options: ServiceContextOptions) => Promise<void>;
}
```

> 인터페이스에 명시만 되어 있다면 외부에서 호출하지 않도록 주석과 네이밍 가이드를 남기고, 호출 지점은 `BuiltInToolProvider` 내부에만 둡니다.

---

## 2️⃣ 제안: Session/Assistant 전환 시 `switchContext` 일괄 호출

### 📝 제안 내용 · 2️⃣

```typescript
// BuiltInToolProvider에서 useSession hook을 구독
// session 또는 assistant 변화 시 useEffect에서 switchContext를 일괄적으로 호출
```

### ✅ 현재 상태 · 2️⃣

**파일**: `src/features/tools/index.tsx` (Line 78)

```typescript
const { getCurrentSession } = useSessionContext();
```

- `useSessionContext`를 통해 현재 세션을 가져오고 있음
- `buildToolPrompt`에서 현재 세션 ID를 가져와 각 서비스의 `getServiceContext`에 전달

### 🔍 분석 · 2️⃣

- ✅ **BuiltInToolProvider는 세션 컨텍스트를 구독하고 있음**
- ❗ **구체화 필요:** `BuiltInToolProvider` 자체에는 `switchContext`를 자동 호출하는 useEffect가 없습니다. 그러나 WebMCP 서버들에 대해서는 `WebMCPContextSetter`가 세션/어시스턴트 변경을 감지하고 `setContext`를 전송하는 자동 동기화 로직이 이미 존재합니다.
- ✅ `buildToolPrompt`는 호출될 때마다 최신 세션 정보를 사용

### 💡 권장사항 · 2️⃣

**세션 또는 현재 어시스턴트가 바뀌면 즉시 각 서비스의 `switchContext`를 호출해 내부 캐시·구독 상태를 동기화해야 합니다.**

1. `useSessionContext`에서 `currentSession?.id`와 현재 어시스턴트 id를 구독하고, 변경 시 `switchContext`를 순차 호출합니다.
2. `switchContext`는 실패해도 전체 앱이 멈추지 않도록 `Promise.allSettled` 패턴을 이용해 개별 오류를 로깅합니다.
3. 세션이 없는 초기 상태에서는 모든 서비스에 `undefined`를 전달해 정리(clean-up)를 유도할 수 있게 해두는 편이 좋습니다.

```typescript
useEffect(() => {
  const sessionId = currentSession?.id;
  const assistantId = currentAssistant?.id;
  const readyServices = Array.from(serviceEntriesRef.current.values()).filter(
    (entry) => entry.status === 'ready' && entry.service.switchContext,
  );

  if (readyServices.length === 0) return;

  Promise.allSettled(
    readyServices.map((entry) =>
      entry.service.switchContext?.({ sessionId, assistantId }),
    ),
  ).then((results) => {
    results.forEach((result, index) => {
      if (result.status === 'rejected') {
        const { service } = readyServices[index];
        logger.error('switchContext failed', {
          serviceId: service.metadata.displayName,
          err: result.reason,
        });
      }
    });
  });
}, [currentSession?.id, currentAssistant?.id]);
```

> `switchSession`이 내부 훅이라는 점을 강조하고, 호출 로직은 `BuiltInToolProvider`에 집중시켜 외부 노출을 차단합니다.

---

## 3️⃣ 제안: Legacy 인터페이스 제거 (`setContext` / `setServerContext`)

### 📝 제안 내용 · 3️⃣

```text
기존 setContext / setServerContext 등의 별도 interface가 있었으나
새로운 switchSession으로 migration을 통해 하나의 일관된 interface로 통합
```

### ✅ 현재 상태 · 3️⃣

**검색 결과 (정정)**: WebMCP 계층에서 `setContext` 관련 코드가 다수 존재합니다 (planning, playbook 서버 모듈, 프록시 및 워커 메시지 핸들러 등).

```text
참고 파일: src/lib/web-mcp/modules/planning-server.ts (setContext 구현)
참고 파일: src/lib/web-mcp/modules/playbook-store.ts (setContext 구현)
참고 파일: src/lib/web-mcp/mcp-proxy.ts (proxy.setContext)
참고 파일: src/lib/web-mcp/mcp-worker.ts ("setContext" 메시지 처리)
참고 파일: src/lib/web-mcp/WebMCPContextSetter.tsx (세션/어시스턴트 변경 시 setContext 호출)
```

### 🔍 분석 · 3️⃣

- ⚠️ **정정:** `setContext`는 WebMCP 레이어에서 활성화되어 있으며 여러 모듈이 이를 사용합니다. WebMCP 계층에서는 여전히 컨텍스트 전송(setContext)이 표준 방식으로 사용되고 있습니다.
- ❗ 다만 `BuiltInToolProvider` 측면에서 제안한 `switchSession`(또는 `switchContext`) 같은 통합된 전환 훅은 아직 구현되어 있지 않습니다. 즉, 레이어별로는 컨텍스트 전송이 존재하지만, Provider 차원의 일관된 전환 루틴은 미구현 상태입니다.

### 💡 권장사항 · 3️⃣

**위와 같은 이유로, WebMCP의 `setContext` 사용을 인지한 채로 `BuiltInToolProvider` 차원에서 `switchContext`/`switchSession` 통합 훅을 설계·도입하는 것이 바람직합니다.**

---

## 4️⃣ 제안: `getServiceContext` 반환 타입 개선

### 📝 제안 내용 · 4️⃣

```typescript
interface ServiceContext<T> {
  contextPrompt: string;
  structuredState: T;
}

// 변경: getServiceContext -> Promise<ServiceContext<T>>
```

### ✅ 현재 상태 · 4️⃣

**파일**: `src/features/tools/index.tsx` (Line 43)

```typescript
export interface BuiltInService {
  getServiceContext?: (options?: ServiceContextOptions) => Promise<string>;
}
```

**사용 예시**:

- `BrowserToolProvider.tsx` (Line 152-177): 문자열 반환
- `WebMCPServiceRegistry.tsx` (Line 155-156): 문자열 반환
- `RustMCPToolProvider.tsx`: `getServiceContext` 구현 없음 (직접 rust backend 호출)

### 🔍 분석 · 4️⃣

- ❌ **현재는 `Promise<string>`만 반환**
- ❌ **Structured state를 반환하는 메커니즘이 없음**

### 💡 권장사항 · 4️⃣

**이 제안은 매우 유용하며, 구현 가치가 높습니다.**

**구현 방안:**

#### Step 1: 인터페이스 수정

```typescript
// src/features/tools/index.tsx

export interface ServiceContext<T = unknown> {
  contextPrompt: string;
  structuredState?: T; // optional로 하여 기존 호환성 유지
}

export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;

  // 개선된 반환 타입
  getServiceContext?: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown> | string>; // 기존 string도 허용하여 호환성 유지
}
```

#### Step 2: BuiltInToolProvider 수정

```typescript
// src/features/tools/index.tsx - buildToolPrompt 수정

const buildToolPrompt = useCallback(async (): Promise<string> => {
  const prompts: string[] = [];
  const serviceContexts: Record<string, unknown> = {};

  // ... existing code ...

  if (currentSession?.id) {
    for (const [serviceId, entry] of serviceEntries.entries()) {
      if (entry.status === 'ready' && entry.service.getServiceContext) {
        try {
          const result = await entry.service.getServiceContext(contextOptions);

          // 문자열 또는 ServiceContext 처리
          if (typeof result === 'string') {
            if (result) prompts.push(result);
          } else {
            if (result.contextPrompt) prompts.push(result.contextPrompt);
            if (result.structuredState) {
              serviceContexts[serviceId] = result.structuredState;
            }
          }
        } catch (err) {
          logger.error('Failed to get service context', { serviceId, err });
        }
      }
    }
  }

  // serviceContexts를 context에 저장하여 UI에서 사용 가능하도록
  // ... (별도 상태 관리 필요)

  return prompts.join('\n\n');
}, [serviceEntries, availableTools.length, getCurrentSession]);
```

#### Step 3: Context에 structuredState 추가

```typescript
interface BuiltInToolContextType {
  register: (serviceId: string, service: BuiltInService) => void;
  unregister: (serviceId: string) => void;
  availableTools: MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  buildToolPrompt: () => Promise<string>;
  status: Record<string, ServiceStatus>;
  getServiceMetadata: (alias: string) => ServiceMetadata | null;

  // 새로 추가
  serviceContexts: Record<string, unknown>;
}
```

#### Step 4: 각 서비스 구현 업데이트

```typescript
// BrowserToolProvider.tsx - 예시
getServiceContext: async (
  options?: ServiceContextOptions,
): Promise<ServiceContext<BrowserState>> => {
  try {
    const sessions = await listBrowserSessions();

    const contextPrompt = sessions.length === 0
      ? '# Browser Sessions\nNo active browser sessions.'
      : `# Browser Sessions\n${sessions.map(s =>
          `Session ${s.id}: ${s.url || 'No URL'} (${s.title || 'Untitled'})`
        ).join('\n')}`;

    return {
      contextPrompt,
      structuredState: {
        sessions,
        activeSessions: sessions.length,
      },
    };
  } catch (error) {
    return {
      contextPrompt: '# Browser Sessions\nError loading browser sessions.',
      structuredState: { error: String(error) },
    };
  }
},
```

---

## 5️⃣ 제안: UI 컴포넌트에서 structuredState 활용

### 📝 제안 내용 · 5️⃣

```text
- ChatPlanningPanel.tsx
- WorkspaceFilesPanel.tsx
- SessionFilesPopover.tsx

각 도구의 상태를 UI에 시각화하기 위해 serviceContext: Record<string, unknown> 추가
```

### ✅ 현재 상태 · 5️⃣

**분석한 파일들:**

1. **ChatPlanningPanel.tsx**:
   - `useWebMCPServer` 훅 사용
   - Planning 서버의 `get_current_state()` 직접 호출
   - **BuiltInToolContext를 사용하지 않음**

2. **WorkspaceFilesPanel.tsx**:
   - `useRustBackend` 훅 사용
   - `listWorkspaceFiles()` 직접 호출
   - **BuiltInToolContext를 사용하지 않음**

3. **SessionFilesPopover.tsx**:
   - `useRustMCPServer` 훅 사용
   - ContentStore 서버의 `readContent()` 직접 호출
   - **BuiltInToolContext를 사용하지 않음**

### 🔍 분석 · 5️⃣

- ❌ **현재 UI 컴포넌트들은 BuiltInToolContext를 사용하지 않음**
- ✅ 각 컴포넌트가 개별적으로 서버 상태를 관리
- 🤔 **이 구조가 실제로 문제인가?**

### 💡 권장사항 · 5️⃣

**서비스별 UI가 제각각 API를 호출하는 대신, 공통 `serviceContext`를 통해 상태를 주입받도록 단계적으로 전환하는 편이 유지보수에 유리합니다.**

1. `getServiceContext`가 `contextPrompt`와 `structuredState`를 함께 반환하도록 확장하면, 모든 UI 패널이 동일한 진입점에서 데이터를 얻을 수 있습니다.
2. `structuredState`에는 서비스 고유 타입을 넣되, `serviceId`를 키로 쓰는 맵과 제네릭 훅으로 타입을 안전하게 구별합니다 (예: `ServiceContextRegistry = Record<string, unknown>` + `as const` 키 매핑).
3. UI 컴포넌트는 `useServiceContext<'planning'>()` 같이 명시적 타입 파라미터를 제공해 새로운 필드가 추가돼도 타입 추론을 통해 즉시 컴파일 오류를 받을 수 있습니다.
4. 직접 호출이 필요한 케이스(예: 대화형 폴링)는 점진적으로 감싸되, 최소한 **소스 오브 트루스는 BuiltInToolProvider**가 되도록 합의합니다.

```typescript
type ServiceContextRegistry = {
  planning?: PlanningServiceState;
  contentStore?: ContentStoreState;
  workspace?: WorkspaceIndexState;
  // ... 추가 서비스는 여기서 선언적으로 확장
};

function useServiceContext<K extends keyof ServiceContextRegistry>(
  key: K,
): ServiceContextRegistry[K] | undefined {
  const { serviceContexts } = useBuiltInTool();
  return serviceContexts[key] as ServiceContextRegistry[K] | undefined;
}
```

> 새로운 서비스가 생기면 레지스트리 타입에만 필드를 추가하면 되므로, “함수명을 하나씩 새로 만든다”는 우려 없이 확장할 수 있습니다.

---

## 6️⃣ 제안: `useServiceContext<T>(serviceName)` 훅 추가

### 📝 제안 내용 · 6️⃣

```typescript
// serviceContext를 쉽게 구독할 수 있도록
useServiceContext<T>(serviceName);
```

### 🔍 분석 · 6️⃣

위의 5️⃣ 분석과 동일한 결론

### 💡 권장사항 · 6️⃣

- `serviceContexts` 맵과 제네릭 훅을 도입하기로 결정했다면, `useServiceContext`는 함께 제공되는 것이 자연스럽습니다.
- 초기에 구현 범위를 제한하려면 `serviceContextRegistry` 타입을 별도 파일로 분리해 서비스가 자기 타입을 선언적으로 등록할 수 있게 만듭니다.
- `switchSession`과 `getServiceContext` 확장 후, UI에서 실제로 데이터를 소비하도록 **최소 한 개 패널(ChatPlanningPanel 등)** 을 마이그레이션하며 검증하는 것이 좋습니다.

---

## 🎯 최종 권장사항 요약

### ✅ 구현하면 좋은 것

1. **`switchSession` 내부 인터페이스 추가** (1️⃣, 2️⃣) — 세션 의존 서비스가 캐시·구독을 즉시 재구성할 수 있도록 `BuiltInService`에 명시하고, 외부 노출 없이 `BuiltInToolProvider`가 호출 책임을 집니다.

2. **`getServiceContext` 반환 타입 개선** (4️⃣) — `Promise<string>` 대신 `Promise<ServiceContext<T> | string>`을 반환해 문자열 프롬프트와 구조화된 상태를 동시에 전달하고, 기존 구현과의 호환성을 유지합니다.

3. **공통 `serviceContexts` 맵과 `useServiceContext` 훅 도입** (5️⃣, 6️⃣) — 서비스별 UI에 동일한 진입점을 제공해 타입 구분과 확장을 단일 지점에서 관리하고, 새 필드 추가 시 컴파일 단계에서 영향 범위를 즉시 확인할 수 있게 합니다.

### ❌ 구현하지 않는 것이 좋은 것

- 현재 분석 관점에서는 별도 금기 사항 없음. 다만 `switchSession`을 외부 API로 노출하거나, 서비스가 자체적으로 세션 전환을 감지하도록 중복 로직을 두는 것은 피해야 합니다.

### ✅ 이미 완료된 것

1. **Legacy 인터페이스 제거** (3️⃣)
   - `setContext` / `setServerContext` 관련 코드 없음

---

## 📝 제안하는 최소 변경 계획

현재 코드베이스와 가장 잘 맞는 최소한의 개선:

```typescript
// src/features/tools/index.tsx

// 1. ServiceContext 타입 추가 (optional structuredState)
export interface ServiceContext<T = unknown> {
  contextPrompt: string;
  structuredState?: T;
}

// 2. BuiltInService 인터페이스 수정 (하위 호환성 유지)
export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
  getServiceContext?: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown> | string>; // 기존 string도 허용
}

// 3. buildToolPrompt에서 두 타입 모두 처리
// (이미 위 섹션에서 설명함)
```

이렇게 하면:

- ✅ 기존 코드와 호환성 유지
- ✅ 점진적으로 각 서비스를 개선 가능
- ✅ 필요한 서비스만 structuredState 반환
- ✅ 불필요한 복잡도 증가 없음

---

## 결론

scratch.md의 제안 중 일부는 현재 아키텍처와 맞지 않거나 불필요합니다.
**가장 가치 있는 개선은 `getServiceContext`의 반환 타입을 확장하는 것**이며,
나머지는 현재 구조를 유지하는 것이 더 적절합니다.
