# UI Builtin Tools 구현 계획

**작성일**: 2025-01-09  
**목표**: 사용자와의 상호작용을 위한 Built-in MCP UI 도구 구현

## 작업 목적

AI 에이전트가 사용자와 직접 상호작용할 수 있는 UI 도구 세트를 제공하여, 선택지 제시, 텍스트 입력 요청, 데이터 시각화 등의 기능을 지원한다.

**핵심 요구사항**:

- `visualizeData`: 기본 시각화 (bar/chart)
- `promptUser`: 사용자 프롬프트 (선택지/텍스트 입력)
- `reply`: 사용자 응답 처리

## 현재 상태 / 문제점

### 현재 구조 (Birdeye View)

**WebMCP 아키텍처**:

```
[Chat UI / Tool Renderer]
    ↓ uses
[WebMCPContext] → provides getServerProxy()
    ↓ wraps
[WebMCPProxy] → communicates via postMessage
    ↓ sends to
[Web Worker (mcp-worker.ts)]
    ↓ loads
[MCP Server Modules] (planning, playbook, ...)
    ↓ returns
[MCPResponse with UIResource]
    ↓ rendered by
[UIResourceRenderer] → fires onUIAction
    ↓ routes back to
[WebMCPProxy.callTool()]
```

**기존 구현 패턴 (playbook-store 참고)**:

- `createUIResource()`: UI 리소스 생성 (`rawHtml` 또는 `remoteDom`)
- `createMCPStructuredMultipartResponse()`: 텍스트 + UI 리소스 응답
- UIResource에 `serviceInfo.serverName` 첨부하여 클라이언트 라우팅 지원
- iframe 내 스크립트: `window.parent.postMessage({ type: 'tool', payload: { toolName, params } }, '*')`

### 문제점

1. **UI 상호작용 도구 부재**: 사용자 입력/선택을 받는 built-in 도구가 없음
2. **UIAction 라우팅 미구현**: `UIResourceRenderer`의 `onUIAction` 핸들러가 전역적으로 연결되지 않음
3. **응답 비동기 처리**: UI에서 발생한 액션을 원래 툴 호출로 다시 연결하는 메커니즘 필요

## 변경 이후 상태 / 해결 판정 기준

### 성공 기준

1. **새 MCP 서버 모듈 (`ui-tools`) 추가 완료**:
   - `prompt_user`: 사용자 프롬프트 UI 생성 (텍스트/선택지)
   - `reply_prompt`: 사용자 응답 수신
   - `visualize_data`: 간단 데이터 시각화

2. **Worker 등록 완료**:
   - `mcp-worker.ts`에 `ui-tools` 모듈 등록

3. **통합 검증**:
   - Chat UI에서 `prompt_user` 호출 시 UI 렌더링
   - 사용자 액션 시 `reply_prompt` 자동 호출
   - 응답이 원래 호출 컨텍스트로 반환

### ✅ 이미 구현된 사항 (변경 불필요)

- **UIAction 라우팅**: `MessageRenderer.tsx:95-335`에 이미 완전히 구현됨
  - `UIResourceRenderer`의 `onUIAction` 핸들러 연결됨
  - `serviceInfo.serverName` 기반 자동 라우팅 동작 중
  - Tauri 명령어, MCP 도구, Intent, Prompt 등 모든 액션 타입 처리 완료

### 판정 방법

- `pnpm build` 성공 (타입 오류 없음)
- `pnpm refactor:validate` 통과
- Smoke test: Chat에서 `ui.prompt_user()` 호출 → UI 렌더 → 사용자 클릭 → `reply_prompt` 실행 확인

## 구현 계획

### 1. 새 MCP 서버 모듈 추가

**파일**: `src/lib/web-mcp/modules/ui-tools.ts`

**구현 내용**:

