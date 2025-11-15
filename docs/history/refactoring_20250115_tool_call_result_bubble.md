# Refactoring Plan: Tool Call + Result 통합 Bubble UI 개선

**작성일**: 2025-01-15  
**작업자**: AI Assistant  
**리뷰어**: fritzprix

---

## 1. 작업의 목적

Tool Call과 Tool Result가 현재 별도의 메시지 버블로 표시되어 채팅 화면의 공간을 과도하게 차지하는 문제를 해결하고, 접힌 상태에서도 성공/에러 상태를 명확히 구분할 수 있도록 UI/UX를 개선한다.

**핵심 목표**:

- Tool Call + Result를 하나의 통합 버블로 표시
- 접힌 상태(collapsed)에서도 성공/에러 상태를 색상, 배지, 아이콘으로 명확히 표시
- 공간 효율성 향상 (기존 2개 버블 → 1개 통합 버블)
- 에러 발생 시 자동으로 펼쳐져 상세 내용 즉시 확인 가능

---

## 2. 현재의 상태 / 문제점

### 2.1 현재 구조

```
[Assistant Message with tool_calls]
  ↓
[Tool Result Message 1 (role: tool)]
  ↓
[Tool Result Message 2 (role: tool)]
  ↓
...
```

**문제점**:

1. **공간 낭비**: Tool call 1개당 최소 2개 버블 (call + result) 필요
2. **관계 불명확**: Call과 Result의 연관성이 시각적으로 명확하지 않음
3. **에러 인지 어려움**: 접힌 상태에서 에러 발생 여부를 즉시 파악 불가
4. **스크롤 부담**: 여러 tool이 연속 호출되면 화면이 급격히 길어짐

### 2.2 현재 렌더링 흐름

**파일**: `src/features/chat/MessageBubbleRouter.tsx`

```typescript
// Assistant 메시지에 tool_calls가 있으면
if (hasToolCalls) {
  return <ToolCallBubble tool_calls={message.tool_calls!} />;
}

// Tool result 메시지는 별도로
if (message.role === 'tool') {
  return <ToolOutputBubble message={message} />;
}
```

**파일**: `src/features/chat/components/ChatMessages.tsx`

```typescript
// 메시지를 순차적으로 렌더링 (그룹핑 없음)
{messages.map((m) => (
  <MessageBubble
    key={m.id}
    message={m}
    currentAssistantName={getAssistantNameForMessage(m)}
  />
))}
```

### 2.3 메시지 생성 흐름

**파일**: `src/hooks/use-tool-processor.ts` (107-155줄)

```typescript
// Tool call 실행 후 별도의 tool result 메시지 생성
const toolResultMessage: Message = {
  id: createId(),
  role: 'tool',
  content: isMCPError(mcpResponse)
    ? buildErrorContent(`Error: ${mcpResponse.error.message}`)
    : mcpResponse.result?.content || [],
  tool_call_id: toolCall.id,
  sessionId: currentSession?.id || '',
  threadId: currentSession?.id || '',
};

// 별도 메시지로 제출
submitRef.current(messages, currentAssistant?.id);
```

---

## 3. 관련 코드의 구조 및 동작 방식 Summary (Birdeye View)

### 3.1 메시지 데이터 구조

**파일**: `src/models/chat.ts`

```typescript
export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: MCPContent[];
  tool_calls?: ToolCall[]; // Assistant 메시지에만
  tool_call_id?: string; // Tool result 메시지에만
  // ...
}

export interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string; // JSON stringified
  };
}
```

### 3.2 렌더링 계층 구조

```
ChatMessages (messages 배열 렌더링)
  ↓
MessageBubble (개별 메시지 래퍼)
  ↓
MessageBubbleRouter (역할별 라우팅)
  ↓
├─ ContentBubble (일반 텍스트)
├─ ToolCallBubble (tool_calls 표시)
└─ ToolOutputBubble (tool result 표시)
```

