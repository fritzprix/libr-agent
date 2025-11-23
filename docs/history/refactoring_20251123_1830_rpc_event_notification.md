# Refactoring Plan 1: RPC Proxy Event Notification Mechanism

**Date**: 2025-11-23 18:30  
**Author**: AI Assistant  
**Status**: Proposal  
**Dependencies**: None (Foundation for Plan 2 & 3)

---

## 작업의 목적

현재 `WebMCPProxy`는 Main Thread → Worker로의 요청-응답(RPC) 패턴만 지원합니다. Worker에서 발생한 데이터 변경 사항을 Main Thread에 알리기 위해, **Worker-initiated Notification** 메커니즘을 추가하여 양방향 이벤트 통신을 가능하게 합니다.

이는 다음 단계(Plan 2)에서 서비스 레이어가 DB 변경 시 UI를 자동으로 갱신하는 기반이 됩니다.

---

## 현재의 상태 / 문제점

### 단방향 통신 구조

```typescript
// Main Thread → Worker: ✅ 지원됨
await proxy.callTool('mcp_manager', 'create_server', { ... });

// Worker → Main Thread: ❌ 응답만 가능, 비동기 알림 불가
// Worker 내부에서 DB 변경 시 Main Thread가 이를 감지할 방법이 없음
```

### 현재 메시지 프로토콜

**`WebMCPMessage` (요청)**:

```typescript
interface WebMCPMessage {
  id: string;
  type: 'listTools' | 'callTool' | 'ping' | ...;
  serverName?: string;
  toolName?: string;
  args?: unknown;
}
```

**`MCPResponse` (응답)**:

```typescript
interface MCPResponse<T> {
  id: string; // 요청 ID와 매칭
  result?: { structuredContent?: T; textContent?: string };
  error?: string | { message: string };
}
```

### 문제점

1. **응답 전용 구조**: Worker는 요청에 대한 응답만 반환 가능
2. **비동기 이벤트 부재**: 요청 없이 Worker가 먼저 알림을 보낼 수 없음
3. **이벤트 핸들러 부재**: Main Thread에 알림 수신 핸들러가 없음

---

## 관련 코드의 구조 및 동작 방식 Summary

### Bird's Eye View

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Thread                                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ WebMCPProxy                                              │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ sendMessage() → Request                      │        │
│  │  │ handleWorkerMessage() → Response only        │        │
│  │  │ pendingRequests: Map<id, {resolve, reject}> │        │
│  │  │ ❌ No notification handler                   │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ postMessage
                           │ { id, result } ← Response
                           │
┌──────────────────────────┴───────────────────────────────────┐
│                   Web Worker                                 │
│  ┌──────────────────────────────────────────────────────────┤
│  │ mcp-worker.ts                                            │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ handleMessage() → Process request            │        │
│  │  │ self.postMessage({ id, result }) → Response  │        │
│  │  │ ❌ No way to send async notifications        │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

### 현재 통신 흐름

```typescript
// 1. Main Thread 요청
proxy.callTool('server', 'tool', args)
  → sendMessage({ id: '123', type: 'callTool', ... })
    → worker.postMessage({ id: '123', ... })
      → pendingRequests.set('123', { resolve, reject })

// 2. Worker 처리
handleMessage({ id: '123', ... })
  → Execute tool
    → self.postMessage({ id: '123', result: {...} })

// 3. Main Thread 응답
handleWorkerMessage({ id: '123', result })
  → pending = pendingRequests.get('123')
    → pending.resolve(result)
      → Promise resolves ✅

// ❌ Worker에서 자발적 알림 불가 (요청 ID 없음)
```

---

## 변경 이후의 상태 / 해결 판정 기준