```typescript
/**
 * UI 상호작용을 위한 Built-in MCP 서버
 * - prompt_user: 사용자 프롬프트 생성
 * - reply_prompt: 사용자 응답 수신
 * - visualize_data: 데이터 시각화
 */

import {
  createMCPStructuredResponse,
  createMCPTextResponse,
  createMCPStructuredMultipartResponse,
} from '@/lib/mcp-response-utils';
import type {
  MCPResponse,
  MCPTool,
  WebMCPServer,
  MCPContent,
  ServiceInfo,
} from '@/lib/mcp-types';
import { createUIResource, type UIResource } from '@mcp-ui/server';

let nextMessageId = 1;

// prompt_user 툴 구현
// - 입력: { prompt: string, type: 'text'|'select'|'multiselect', options?: string[] }
// - 출력: multipart(text + UIResource) + structured { messageId }
// - UIResource: rawHtml with inline form + postMessage script

// reply_prompt 툴 구현
// - 입력: { messageId: string, answer: unknown }
// - 출력: structured { messageId, answer, timestamp }

// visualize_data 툴 구현
// - 입력: { type: 'bar'|'line', data: Array<{label:string, value:number}> }
// - 출력: multipart(text + UIResource with SVG/Chart)

const tools: MCPTool[] = [
  {
    name: 'prompt_user',
    description:
      'Display interactive prompt to user (text input, select, multiselect)',
    inputSchema: {
      type: 'object',
      properties: {
        prompt: { type: 'string', description: 'Question to ask user' },
        type: {
          type: 'string',
          enum: ['text', 'select', 'multiselect'],
          description: 'Type of prompt',
        },
        options: {
          type: 'array',
          items: { type: 'string' },
          description: 'Options for select/multiselect',
        },
      },
      required: ['prompt', 'type'],
    },
  },
  {
    name: 'reply_prompt',
    description: 'Receive user response from prompt UI (called by UI action)',
    inputSchema: {
      type: 'object',
      properties: {
        messageId: { type: 'string' },
        answer: { description: 'User answer' },
      },
      required: ['messageId', 'answer'],
    },
  },
  {
    name: 'visualize_data',
    description: 'Visualize data as bar or line chart',
    inputSchema: {
      type: 'object',
      properties: {
        type: { type: 'string', enum: ['bar', 'line'] },
        data: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              label: { type: 'string' },
              value: { type: 'number' },
            },
            required: ['label', 'value'],
          },
        },
      },
      required: ['type', 'data'],
    },
  },
];

const uiTools: WebMCPServer = {
  name: 'ui',
  version: '0.1.0',
  description: 'Built-in UI interaction tools',
  tools,

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // Tool 구현 로직
  },
};

export default uiTools;
```

**핵심 구현 사항**:

- `prompt_user`의 HTML 생성 시 `window.parent.postMessage({ type: 'tool', payload: { toolName: 'reply_prompt', params: { messageId, answer } } }, '*')` 포함
- `createUIResource()` 후 `serviceInfo = { serverName: 'ui', toolName: '', backendType: 'BuiltInWeb' }` 첨부
- messageId 생성: `ui-${Date.now()}-${nextMessageId++}`

### 2. Worker 모듈 등록

**파일**: `src/lib/web-mcp/mcp-worker.ts`

**수정 위치**:

```typescript
// Static imports section
import planningServer from './modules/planning-server';
import playbookStore from './modules/playbook-store';
import uiTools from './modules/ui-tools'; // ← 추가

// MODULE_REGISTRY
const MODULE_REGISTRY = [
  { key: 'planning', module: planningServer },
  { key: 'playbook', module: playbookStore },
  { key: 'ui', module: uiTools }, // ← 추가
] as const;
```

### 3. ~~UIAction 라우팅 구현~~ ✅ 이미 완료

**상태**: `MessageRenderer.tsx:95-335`에 이미 구현되어 있음

**구현 내용** (참고용):

- `handleUIAction` 콜백이 `UIResourceRenderer`에 연결됨 (line 392-399)
- Service context 기반 자동 라우팅 구현됨 (line 197-215)
- 모든 UIAction 타입 처리 완료 (tool, intent, prompt, link, notify)
- **변경 불필요**

### 4. ~~Chat UI 연동~~ ✅ 이미 완료

**상태**: `MessageRenderer.tsx`에 이미 완전히 구현되어 있음

**구현 내용** (참고용):

```tsx
// MessageRenderer.tsx:392-399
<UIResourceRenderer
  key={key}
  remoteDomProps={remoteDomProps}
  onUIAction={handleUIAction} // ← 이미 연결됨
  supportedContentTypes={[...supportedContentTypes]}
  htmlProps={{ autoResizeIframe: { height: true } }}
  resource={item.resource}
/>
```