### 3.3 Tool 실행 흐름

```
1. AI가 tool_calls를 포함한 assistant 메시지 생성
   ↓
2. useToolProcessor가 감지하여 실행
   ↓
3. executeToolCall (useUnifiedMCP) 호출
   ↓
4. MCPResponse 수신
   ↓
5. tool result 메시지 생성 및 제출
   ↓
6. ChatMessages에서 별도 버블로 렌더링
```

---

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1 목표 구조

```
[Assistant Message]
  └─ [ToolCallResultBubble (통합)]
      ├─ 접힌 상태: Tool명 + 파라미터 개수 + 상태 배지 + 실행시간
      └─ 펼친 상태: Parameters + Result/Error Details
```

### 4.2 성공 판정 기준

**필수 요구사항**:

- [ ] Tool Call + Result가 하나의 버블로 통합됨
- [ ] 접힌 상태에서 성공/에러가 색상 + 배지로 구분됨
- [ ] 에러 발생 시 자동으로 펼쳐짐
- [ ] 공간 사용량 50% 이상 감소 (2 bubbles → 1 bubble)
- [ ] 기존 기능 유지 (copy, JSON view 등)

**선택 요구사항**:

- [ ] 실행 시간 표시
- [ ] Retry 버튼 (에러 시)
- [ ] 파라미터/결과 개별 접기/펼기

### 4.3 시각적 목표

**접힌 상태 (성공)**:

```
┌─────────────────────────────────────────────────┐
│ │ 🔧 read_file • 2 params → [✓ Success] (0.2s) │
└─────────────────────────────────────────────────┘
  ↑ 초록 테두리
```

**접힌 상태 (에러)**:

```
┌─────────────────────────────────────────────────┐
│ │ 🔧 delete_file • 1 param → [✕ Error] (0.1s)  │
└─────────────────────────────────────────────────┘
  ↑ 빨강 테두리 + 배경
```

**펼친 상태 (에러)**:

```
┌─────────────────────────────────────────────────┐
│ │ 🔧 delete_file • 1 param → [✕ Error] (0.1s)  │
│ ├───────────────────────────────────────────────│
│ │ Parameters                                    │
│ │ { "path": "/protected/file.txt" }             │
│ │                                               │
│ │ Error Details                                 │
│ │ ⚠ Permission denied: Cannot delete...        │
└─────────────────────────────────────────────────┘
```

---

## 5. 수정이 필요한 코드 및 수정 부분의 코드 스니핏

### 5.1 신규 컴포넌트: ToolCallResultBubble

**파일**: `src/features/chat/ToolCallResultBubble.tsx` (신규 생성)