### 목표 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Thread                                │
│  ┌──────────────────────────────────────────────────────────┤
│  │ WebMCPProxy                                              │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ sendMessage() → Request                      │        │
│  │  │ handleWorkerMessage() → Response + Notify    │        │
│  │  │ pendingRequests: Map<id, {resolve, reject}> │        │
│  │  │ ✅ notificationHandlers: Map<type, handler>  │        │
│  │  │ ✅ onNotify(type, handler)                   │        │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ postMessage
                           │ Response: { id, result }
                           │ Notification: { type: 'notify', ... } ← NEW!
                           │
┌──────────────────────────┴───────────────────────────────────┐
│                   Web Worker                                 │
│  ┌──────────────────────────────────────────────────────────┤
│  │ mcp-worker.ts                                            │
│  │  ┌──────────────────────────────────────────────┐        │
│  │  │ handleMessage() → Process request            │        │
│  │  │ self.postMessage({ id, result }) → Response  │        │
│  │  │ ✅ sendNotification(type, data)              │        │
│  │  │    → self.postMessage({ type: 'notify', ... })│       │
│  │  └──────────────────────────────────────────────┘        │
│  └──────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────┘
```

### 새로운 통신 흐름

**기존 RPC (변경 없음)**:

```typescript
proxy.callTool(...) → Request → Response → Promise.resolve()
```

**새로운 Notification (추가)**:

```typescript
// Worker side
sendNotification('db-changed', { entity: 'mcpServers', action: 'save' })
  → self.postMessage({
      type: 'notify',
      notifyType: 'db-changed',
      data: {...}
    })

// Main Thread side
proxy.onNotify('db-changed', (data) => {
  console.log('DB changed:', data);
  // React Context에서 refreshAll() 호출 가능
});
```

### 해결 판정 기준

1. **메시지 타입 확장**
   - [ ] `WebMCPMessage` 타입에 `'notify'` 추가
   - [ ] `WebMCPNotification` 인터페이스 정의

2. **Worker 측 알림 발송**
   - [ ] `sendNotification()` 헬퍼 함수 구현
   - [ ] 요청 ID 없이 알림만 전송 가능

3. **Main Thread 측 알림 수신**
   - [ ] `WebMCPProxy.onNotify()` 메서드 추가
   - [ ] 알림 타입별 핸들러 등록/제거 지원
   - [ ] `handleWorkerMessage()`에서 알림/응답 구분 처리

4. **테스트 시나리오**
   - [ ] Worker가 알림 발송 → Main Thread 핸들러 실행 확인
   - [ ] 여러 핸들러 동시 등록 가능
   - [ ] Cleanup 시 핸들러 제거 확인

---

## 수정이 필요한 코드 및 수정부분의 코드 스니펫

### 1. 메시지 타입 확장

**파일**: `src/lib/mcp/web-worker/message.ts`

**추가**:

```typescript
/**
 * Notification message sent from Worker to Main Thread
 * (asynchronous, no request ID required)
 */
export interface WebMCPNotification {
  /** Message type identifier */
  type: 'notify';
  /** Notification type (e.g., 'db-changed', 'server-status') */
  notifyType: string;
  /** Notification payload */
  data?: unknown;
}

/**
 * Combined message type for Worker → Main Thread communication
 */
export type WebMCPWorkerMessage = MCPResponse<unknown> | WebMCPNotification;
```

### 2. Worker 측 알림 발송 헬퍼

**파일**: `src/lib/web-mcp/mcp-worker.ts`

**추가**:

```typescript
/**
 * Send a notification from Worker to Main Thread
 * @param notifyType Type of notification
 * @param data Notification payload
 */
export function sendNotification(notifyType: string, data?: unknown): void {
  const notification: WebMCPNotification = {
    type: 'notify',
    notifyType,
    data,
  };
  self.postMessage(notification);
  logger.debug(`Notification sent: ${notifyType}`, data);
}

// Export for use in server modules
(
  self as DedicatedWorkerGlobalScope & {
    sendNotification: typeof sendNotification;
  }
).sendNotification = sendNotification;
```

### 3. Main Thread 측 알림 핸들러

**파일**: `src/lib/web-mcp/mcp-proxy.ts`

**추가 속성**:

```typescript
export class WebMCPProxy {
  private worker: Worker | null = null;
  private pendingRequests = new Map<...>();

