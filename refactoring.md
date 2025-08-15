# Refactoring Plan — Integrate `@mcp-ui/client` for UIResource rendering

목표: 외부 `@mcp-ui/client` 패키지를 표준 renderer로 도입하여 MCP 서버가 반환하는 UI 리소스(`UIResource`)를 안전하게 렌더링하고, 액션 이벤트를 호스트(MCP)로 전달할 수 있도록 전체 흐름을 정비합니다.

요구사항 체크리스트

- 프로젝트 종속성에 `@mcp-ui/client`를 추가하고 빌드/타입 체크가 통과해야 합니다.
- Message 모델과 MCP 타입을 `@mcp-ui` 스펙과 호환되도록 조정(필요시 매핑 유틸 추가).
- `ToolCaller`가 MCP 응답에 포함된 `resource` 항목을 구조적으로 보존하여 `Message.uiResource`에 넣어야 합니다.
- `use-unified-mcp`, `use-web-mcp`, `use-mcp-server`는 `UIResource`/`MCPResponse`를 손상시키지 않고 전달.
- `MessageBubbleRouter`는 `uiResource`를 감지하면 `@mcp-ui/client`의 `UIResourceRenderer`(React)로 라우팅.
- `UI`에서 발생한 액션(onUIAction)은 적절한 훅(예: `useUnifiedMCP`)을 통해 도구 호출이나 Intent로 변환.
- 보안: iframe sandbox, origin 검증, X-Frame-Options fallback을 구현.
- 테스트: 단위 + E2E 스모크(모의 web tool)를 추가.

우선순위(권장)

1. 패키지 설치 및 타입 확인
2. 타입(모델) 및 MCP 타입 정렬/매핑 유틸 추가
3. `ToolCaller` 변경 — 구조화된 UIResource 보존
4. `MessageBubbleRouter`에서 `UIResourceRenderer` 사용(단계적 통합)
5. onUIAction 핸들러 연결 및 보안 검증
6. 테스트 및 문서화

설치 (빠르게 적용하려면)

```bash
pnpm add @mcp-ui/client@latest
# 또는 프로젝트 정책에 따라 특정 버전 핀
# pnpm add @mcp-ui/client@1.2.3
pnpm install
pnpm run build
```

파일별 구체 작업 지침

1. `src/models/chat.ts` — 타입 정렬 및 `UIResource` 매핑

- 목표: 앱 내부 `UIResource` 타입이 `@mcp-ui` 스펙과 호환되도록 정의합니다.
- 변경 (권장 위치: `src/models/chat.ts` 상단에 추가)

```ts
// src/models/chat.ts
export interface UIResource {
  uri?: string; // ui://... 식별자
  mimeType: string; // 'text/html' | 'text/uri-list' | 'application/vnd.mcp-ui.remote-dom'
  text?: string;
  blob?: string;
  metadata?: Record<string, unknown>;
}

export interface Message {
  // ...existing fields
  content: string;
  uiResource?: UIResource | UIResource[];
}
```

- 주의: `@mcp-ui/client`의 타입이 프로젝트의 타입 시스템(경로 alias 등)과 일치하지 않으면 매핑 유틸을 만듭니다.

2. `src/lib/mcp-types.ts` — MCPResponse content union 확장

- MCP 응답의 `result.content` 항목이 `{ type:'resource', resource: UIResource }`를 포함할 수 있도록 타입을 확장합니다.

```ts
// 예시
export type MCPContentItem =
  | { type: 'text'; text: string }
  | { type: 'resource'; resource: UIResource }
  | { type: 'file' /* ... */ };
```

3. `src/features/chat/ToolCaller.tsx` — 구조화된 UIResource 보존

- 현재: `serializeToolResult`가 모든 것을 문자열로 만든 후 `Message.content`로 저장합니다. 이 부분을 고칩니다.
- 변경 포인트:
  - `serializeToolResult`가 `SerializedToolResult` 객체({ text?, uiResource?, metadata })를 반환하게 수정
  - `toolResults.push(...)` 시 `uiResource` 필드를 포함한 구조화된 `Message` 객체 생성

```ts
// ToolCaller 내 변경 요지
const serialized = serializeToolResult(
  mcpResponse,
  toolName,
  executionStartTime,
);

toolResults.push({
  id: createId(),
  assistantId: currentAssistant?.id,
  role: 'tool',
  content: serialized.text || '',
  uiResource: serialized.uiResource,
  tool_call_id: toolCall.id,
  sessionId: currentSession?.id || '',
});
```

- 권장: `mcpResponse.result.content`를 순회하여 `type==='resource'` 항목을 모두 `uiResource[]`로 수집.

4. `src/hooks/use-unified-mcp.ts` & `src/hooks/use-web-mcp.ts` — web-branch 처리

- 목표: web 툴이 `UIResource`를 반환하면 이를 적절한 `MCPResponse`로 래핑하거나 그대로 전달.
- 핵심: web tool 결과를 검사하는 헬퍼 추가