```typescript
import React, { useState, useEffect } from 'react';
import { ToolCall, Message } from '@/models/chat';
import { Badge } from '@/components/ui/badge';
import {
  Wrench,
  CheckCircle,
  XCircle,
  Loader2,
  ChevronDown,
  AlertCircle,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import MessageRenderer from '@/components/MessageRenderer';

interface ToolCallResultBubbleProps {
  toolCall: ToolCall;
  toolResult?: Message;
  isLoading?: boolean;
}

export const ToolCallResultBubble: React.FC<ToolCallResultBubbleProps> = ({
  toolCall,
  toolResult,
  isLoading = false,
}) => {
  // 에러 감지
  const hasError = toolResult?.content?.some(c =>
    c.type === 'text' &&
    (c.text?.startsWith('❌') || c.text?.startsWith('Error:'))
  ) || false;

  // 실행 시간 추출 (metadata에서)
  const executionTime = (toolResult as any)?.metadata?.executionTime;

  // 에러 발생 시 자동 펼침
  const [isExpanded, setIsExpanded] = useState(hasError);

  useEffect(() => {
    if (hasError && !isExpanded) {
      setIsExpanded(true);
    }
  }, [hasError]);

  // Tool 이름 파싱 (서버명 제거)
  const toolName = toolCall.function.name.split('__').pop() || toolCall.function.name;
  const params = JSON.parse(toolCall.function.arguments);
  const paramCount = Object.keys(params).length;

  // 상태 배지
  const getStatusBadge = () => {
    if (isLoading) {
      return (
        <Badge variant="outline" className="gap-1 border-blue-500 text-blue-700 bg-blue-50">
          <Loader2 className="w-3 h-3 animate-spin" />
          Running
        </Badge>
      );
    }

    if (hasError) {
      return (
        <Badge variant="destructive" className="gap-1">
          <XCircle className="w-3 h-3" />
          Error
        </Badge>
      );
    }

    return (
      <Badge variant="outline" className="gap-1 border-green-500 text-green-700 bg-green-50">
        <CheckCircle className="w-3 h-3" />
        Success
      </Badge>
    );
  };

  // 컨테이너 스타일
  const containerClass = cn(
    "rounded-lg border transition-all mb-2",
    isLoading && "border-l-4 border-blue-500 bg-blue-50/30",
    !isLoading && hasError && "border-l-4 border-red-500 bg-red-50/50",
    !isLoading && !hasError && "border-l-4 border-green-500 bg-green-50/30",
  );

  return (
    <div className={containerClass}>
      {/* 헤더: 접힌 상태에서도 모든 정보 표시 */}
      <div
        className="flex items-center justify-between gap-2 p-3 cursor-pointer hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        {/* Left: Tool 정보 */}
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <Wrench className="w-4 h-4 flex-shrink-0 text-muted-foreground" />
          <span className="font-medium truncate">{toolName}</span>
          <span className="text-xs text-muted-foreground">
            • {paramCount} {paramCount === 1 ? 'param' : 'params'}
          </span>
        </div>

        {/* Right: 상태 + 시간 + 펼침 아이콘 */}
        <div className="flex items-center gap-2 flex-shrink-0">
          {getStatusBadge()}
          {executionTime && !isLoading && (
            <span className="text-xs text-muted-foreground">
              ({(executionTime / 1000).toFixed(1)}s)
            </span>
          )}
          <ChevronDown className={cn(
            "w-4 h-4 transition-transform text-muted-foreground",
            isExpanded && "rotate-180"
          )} />
        </div>
      </div>

      {/* 펼친 상태: 상세 정보 */}
      {isExpanded && (
        <div className="border-t px-3 pb-3 space-y-3">
          {/* Parameters */}
          <div className="pt-3">
            <div className="text-xs font-medium text-muted-foreground mb-2">
              Parameters
            </div>
            <div className="bg-muted/50 rounded p-2">
              <pre className="text-xs overflow-x-auto font-mono">
                {JSON.stringify(params, null, 2)}
              </pre>
            </div>
          </div>

          {/* Result or Error */}
          {toolResult && (
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-2">
                {hasError ? 'Error Details' : 'Result'}
              </div>
              {hasError ? (
                <div className="bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 rounded p-3">
                  <div className="flex items-start gap-2">
                    <AlertCircle className="w-4 h-4 text-red-600 dark:text-red-400 flex-shrink-0 mt-0.5" />
                    <div className="flex-1 min-w-0">
                      <MessageRenderer
                        content={toolResult.content}
                        className="text-sm text-red-900 dark:text-red-100"
                      />
                    </div>
                  </div>
                </div>
              ) : (
                <div className="bg-background rounded border p-2">
                  <MessageRenderer
                    content={toolResult.content}
                    className="text-sm"
                  />
                </div>
              )}
            </div>
          )}

          {/* Loading state */}
          {isLoading && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span>Executing tool...</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default ToolCallResultBubble;
```

### 5.2 MessageBubbleRouter 수정

**파일**: `src/features/chat/MessageBubbleRouter.tsx`