  // ✅ NEW: Notification handlers
  private notificationHandlers = new Map<
    string,
    Set<(data: unknown) => void>
  >();

  // ... existing code ...
}
```

**추가 메서드**:

```typescript
/**
 * Register a notification handler for a specific notification type
 * @param notifyType Type of notification to listen for
 * @param handler Callback function to handle the notification
 * @returns Unsubscribe function
 */
onNotify(notifyType: string, handler: (data: unknown) => void): () => void {
  if (!this.notificationHandlers.has(notifyType)) {
    this.notificationHandlers.set(notifyType, new Set());
  }

  this.notificationHandlers.get(notifyType)!.add(handler);
  logger.debug(`Registered notification handler for: ${notifyType}`);

  // Return unsubscribe function
  return () => {
    const handlers = this.notificationHandlers.get(notifyType);
    if (handlers) {
      handlers.delete(handler);
      if (handlers.size === 0) {
        this.notificationHandlers.delete(notifyType);
      }
    }
    logger.debug(`Unregistered notification handler for: ${notifyType}`);
  };
}

/**
 * Remove all notification handlers (cleanup)
 */
private clearNotificationHandlers(): void {
  this.notificationHandlers.clear();
}
```

**수정 메서드**:

```typescript
/**
 * Handles incoming messages from the worker
 * Now supports both responses and notifications
 */
private handleWorkerMessage(event: MessageEvent<WebMCPWorkerMessage>): void {
  const message = event.data;

  // Check if this is a notification (no request ID)
  if ('type' in message && message.type === 'notify') {
    const notification = message as WebMCPNotification;
    this.handleNotification(notification);
    return;
  }

  // Otherwise, treat as response (existing logic)
  const response = message as MCPResponse<unknown>;

  if (!response || response.id === undefined || response.id === null) {
    logger.warn('Invalid worker response - missing id', { responseId: response?.id });
    return;
  }

  const pending = this.pendingRequests.get(String(response.id));
  if (!pending) {
    logger.warn('No pending request found for response', { responseId: response.id });
    return;
  }

  clearTimeout(pending.timeout);
  this.pendingRequests.delete(String(response.id));

  try {
    if (response.error) {
      const errorMessage =
        typeof response.error === 'string'
          ? response.error
          : response.error.message || 'Unknown error';
      logger.error('Worker returned error', {
        responseId: response.id,
        error: response.error,
        errorMessage,
      });
      pending.reject(new Error(errorMessage));
    } else {
      logger.debug('Worker returned success', {
        responseId: response.id,
        hasResult: !!response.result,
      });
      pending.resolve(response);
    }
  } catch (handleError) {
    logger.error('Error handling worker message', {
      responseId: response.id,
      handleError,
      response,
    });
    pending.reject(
      handleError instanceof Error
        ? handleError
        : new Error(String(handleError)),
    );
  }
}

/**
 * Handle notification from worker
 * @private
 */
