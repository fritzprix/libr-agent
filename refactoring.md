# Refactoring Plan — MCP-UI / UIResource Integration

목표: 이 저장소에 MCP-UI(웹에 전달되는 UI 리소스: HTML, 외부 URL, RemoteDOM)를 안전하게 수용하고 렌더링할 수 있게 구조와 흐름을 정비합니다.

이 문서는 fresh dev가 보고 바로 작업할 수 있도록 구체적인 파일 목록, 수정 위치, 타입 변화, 코드 스케치, 테스트 및 검증 절차를 모두 포함합니다.

요약 체크리스트

- Message 모델에 구조화된 UI 리소스 필드 추가 (`uiResource` 또는 `resources`).
- MCP 응답 수집/프록시에서 `resource` 타입을 감지해 Message에 보존.
- `ToolCaller`가 tool 실행 결과를 문자열로 직렬화하지 않고 구조화 필드로 저장하도록 변경.
- `use-unified-mcp`, `use-web-mcp`, `use-mcp-server` 훅에서 UIResource/MCPResponse를 손상시키지 않고 전달.
- UI 렌더러 컴포넌트(`UIResourceRenderer`) 추가(iframe 기반의 안전한 렌더러, `text/html`/`text/uri-list` 우선).
- `MessageBubbleRouter`에 UIResource 분기 추가.
- onUIAction 이벤트(iframe/postMessage)를 호스트로 전파할 수 있는 경로 마련.
- 보안 및 엣지케이스 문서화(iframe sandbox, X-Frame-Options fallback).

우선순위

1. 타입 및 MCP 타입 확장 (model + mcp-types).
2. MCP 프록시 / unified 훅에서 resource 보존 보장.
3. `ToolCaller` 변경(핵심 실행 흐름) — 구조화된 메시지 생성.
4. `UIResourceRenderer` 컴포넌트 작성 및 라우팅 통합.
5. 테스트/예제 및 보안 검토.

핵심 개념 (간단)

- UIResource: 서버가 반환하는 UI 리소스 객체(문서 상의 `UIResource` 스펙을 따름).
- Message 내 `uiResource` 필드: 렌더러가 안전히 액세스할 수 있는 구조체 필드.

파일별 변경 지침 (정확한 위치와 예시)

1. `src/models/chat.ts` — 타입 확장

- 변경 목적: Message가 구조화된 UI 리소스를 보관할 수 있어야 함.
- 변경 내용 (추가):

```ts
// 새 타입 정의 (예시)
export interface UIResource {
  uri?: string; // ui://... 형태 권장
  mimeType: string; // 'text/html' | 'text/uri-list' | 'application/vnd.mcp-ui.remote-dom'
  text?: string; // inline HTML or remote-dom script
  blob?: string; // base64-encoded content when used
}

export interface Message {
  // ...existing fields...
  content: string; // 요약/텍스트 표현은 유지
  uiResource?: UIResource | UIResource[]; // NEW: structured resource(s)
}
```

주의: `content`는 사용자/도구 친화적인 텍스트 요약을 담도록 유지하되, 실제 렌더링 가능한 UI는 `uiResource`에서 읽습니다.

2. `src/lib/mcp-types.ts` (또는 MCP 타입 정의가 있는 파일) — MCPResponse content union 확장

- 변경 목적: `MCPResponse.result.content` 항목이 `resource` 타입을 포함할 수 있도록 함.
- 제안 스니펫:

```ts
type MCPContentItem =
	| { type: 'text'; text: string }
	| { type: 'resource'; resource: UIResource }
	| { type: 'file'; ... };
```

3. `src/lib/web-mcp/mcp-proxy.ts` 및 Tauri/MCP 컨텍스트 — 프록시/컨텍스트에서 resource 보존