```typescript
// 기존
import ToolCallBubble from './ToolCallBubble';
import ToolOutputBubble from './ToolOutputBubble';

// 추가
import ToolCallResultBubble from './ToolCallResultBubble';

interface MessageBubbleRouterProps {
  message: Message;
  nextMessages?: Message[]; // 추가: 다음 메시지들 (tool results용)
}

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({
  message,
  nextMessages = [],
}) => {
  const hasToolCalls =
    message.tool_calls &&
    Array.isArray(message.tool_calls) &&
    message.tool_calls.length > 0 &&
    message.tool_calls.every((tc) => tc && tc.function && tc.function.name);

  const hasText = !!(message.content && message.content.length > 0);

  // Tool calls가 있으면 통합 버블 사용
  if (hasToolCalls) {
    return (
      <>
        {hasText && <ContentBubble message={message} />}
        {message.tool_calls!.map((toolCall) => {
          // 해당 tool call의 result 찾기
          const toolResult = nextMessages.find(
            m => m.role === 'tool' && m.tool_call_id === toolCall.id
          );

          return (
            <ToolCallResultBubble
              key={toolCall.id}
              toolCall={toolCall}
              toolResult={toolResult}
              isLoading={!toolResult}
            />
          );
        })}
      </>
    );
  }

  // Tool result 메시지는 이미 통합 버블에서 처리됨
  // (단독으로 렌더링되지 않음)
  if (message.role === 'tool') {
    return null;
  }

  return <ContentBubble message={message} />;
};
```

### 5.3 ChatMessages 수정 (메시지 그룹핑)

**파일**: `src/features/chat/components/ChatMessages.tsx`

```typescript
// 메시지 그룹핑 로직 추가
const groupedMessages = useMemo(() => {
  const result: Array<{
    message: Message;
    nextMessages: Message[];
  }> = [];

  let i = 0;
  while (i < messages.length) {
    const msg = messages[i];

    if (msg.role === 'assistant' && msg.tool_calls?.length) {
      // Tool calls를 가진 메시지: 다음 tool results 수집
      const toolCallIds = new Set(msg.tool_calls.map(tc => tc.id));
      const nextToolResults: Message[] = [];

      let j = i + 1;
      while (j < messages.length && nextToolResults.length < toolCallIds.size) {
        if (messages[j].role === 'tool' &&
            messages[j].tool_call_id &&
            toolCallIds.has(messages[j].tool_call_id!)) {
          nextToolResults.push(messages[j]);
        } else {
          break; // tool result가 아니면 중단
        }
        j++;
      }

      result.push({
        message: msg,
        nextMessages: nextToolResults,
      });

      i = j; // tool results를 건너뜀
    } else if (msg.role !== 'tool') {
      // 일반 메시지 (tool result는 이미 그룹에 포함됨)
      result.push({
        message: msg,
        nextMessages: [],
      });
      i++;
    } else {
      // 단독 tool result (매칭되는 call 없음) - 표시 안 함
      i++;
    }
  }

  return result;
}, [messages]);

// 렌더링
{groupedMessages.map(({ message, nextMessages }) => (
  <MessageBubble
    key={message.id}
    message={message}
    nextMessages={nextMessages}
    currentAssistantName={getAssistantNameForMessage(message)}
  />
))}
```

### 5.4 MessageBubble 수정

**파일**: `src/features/chat/MessageBubble.tsx`

```typescript
interface MessageBubbleProps {
  message: Message;
  currentAssistantName?: string;
  nextMessages?: Message[]; // 추가
}

const MessageBubble: React.FC<MessageBubbleProps> = ({
  message,
  currentAssistantName,
  nextMessages = [], // 추가
}) => {
  // ... 기존 코드 ...

  <MessageBubbleRouter
    message={message}
    nextMessages={nextMessages} // 추가
  />
```

### 5.5 실행 시간 추적 (useToolProcessor)

**파일**: `src/hooks/use-tool-processor.ts` (107-130줄 수정)