private handleNotification(notification: WebMCPNotification): void {
  const { notifyType, data } = notification;
  logger.debug(`Received notification: ${notifyType}`, data);

  const handlers = this.notificationHandlers.get(notifyType);
  if (!handlers || handlers.size === 0) {
    logger.debug(`No handlers registered for notification: ${notifyType}`);
    return;
  }

  // Execute all registered handlers
  for (const handler of handlers) {
    try {
      handler(data);
    } catch (error) {
      logger.error(`Error in notification handler for ${notifyType}`, error);
    }
  }
}
```

**cleanup 메서드 수정**:

```typescript
cleanup(): void {
  logger.debug('Cleaning up WebMCP proxy', {
    pendingRequests: this.pendingRequests.size,
    notificationHandlers: this.notificationHandlers.size, // NEW
  });

  for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
    clearTimeout(timeout);
    reject(new Error('Worker terminated'));
  }
  this.pendingRequests.clear();

  this.clearNotificationHandlers(); // NEW

  if (this.worker) {
    this.worker.terminate();
    this.worker = null;
  }
  this.isInitialized = false;
}
```

---

## 재사용 가능한 연관 코드

### 참고: 현재 RPC 응답 처리 패턴

**파일**: `src/lib/web-mcp/mcp-proxy.ts`

```typescript
// 현재 구조: Request-Response만 처리
private handleWorkerMessage(event: MessageEvent<MCPResponse<unknown>>): void {
  const response = event.data;
  const pending = this.pendingRequests.get(String(response.id));
  // ... resolve/reject ...
}
```

**새로운 구조**: Response + Notification 모두 처리 (위 수정 코드 참조)

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드

### 1. Worker Notification 발송 테스트

**파일**: `src/lib/web-mcp/__tests__/mcp-worker.test.ts` (신규)

```typescript
import { describe, it, expect, vi } from 'vitest';
import { sendNotification } from '../mcp-worker';

describe('sendNotification', () => {
  it('should send notification via postMessage', () => {
    const postMessageSpy = vi.spyOn(self, 'postMessage');

    sendNotification('test-event', { key: 'value' });

    expect(postMessageSpy).toHaveBeenCalledWith({
      type: 'notify',
      notifyType: 'test-event',
      data: { key: 'value' },
    });
  });

  it('should send notification without data', () => {
    const postMessageSpy = vi.spyOn(self, 'postMessage');

    sendNotification('simple-event');

    expect(postMessageSpy).toHaveBeenCalledWith({
      type: 'notify',
      notifyType: 'simple-event',
      data: undefined,
    });
  });
});
```

### 2. Main Thread Notification 수신 테스트

**파일**: `src/lib/web-mcp/__tests__/mcp-proxy-notification.test.ts` (신규)

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { WebMCPProxy } from '../mcp-proxy';

describe('WebMCPProxy Notification', () => {
  let proxy: WebMCPProxy;
  let mockWorker: Worker;

  beforeEach(() => {
    mockWorker = {
      postMessage: vi.fn(),
      terminate: vi.fn(),
      onmessage: null,
      onerror: null,
    } as unknown as Worker;

    proxy = new WebMCPProxy({ workerInstance: mockWorker });
  });

  it('should register notification handler', () => {
    const handler = vi.fn();
    const unsubscribe = proxy.onNotify('test-event', handler);

    expect(unsubscribe).toBeInstanceOf(Function);
  });

  it('should call handler when notification received', async () => {
    const handler = vi.fn();
    proxy.onNotify('db-changed', handler);

    // Simulate worker notification
    const notification = {
      type: 'notify',
      notifyType: 'db-changed',
      data: { entity: 'mcpServers' },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).toHaveBeenCalledWith({ entity: 'mcpServers' });
  });

  it('should support multiple handlers for same event', async () => {
    const handler1 = vi.fn();
    const handler2 = vi.fn();

    proxy.onNotify('test-event', handler1);
    proxy.onNotify('test-event', handler2);

    const notification = {
      type: 'notify',
      notifyType: 'test-event',
      data: { value: 123 },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler1).toHaveBeenCalledWith({ value: 123 });
    expect(handler2).toHaveBeenCalledWith({ value: 123 });
  });

  it('should unsubscribe handler', async () => {
    const handler = vi.fn();
    const unsubscribe = proxy.onNotify('test-event', handler);

    unsubscribe();

    const notification = {
      type: 'notify',
      notifyType: 'test-event',
      data: {},
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).not.toHaveBeenCalled();
  });

  it('should not interfere with RPC responses', async () => {
    const handler = vi.fn();
    proxy.onNotify('test-event', handler);

    // Simulate RPC response (not notification)
    const response = {
      id: '123',
      result: { structuredContent: 'test' },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: response }));

    // Notification handler should not be called
    expect(handler).not.toHaveBeenCalled();
  });

  it('should clear handlers on cleanup', () => {
    const handler = vi.fn();
    proxy.onNotify('test-event', handler);

    proxy.cleanup();

    const notification = {
      type: 'notify',
      notifyType: 'test-event',
      data: {},
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).not.toHaveBeenCalled();
  });
});
```