```ts
const looksLikeUIResource = (v: unknown): v is UIResource =>
  !!v && typeof v === 'object' && 'mimeType' in (v as any);

// executeToolCall의 web branch
const result = await executeWebTool(toolName, args);
if (isMCPResponse(result)) return result;
if (looksLikeUIResource(result)) {
  return {
    jsonrpc: '2.0',
    id: toolCall.id,
    result: { content: [{ type: 'resource', resource: result as UIResource }] },
  } as MCPResponse;
}
// fallback: wrap as text
```

- 또한 `use-web-mcp.ts`의 `executeCall` 시그니처에 `Promise<MCPResponse | UIResource | string>` 등 명시적 타입 주석을 추가하면 좋습니다.

5. `src/lib/web-mcp/mcp-proxy.ts` 및 Tauri MCP 경로 — Message 매핑

- MCP 프록시가 MCPResponse를 Message로 변환할 때 `content`에서 `resource` 항목을 찾아 `message.uiResource`로 매핑합니다.
- `text/uri-list`의 경우 첫 유효 URL만 사용. base64 blob은 `uiResource.blob`으로 보존.

6. `src/features/chat/MessageBubbleRouter.tsx` — `UIResourceRenderer` 라우팅

- `MessageBubbleRouter` 최상단에 `message.uiResource` 검사 추가. React import 방식으로 `@mcp-ui/client`의 `UIResourceRenderer`를 사용합니다.

예시 변경:

```tsx
import { UIResourceRenderer } from '@mcp-ui/client';

if (message.uiResource) {
  const resource = Array.isArray(message.uiResource)
    ? message.uiResource[0]
    : message.uiResource;
  return (
    <UIResourceRenderer
      resource={resource}
      onUIAction={(action) => handleUIAction(action, message)}
    />
  );
}
```

- `handleUIAction`은 아래에서 설명할 onUIAction → MCP 호출 변환 로직과 연결합니다.

7. onUIAction 핸들링 (iframe → host)

- `@mcp-ui/client`의 `onUIAction` 이벤트를 받아 `useUnifiedMCP.executeToolCall`이나 ChatContext의 `submit`로 변환합니다.
- 예시 handler:

```ts
const handleUIAction = async (action, message) => {
  switch (action.type) {
    case 'tool':
      await executeToolCall({
        id: createId(),
        type: 'function',
        function: {
          name: action.payload.toolName,
          arguments: JSON.stringify(action.payload.params),
        },
      });
      break;
    case 'intent':
      // map to assistant intent handler
      break;
    case 'prompt':
      // create new message with content = action.payload.prompt
      break;
    case 'link':
      window.open(action.payload.url, '_blank');
      break;
    default:
      console.warn('Unknown UI action', action);
  }
};
```

- 메시지와 연동된 비동기 응답이 필요하면 `message.id`를 포함해 `executeToolCall` 혹은 MCP proxy에 콜백 등록.

8. 보안 권장사항

- iframe sandbox 설정: 가능한 한 최소 권한 사용(예: `sandbox=""` 또는 `sandbox="allow-scripts"`를 신중히 결정). `allow-same-origin`은 필요 시에만 사용.
- postMessage 처리: origin 검증, payload 스키마 검증

```ts
window.addEventListener('message', (ev) => {
  if (ev.origin !== expectedOrigin) return;
  // validate ev.data shape
});
```

- 외부 URL(embed) 실패(X-Frame-Options) 시 fallback: 링크로 열기. 사용자에게 경고 표시.
- base64 blob/대용량 컨텐츠는 서버/클라이언트에서 size limit를 적용.

9. 테스트 계획

- 단위 테스트
  - `ToolCaller`가 `mcpResponse`의 resource를 `Message.uiResource`로 변환하는지 확인
  - `use-unified-mcp`가 web tool의 `UIResource`를 적절히 래핑하는지 확인
- 통합 스모크
  - Mock web tool을 만들어 UIResource text/html 반환 → Chat에 iframe으로 렌더되는지 확인
  - iframe -> parent postMessage로 tool 호출이 트리거되어 `useUnifiedMCP`가 호출되는지 확인

10. PR/커밋 가이드

- 커밋 분리: (1) deps 설치, (2) 타입 변경, (3) ToolCaller + 훅 변경, (4) 라우팅 + UI 통합, (5) 테스트
- PR 본문: 변경 요약, 수동 스모크 테스트 절차, 보안/인수 조건 명시

작업 체크포인트(제가 패치로 적용 가능)

- 빠른 적용 권장 (제가 적용 시 순서):
  1. `pnpm add @mcp-ui/client` 및 빌드 확인
  2. `src/models/chat.ts` 타입 추가 + `src/lib/mcp-types.ts` 확장
  3. `ToolCaller.tsx` 수정하여 구조화된 `uiResource` 보존
  4. `MessageBubbleRouter.tsx`에서 `UIResourceRenderer` 사용
  5. 간단한 mocked tool로 E2E smoke 확인

원하시면 제가 위 1~4를 패치로 적용하고 빌드 및 간단한 스모크를 실행해 드리겠습니다. 어느 범위를 적용할까요? (추천: 1~4 전부)