```typescript
// 실행 시간 측정 시작
const executionStartTime = Date.now();

const mcpResponse = await executeToolCallRef.current(toolCall);

// 실행 시간 계산
const executionTime = Date.now() - executionStartTime;

const toolResultMessage: Message = {
  id: createId(),
  assistantId: currentAssistant?.id,
  role: 'tool',
  content: isMCPError(mcpResponse)
    ? buildErrorContent(
        `Error: ${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`,
      )
    : mcpResponse.result?.content || [],
  tool_call_id: toolCall.id,
  sessionId: currentSession?.id || '',
  threadId: currentSession?.id || '',
  // 실행 시간 메타데이터 추가
  metadata: {
    executionTime,
  },
} as Message & { metadata?: { executionTime: number } };
```

### 5.6 Message 타입 확장 (선택)

**파일**: `src/models/chat.ts`

```typescript
export interface Message {
  // ... 기존 필드 ...

  // 메타데이터 (선택적)
  metadata?: {
    executionTime?: number; // Tool 실행 시간 (ms)
    retryCount?: number; // 재시도 횟수
    [key: string]: unknown; // 확장 가능
  };
}
```

---

## 6. 재사용 가능한 연관 코드

### 6.1 기존 컴포넌트 재사용

**BaseBubble** (`src/components/ui/BaseBubble.tsx`):

- 접기/펼치기 기능
- Copy 버튼
- Badge 표시
  → ToolCallResultBubble의 내부 섹션에 활용 가능

**MessageRenderer** (`src/components/MessageRenderer.tsx`):

- MCPContent 배열 렌더링
- UI Resource 처리
  → Result 표시에 그대로 사용

**JsonViewer** (`src/components/ui/JsonViewer.tsx`):

- JSON 데이터 시각화
  → Parameters 표시에 사용 가능 (선택적 개선)

### 6.2 유틸리티 함수

**파일**: `src/lib/utils.ts`

```typescript
// 에러 컨텐츠 감지
export function isErrorContent(content: MCPContent[]): boolean {
  return content.some(
    (c) =>
      c.type === 'text' &&
      (c.text?.startsWith('❌') || c.text?.startsWith('Error:')),
  );
}

// 에러 메시지 추출
export function extractErrorMessage(content: MCPContent[]): string {
  const errorContent = content.find(
    (c) =>
      c.type === 'text' &&
      (c.text?.startsWith('❌') || c.text?.startsWith('Error:')),
  );

  return errorContent?.type === 'text'
    ? errorContent.text?.replace(/^(❌|Error:)\s*/, '') || 'Unknown error'
    : 'Unknown error';
}
```

### 6.3 타입 가드

**파일**: `src/lib/mcp/utils/type-guards.ts`

```typescript
// 이미 존재하는 함수 재사용
export function isMCPError(response: MCPResponse<unknown>): boolean {
  return response.error !== undefined;
}
```

---

## 7. Test Code 추가 및 수정 필요 부분에 대한 가이드

### 7.1 단위 테스트

**파일**: `src/features/chat/__tests__/ToolCallResultBubble.test.tsx` (신규)

```typescript
import { render, screen, fireEvent } from '@testing-library/react';
import ToolCallResultBubble from '../ToolCallResultBubble';
import { ToolCall, Message } from '@/models/chat';

describe('ToolCallResultBubble', () => {
  const mockToolCall: ToolCall = {
    id: 'test-call-1',
    type: 'function',
    function: {
      name: 'workspace__read_file',
      arguments: JSON.stringify({ path: '/test/file.txt', lines: 10 }),
    },
  };

  const mockSuccessResult: Message = {
    id: 'test-result-1',
    role: 'tool',
    tool_call_id: 'test-call-1',
    content: [{ type: 'text', text: 'File content here...' }],
    sessionId: 'session-1',
    threadId: 'session-1',
    metadata: { executionTime: 250 },
  };

  const mockErrorResult: Message = {
    id: 'test-result-2',
    role: 'tool',
    tool_call_id: 'test-call-1',
    content: [{ type: 'text', text: '❌ File not found' }],
    sessionId: 'session-1',
    threadId: 'session-1',
    metadata: { executionTime: 100 },
  };

  test('접힌 상태에서 성공 배지 표시', () => {
    render(
      <ToolCallResultBubble
        toolCall={mockToolCall}
        toolResult={mockSuccessResult}
      />
    );

    expect(screen.getByText('Success')).toBeInTheDocument();
    expect(screen.getByText(/read_file/)).toBeInTheDocument();
    expect(screen.getByText(/2 params/)).toBeInTheDocument();
  });

  test('에러 발생 시 자동으로 펼쳐짐', () => {
    render(
      <ToolCallResultBubble
        toolCall={mockToolCall}
        toolResult={mockErrorResult}
      />
    );

    expect(screen.getByText('Error')).toBeInTheDocument();
    expect(screen.getByText(/File not found/)).toBeInTheDocument();
    expect(screen.getByText('Error Details')).toBeInTheDocument();
  });

  test('클릭 시 펼침/접힘 토글', () => {
    const { container } = render(
      <ToolCallResultBubble
        toolCall={mockToolCall}
        toolResult={mockSuccessResult}
      />
    );

    const header = container.querySelector('.cursor-pointer');
    expect(screen.queryByText('Parameters')).not.toBeInTheDocument();

    fireEvent.click(header!);
    expect(screen.getByText('Parameters')).toBeInTheDocument();

    fireEvent.click(header!);
    expect(screen.queryByText('Parameters')).not.toBeInTheDocument();
  });

  test('로딩 상태 표시', () => {
    render(
      <ToolCallResultBubble
        toolCall={mockToolCall}
        isLoading={true}
      />
    );

    expect(screen.getByText('Running')).toBeInTheDocument();
    expect(screen.getByText(/Executing tool/)).toBeInTheDocument();
  });

  test('실행 시간 표시', () => {
    render(
      <ToolCallResultBubble
        toolCall={mockToolCall}
        toolResult={mockSuccessResult}
      />
    );

    expect(screen.getByText('(0.3s)')).toBeInTheDocument();
  });
});
```

### 7.2 통합 테스트

**파일**: `src/features/chat/__tests__/ChatMessages.integration.test.tsx`

```typescript
describe('ChatMessages with ToolCallResultBubble', () => {
  test('tool call과 result가 하나의 버블로 표시됨', async () => {
    const messages: Message[] = [
      {
        id: 'msg-1',
        role: 'assistant',
        content: [],
        tool_calls: [mockToolCall],
        sessionId: 'session-1',
        threadId: 'session-1',
      },
      {
        id: 'msg-2',
        role: 'tool',
        tool_call_id: 'test-call-1',
        content: [{ type: 'text', text: 'Success' }],
        sessionId: 'session-1',
        threadId: 'session-1',
      },
    ];

    render(<ChatMessages messages={messages} />);

    // 하나의 통합 버블만 표시되어야 함
    const toolBubbles = screen.getAllByText(/read_file/);
    expect(toolBubbles).toHaveLength(1);
  });

  test('여러 tool call이 각각 통합 버블로 표시됨', () => {
    // 테스트 구현
  });
});
```

### 7.3 시각적 회귀 테스트 (Storybook)

**파일**: `src/features/chat/ToolCallResultBubble.stories.tsx` (신규)

```typescript
import type { Meta, StoryObj } from '@storybook/react';
import ToolCallResultBubble from './ToolCallResultBubble';

const meta: Meta<typeof ToolCallResultBubble> = {
  title: 'Chat/ToolCallResultBubble',
  component: ToolCallResultBubble,
  parameters: {
    layout: 'padded',
  },
};

export default meta;
type Story = StoryObj<typeof ToolCallResultBubble>;

export const Success: Story = {
  args: {
    toolCall: {
      id: 'call-1',
      type: 'function',
      function: {
        name: 'workspace__read_file',
        arguments: JSON.stringify({ path: '/test.txt' }),
      },
    },
    toolResult: {
      id: 'result-1',
      role: 'tool',
      tool_call_id: 'call-1',
      content: [{ type: 'text', text: 'File content...' }],
      sessionId: 's1',
      threadId: 's1',
      metadata: { executionTime: 250 },
    },
  },
};

export const Error: Story = {
  args: {
    toolCall: {
      id: 'call-2',
      type: 'function',
      function: {
        name: 'workspace__delete_file',
        arguments: JSON.stringify({ path: '/protected.txt' }),
      },
    },
    toolResult: {
      id: 'result-2',
      role: 'tool',
      tool_call_id: 'call-2',
      content: [{ type: 'text', text: '❌ Permission denied' }],
      sessionId: 's1',
      threadId: 's1',
      metadata: { executionTime: 100 },
    },
  },
};

export const Loading: Story = {
  args: {
    toolCall: {
      id: 'call-3',
      type: 'function',
      function: {
        name: 'builtin_content_store__search',
        arguments: JSON.stringify({ query: 'test' }),
      },
    },
    isLoading: true,
  },
};
```

---

## 8. 추가 분석 과제

### 8.1 성능 최적화 검토

**분석 필요 사항**:

- 메시지 그룹핑 로직의 성능 영향 측정 (대량 메시지 시나리오)
- React.memo 적용 필요성 검토
- useMemo 의존성 배열 최적화

**측정 방법**:

```typescript
// ChatMessages.tsx에 성능 측정 추가
const groupingStartTime = performance.now();
const groupedMessages = useMemo(() => {
  // ... 그룹핑 로직
}, [messages]);
const groupingTime = performance.now() - groupingStartTime;

if (groupingTime > 16) {
  // 1 frame (60fps)
  logger.warn('Message grouping took too long', {
    groupingTime,
    messageCount: messages.length,
  });
}
```

### 8.2 접근성 (a11y) 검토

**검토 항목**:

- [ ] 키보드 네비게이션 (Space/Enter로 펼침/접힘)
- [ ] 스크린 리더 지원 (aria-expanded, aria-label)
- [ ] Focus 관리 (펼쳤을 때 내부 요소로 포커스 이동)
- [ ] 색상 대비 (WCAG AA 준수)

**구현 예시**:

```typescript
<div
  role="button"
  tabIndex={0}
  aria-expanded={isExpanded}
  aria-label={`${toolName} tool call ${hasError ? 'failed' : 'succeeded'}`}
  onClick={() => setIsExpanded(!isExpanded)}
  onKeyDown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      setIsExpanded(!isExpanded);
    }
  }}
>
```

### 8.3 에러 복구 전략

**분석 필요 사항**:

- Tool call과 result의 ID 매칭 실패 케이스 처리
- 누락된 tool result 표시 방법 (timeout 등)
- Retry 기능 구현 시 상태 관리 전략

**고려 사항**:

```typescript
// 매칭되지 않은 tool call 처리
if (toolCall && !toolResult && !isLoading) {
  // 5초 이상 대기 시 timeout 표시?
  // 수동 재시도 버튼 제공?
}
```

---

## 9. Clarification Q-list

### Q1. 기존 ToolCallBubble, ToolOutputBubble 제거 여부

**질문**: 새 컴포넌트 도입 후 기존 컴포넌트를 즉시 제거할지, 일정 기간 공존시킬지?

**옵션**:

- A: 즉시 제거 (코드 단순화, 단 리스크 높음)
- B: Feature flag로 전환 가능하게 구현 (안전, 단 복잡도 증가)
- C: 기존 코드 유지하되 deprecated 마크 (점진적 전환)

**권장**: Option B (초기 배포 시 안전장치)

### Q2. 여러 tool call의 표시 순서

**질문**: Assistant가 여러 tool을 동시에 호출할 때, 각 tool의 result가 비동기로 도착하는 경우 표시 순서는?

**옵션**:

- A: tool_calls 배열 순서대로 표시 (result 대기)
- B: result 도착 순서대로 표시 (빠른 피드백)
- C: 모든 result 도착 후 일괄 표시 (일관성)

**권장**: Option A (예측 가능성)

### Q3. UI Resource가 포함된 tool result 처리

**질문**: Tool result에 UI Resource (remote-dom 등)가 포함된 경우, 통합 버블 내에서 어떻게 표시할지?

**현재 동작**:

- ToolOutputBubble에서 separateContent로 UI/Text 분리 후 렌더링
- UI Resource는 항상 펼친 상태로 표시

**옵션**:

- A: UI Resource도 접기 가능하게 변경
- B: UI Resource는 항상 펼친 상태 유지 (현재와 동일)
- C: UI Resource가 있으면 자동으로 버블 펼침

**권장**: Option C (UI Resource는 중요한 정보이므로)

### Q4. 실행 시간 표시 단위 및 형식

**질문**: 실행 시간을 어떤 단위와 형식으로 표시할지?

**옵션**:

- A: 밀리초 단위 (123ms)
- B: 초 단위 소수점 1자리 (0.1s)
- C: 상황에 따라 자동 선택 (< 1s: ms, >= 1s: s)

**권장**: Option C (가독성과 정확성 균형)

### Q5. 에러 발생 시 재시도 기능

**질문**: 에러 발생 시 같은 파라미터로 재시도하는 버튼을 제공할지?

**고려사항**:

- 재시도 시 새로운 메시지 생성 vs 기존 메시지 업데이트?
- 재시도 횟수 제한?
- 재시도 중 로딩 상태 표시?

**권장**: Phase 2 기능으로 미루고, 초기에는 구현 제외

### Q6. 다크모드 색상 테마

**질문**: 현재 제안된 색상 (green-50, red-50 등)이 다크모드에서도 적절한지?

**확인 필요**:

- Tailwind의 dark: variant 활용 여부
- 다크모드 전용 색상 팔레트 정의 필요 여부

**권장**: Tailwind의 semantic color 사용 (border-green-500/dark:border-green-400)

---

## 10. 작업 일정 및 마일스톤

### Phase 1: 기본 구현 (예상 3-4일)

- [ ] ToolCallResultBubble 컴포넌트 생성
- [ ] MessageBubbleRouter 수정
- [ ] ChatMessages 메시지 그룹핑 로직
- [ ] 기본 단위 테스트

### Phase 2: 고급 기능 (예상 2-3일)

- [ ] 실행 시간 추적 및 표시
- [ ] 접근성 개선 (a11y)
- [ ] Storybook stories 작성
- [ ] 통합 테스트

### Phase 3: 최적화 및 정리 (예상 1-2일)

- [ ] 성능 최적화
- [ ] 기존 컴포넌트 제거 (또는 deprecated)
- [ ] 문서 업데이트
- [ ] E2E 테스트

---

## 11. 참고 자료

- 현재 UI 구조: `src/features/chat/README.md`
- MCP Response 타입: `src/lib/mcp/protocol/response.ts`
- 메시지 모델: `src/models/chat.ts`
- 기존 BaseBubble 구현: `src/components/ui/BaseBubble.tsx`
- Tool Processor 로직: `src/hooks/use-tool-processor.ts`

---

**작성자 노트**:
이 리팩토링은 사용자 경험을 크게 개선할 수 있는 작업입니다. 단계별로 진행하며 각 단계마다 사용자 피드백을 받는 것을 권장합니다. 특히 에러 표시 방식은 사용자 선호도에 따라 조정이 필요할 수 있습니다.