### 3. 통합 테스트 시나리오

**파일**: `src/__tests__/integration/worker-notification.test.ts` (신규)

```typescript
import { describe, it, expect, vi } from 'vitest';
import { WebMCPProxy } from '@/lib/web-mcp/mcp-proxy';

describe('Worker Notification Integration', () => {
  it('should notify Main Thread when Worker emits event', async () => {
    // Create real worker instance
    const proxy = new WebMCPProxy({
      workerPath: new URL('@/lib/web-mcp/mcp-worker.ts', import.meta.url),
    });

    await proxy.initialize();

    const notificationReceived = vi.fn();
    proxy.onNotify('test-notification', notificationReceived);

    // Trigger worker to send notification
    // (requires implementing test tool in worker)
    await proxy.callTool('test-server', 'emit-notification', {
      type: 'test-notification',
      data: { message: 'hello' },
    });

    // Wait for async notification
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(notificationReceived).toHaveBeenCalledWith({ message: 'hello' });

    proxy.cleanup();
  });
});
```

---

## 작업 진행 순서 (권장)

1. **Phase 1: 타입 정의** (Breaking Change 없음)
   - `WebMCPNotification` 인터페이스 추가
   - `WebMCPWorkerMessage` 유니온 타입 정의
   - 타입 검증 테스트

2. **Phase 2: Worker 측 구현**
   - `sendNotification()` 헬퍼 함수 구현
   - 간단한 테스트 도구로 동작 확인

3. **Phase 3: Main Thread 측 구현**
   - `WebMCPProxy.onNotify()` 메서드 추가
   - `handleWorkerMessage()` 수정 (응답/알림 구분)
   - 단위 테스트 작성

4. **Phase 4: 통합 검증**
   - Worker → Main Thread 알림 시나리오 테스트
   - 여러 핸들러 동시 등록 테스트
   - Cleanup 동작 검증

---

## 추가 분석 과제

1. **알림 타입 네이밍 컨벤션**
   - `db-changed`, `server-status`, `tool-progress` 등 표준 타입 정의 필요
   - 타입별 payload 스키마 문서화

2. **성능 고려사항**
   - 알림 빈도가 높을 경우 throttle/debounce 필요 여부 검토
   - 핸들러 실행 순서 보장 여부 결정

3. **에러 핸들링**
   - 핸들러 내부 예외 발생 시 다른 핸들러 실행 계속 보장
   - 로깅 전략 수립

---

## Clarification Q-List

### Q1: 알림 우선순위

**상황**: 여러 핸들러가 등록된 경우 실행 순서

**옵션**:

- A) 등록 순서대로 실행 (FIFO)
- B) 우선순위 파라미터 추가
- C) 순서 보장 없음 (Set 사용)

**권장**: A (FIFO, 예측 가능)

### Q2: 알림 유실 방지

**상황**: Main Thread가 아직 핸들러를 등록하기 전에 Worker가 알림을 보내면 유실됨

**옵션**:

- A) 유실 허용 (실시간 알림만 필요)
- B) Worker에 알림 큐 추가 (일정 시간 보관)
- C) Main Thread 준비 완료 신호 필요

**권장**: A (복잡도 증가 방지, 실시간성 우선)

### Q3: TypeScript 타입 안전성

**상황**: 알림 타입별 payload 타입 지정

**옵션**:

- A) `data: unknown` 유지, 런타임 검증
- B) Generic으로 타입 파라미터화 `onNotify<T>(type, handler: (data: T) => void)`
- C) 알림 타입별 인터페이스 맵 정의

**권장**: B (타입 안전성 + 유연성)

---

## End of Refactoring Plan 1