- 변경 목적: 백엔드(tauri/web/builtin)에서 도착한 MCP 응답을 파싱할 때 `resource` 항목을 감지하면 Message에 매핑.
- 구현 포인트:
  - `mcp-proxy`가 MCP 응답을 Message로 변환할 때 `result.content`의 `{ type:'resource' }` 항목을 찾아 `message.uiResource`로 매핑.
  - `text/uri-list`의 경우 첫번째 유효한 `http/https` URL을 선택(문서 규칙과 일치).
  - base64 `blob` 처리 시 `uiResource.blob`으로 보존.

4. `src/features/chat/ToolCaller.tsx` — 가장 중요한 변경

- 현재: `serializeToolResult`가 항상 문자열(JSON)으로 직렬화하고, 생성되는 `Message.content`에 문자열을 넣음.
- 문제: UIResource 구조가 문자열로 사라짐.
- 변경 목표: `serializeToolResult`가 문자열만 반환하지 않고 구조화된 오브젝트를 반환하도록 변경(아래 예시)

예시 변경(요약):

```ts
// 기존: returns string
const serialized = serializeToolResult(mcpResponse, toolName, start);

// 제안: serialized가 구조체를 반환
type SerializedToolResult = {
  success: boolean;
  text?: string;
  uiResource?: UIResource | UIResource[];
  metadata: Record<string, unknown>;
};

// 메시지 생성
const msg: Message = {
  id: createId(),
  sessionId: currentSession?.id || '',
  role: 'tool',
  content:
    serialized.text ||
    (serialized.uiResource
      ? `[UIResource: ${serialized.uiResource[0]?.uri || 'resource'}]`
      : ''),
  uiResource: serialized.uiResource,
  tool_call_id: toolCall.id,
};
```

핵심: `ToolCaller`는 `mcpResponse`가 resource 항목을 포함하면 이를 구조체로 보존해 `submit` 합니다(직렬 JSON string 대신 구조화된 필드로).

5. `src/hooks/use-unified-mcp.ts` — web-branch에서 반환 처리 강화

- 현재: web 툴 결과를 텍스트로 감싸 `MCPResponse`의 `result.content`로 넣음.
- 변경 목표: 만약 `executeWebTool`(WebMCP)의 반환값이 이미 `MCPResponse`거나 `UIResource`라면 그대로 사용하도록 처리.

요지:

- `executeToolCall`에서 `toolType === 'web'` 일 때
  - `result`가 이미 `MCPResponse`이면 그대로 반환
  - `result`가 `UIResource` 형태이면 `{ jsonrpc:'2.0', id: toolCall.id, result: { content: [ { type: 'resource', resource: result } ] } }` 로 래핑 후 반환
  - 그 외엔 기존 텍스트 포맷 유지

6. `src/hooks/use-web-mcp.ts` 및 `WebMCPProxy` 설계 권장

- 목적: `executeCall`/`callTool`이 가능한 한 `MCPResponse`나 `UIResource`를 반환하도록 일관성 보장.
- 권장: WebMCP 쪽에서 실제 tool 구현자가 UIResource를 반환하면 상위 훅이 이를 변경하지 않도록 문서화 및 타입 보강.

7. `src/features/chat/MessageBubbleRouter.tsx` — 렌더러 라우팅 추가

- 변경: 현재는 tool_calls / tool role / content 순으로 라우팅합니다. 여기에서 `message.uiResource`를 먼저 검사하고 `UIResourceRenderer`로 라우팅하세요.

예시:

```tsx
if (message.uiResource) {
	return <UIResourceRenderer resource={message.uiResource} onUIAction={...} />;
}
```

8. 새 컴포넌트: `src/components/ui/UIResourceRenderer.tsx`

- 역할: `UIResource` 객체를 받아 mimeType에 따라 안전하게 렌더링.
- 우선 지원: `text/html` (iframe srcDoc), `text/uri-list` (iframe src), `application/vnd.mcp-ui.remote-dom` (플래그로 remote-dom 지원 - 우선은 placeholder 또는 host 라이브러리 연계).
- props: `resource`, `onUIAction?: (action)=>void`, `supportedContentTypes?`, `iframeProps?`, `autoResizeIframe?`.
- 보안: iframe에 `sandbox` 속성 기본 적용(예: `sandbox="allow-scripts"`는 기본적으로 차단; 필요 시 최소 권한만 열기). 외부 URL 실패시 fallback으로 링크 제공.