- **변경 불필요**

## 재사용 가능한 연관 코드

### playbook-store.ts 참고 패턴

**UIResource 생성 + serviceInfo 첨부**:

```typescript
function createUiResourceFromHtml(html: string) {
  const res = createUIResource({
    uri: `ui://playbooks/list/${Date.now()}`,
    content: { type: 'rawHtml', htmlString: html },
    encoding: 'text',
  }) as UIResource & { serviceInfo?: ServiceInfo };

  res.serviceInfo = {
    serverName: 'playbook',
    toolName: '',
    backendType: 'BuiltInWeb',
  };

  return res;
}
```

**Multipart 응답 생성**:

```typescript
function makeListMultipartResponse(
  items: PlaybookRecord[],
  formattedText: string,
  structured: unknown,
) {
  const textPart: MCPContent = {
    type: 'text',
    text: formattedText,
  } as unknown as MCPContent;

  const uiResource = createUiResourceFromHtml(buildListItemsHtml(items));

  return createMCPStructuredMultipartResponse(
    [textPart, uiResource],
    structured,
  );
}
```

**iframe 내 postMessage 스크립트**:

```javascript
document.addEventListener('click', function (e) {
  const btn = e.target;
  if (btn && btn.classList.contains('select-pb-btn')) {
    const id = btn.getAttribute('data-pbid');
    window.parent.postMessage(
      {
        type: 'tool',
        payload: { toolName: 'select_playbook', params: { id } },
      },
      '*',
    );
  }
});
```

### mcp-response-utils 위치

**파일**: `src/lib/web-mcp/mcp-response-utils.ts` (또는 유사 유틸리티)

필요 함수:

- `createUIResource()`
- `createMCPStructuredMultipartResponse()`
- `createMCPTextResponse()`

※ 실제 파일 위치는 grep 검색 필요. playbook-store에서 import하는 경로 확인.

## 구현 순서 (수정됨)

1. **Phase 1: 서버 모듈 기본 구현**
   - `ui-tools.ts` 생성 (tools 정의 + 기본 스켈레톤)
   - `mcp-worker.ts` 등록
   - 빌드/타입 체크

2. **Phase 2: 툴 로직 구현**
   - `prompt_user`: HTML 생성 로직 (text/select/multiselect)
   - `reply_prompt`: 응답 수신 및 structured 반환
   - `visualize_data`: 간단 SVG 바 차트 생성

3. ~~**Phase 3: 라우팅 연결**~~ ✅ **이미 완료됨 - 생략**
   - `MessageRenderer.tsx`에 이미 모든 라우팅 구현 완료

4. **Phase 3 (최종): 통합 검증**
   - `pnpm refactor:validate` 실행
   - Smoke test (수동 테스트)

## 추가 분석 과제

1. **UIResourceRenderer 사용 위치 식별**:
   - Chat UI에서 tool result를 렌더링하는 정확한 컴포넌트 위치
   - 이미 `UIResourceRenderer`를 사용 중인지, 아니면 추가 필요한지 확인
   - 검색 키워드: `UIResourceRenderer`, `isUIResource`, `tool result render`

2. **mcp-response-utils 실제 위치**:
   - `createUIResource`, `createMCPStructuredMultipartResponse` 함수가 정의된 파일
   - playbook-store의 import 문 확인
   - 없을 경우 해당 유틸리티 함수 구현 필요

3. **MessageId 관리 전략**:
   - 여러 동시 프롬프트 처리 시 messageId 충돌 방지
   - 타임아웃/만료된 프롬프트 처리 (선택적)

4. **보안 고려사항**:
   - rawHtml 사용 시 XSS 방지 (HTML escape)
   - postMessage origin 검증 (UIResourceRenderer 내부 처리 확인)

## 변경 파일 목록 (수정됨)

### 추가

- `src/lib/web-mcp/modules/ui-tools.ts` (새 MCP 서버 모듈)

### 수정

- `src/lib/web-mcp/mcp-worker.ts` (모듈 등록)

### ~~변경 불필요~~ (이미 구현됨)

- ~~`src/context/WebMCPContext.tsx`~~ - 변경 불필요
- ~~`src/components/MessageRenderer.tsx`~~ - 이미 완전히 구현됨 (line 95-399)