간단한 구현 가이드라인 (iframe 우선)

- `text/html`: use `<iframe sandbox="allow-same-origin" srcDoc={resource.text}>` (검토: allow-same-origin 는 보안상 주의 필요)
- `text/uri-list`: parse first http/https URL and use `<iframe src={url} sandbox=... />`. X-Frame-Options 차단 시 새 탭 링크로 대체.
- `remote-dom`: 초기 구현은 `console.warn('remote-dom not yet supported')` 후 안전한 placeholder 렌더링; 이후 `@mcp-ui/client` 통합 권장.

9. onUIAction 이벤트 경로

- UI에서 발생한 action (예: tool call, intent, prompt, notify, link)은 iframe → parent postMessage로 전달됩니다. `UIResourceRenderer`에서 이를 리스닝하고 prop `onUIAction`을 호출하여 호스트 컴포넌트(예: ChatContext / AssistantContext / useUnifiedMCP)를 통해 실제 도구 호출로 전환합니다.

10. 보안 체크리스트

- iframe sandbox 정책 명시 및 최소 권한 원칙 적용
- 외부 URL은 X-Frame-Options 검사 및 fallback 처리
- remote-dom 스크립트는 호스트에서 검증/필터링 필요
- base64 blob의 최대 허용 크기 정책(과도한 메모리 사용 방지)

11. 테스트 및 스모크 시나리오

- 단위 테스트
  - `ToolCaller`가 `mcpResponse`에 resource가 포함될 때 `message.uiResource`를 채워 submit 하는지 확인
  - `use-unified-mcp`가 web tool의 UIResource를 올바르게 MCPResponse로 래핑/전달하는지 테스트

- 통합 스모크
  - 샘플 web tool(모의)을 만들어 UIResource `{ mimeType:'text/html', text:'<h1>Hello</h1>' }` 반환 → end-to-end로 Chat에 표시되는지 확인
  - 외부 URL 리소스가 X-Frame-Options로 차단될 때 링크로 열린다는 것을 확인

12. Acceptance criteria (완료 조건)

- Message 모델에 `uiResource` 필드 추가되어 타입체크 통과.
- Tool 호출(tauri/web/builtin) 결과로 UIResource가 Message로 전파되고, `MessageBubbleRouter`가 이를 감지해 `UIResourceRenderer`에서 iframe으로 렌더.
- UI에서 발생한 액션(onUIAction)은 호스트로 전달되고, 필요한 경우 통합된 MCP 호출로 변환 가능.

13. 커밋/PR 가이드

- 각 논리 단계를 별도 커밋으로 나눕니다: (1) 타입 확장, (2) 프록시/훅 변경, (3) ToolCaller 변경, (4) UI 렌더러 추가, (5) 테스트/문서.
- PR 설명에 변경 파일, 이유, 수동 테스트 절차(스모크 케이스)을 명시하세요.

14. 작업 우선순위 및 예상 소요

- 타입 + mcp-types 변경: 1-2시간
- 프록시/훅 수정: 2-4시간
- `ToolCaller` 리팩토링: 2-3시간
- `UIResourceRenderer`(첫 버전) + 라우팅: 2-4시간
- 테스트+문서: 1-2시간

부록: 주요 코드 스니펫 (요약)

- Message 타입 확장 (한 줄 요약)

```ts
uiResource?: UIResource | UIResource[];
```

- ToolCaller 변경 핵심 (메시지 생성 부분)

```ts
const serialized = serializeToolResult(mcpResponse, toolName, start);
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

끝으로: 제가 이 변경을 실제로 적용하여 패치를 만들고 빌드/스모크를 돌려드릴 수 있습니다. 어느 단계까지 자동으로 적용해 드릴까요? (추천: 우선 타입 + ToolCaller + MessageBubbleRouter + 간단한 `UIResourceRenderer`를 적용한 PR)
