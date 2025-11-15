# Refactoring Plan: Tool Call Grouping with Collapsible UI

**Date:** 2025-11-15  
**Author:** AI Assistant  
**Status:** Planning

## 작업의 목적

연속된 여러 개의 Tool Call & Result가 개별 bubble로 표시되어 화면 공간을 과도하게 차지하는 문제를 해결하고, 사용자 경험을 개선하기 위해 연속된 tool call들을 하나의 그룹 bubble로 통합하여 표시합니다.

**핵심 목표:**

- 연속된 tool-only call/result를 하나의 collapsible group으로 묶기
- 기본적으로 최신 4개만 표시하고 gradient overlay로 이전 항목 숨김
- text content나 UI resource가 포함된 경우 별도 그룹으로 분리
- 화면 공간 효율성 향상 및 가독성 개선

## 현재의 상태 / 문제점

### 현재 동작 방식

**1. ChatMessages.tsx - 그루핑 로직**

```typescript
// 현재: 1:N 매칭 (assistant message → tool results)
const groupedMessages = useMemo(() => {
  // Assistant message with tool_calls와 그에 대응하는 tool result messages를 매칭
  // tool_call_id로 1:N 관계 설정
  // nextMessages 배열로 전달
}, [messages]);
```

**2. MessageBubbleRouter.tsx - 렌더링 로직**

```typescript
// 현재: 각 tool_call마다 독립적인 ToolCallResultBubble 생성
if (hasToolCalls) {
  return (
    <>
      {hasText && <ContentBubble message={message} />}
      {message.tool_calls!.map((toolCall) => (
        <ToolCallResultBubble
          key={toolCall.id}
          toolCall={toolCall}
          toolResult={toolResult}
          isLoading={!toolResult}
        />
      ))}
    </>
  );
}
```

**3. ToolCallResultBubble.tsx**

- 단일 tool call + result만 처리
- 개별 collapse/expand 상태 관리
- Error/UI Resource 자동 확장

### 문제점

1. **화면 공간 낭비**: 연속된 10개 tool call이 10개의 독립 bubble로 표시됨
2. **스크롤 부담**: 많은 tool call 결과로 인해 중요한 대화 내용이 화면 밖으로 밀림
3. **시각적 혼란**: 각 bubble이 동일한 크기와 중요도로 표시되어 주요 내용 파악 어려움
4. **일관성 부족**: text content와 tool call이 섞여 있을 때 시각적 구분 불명확

### 예시 시나리오

```
[현재 상태]
Assistant: "파일을 분석하겠습니다"
┌─ readFile (config.json) ─┐
└───────────────────────────┘
┌─ readFile (package.json) ─┐
└────────────────────────────┘
┌─ searchFiles (*.ts) ──────┐
└────────────────────────────┘
┌─ analyzeCode (main.ts) ───┐
└────────────────────────────┘
... (6개 더)

[개선 후 - 목표]
Assistant: "파일을 분석하겠습니다"
┌──────────────────────────────────┐
│ 🔧 Tool Executions (10 calls)   │
│ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ [gradient: older 6 items hidden] │
│ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ ✓ searchFiles (45ms)             │
│ ✓ analyzeCode (234ms)            │
│ ✓ readFile (config.json, 12ms)  │
│ ✓ readFile (package.json, 8ms)  │
│         [Show All (10) ▼]        │
└──────────────────────────────────┘
```

## 관련 코드의 구조 및 동작 방식 Summary

### Bird's Eye View - 메시지 흐름

```
┌─────────────────────────────────────────────────┐
│ ChatContext                                     │
│ - messages: Message[]                           │
│ - submit() → AI Service → tool_calls            │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ useToolProcessor                                │
│ - executeToolCall() → MCPResponse               │
│ - creates tool result messages                 │
│ - metadata.executionTime 추가                   │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ ChatMessages.tsx                                │
│ - groupedMessages: useMemo()                    │
│   • tool_call_id로 매칭                         │
│   • nextMessages[] 생성                         │
│ - MessageBubble 렌더링                          │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ MessageBubble → MessageBubbleRouter             │
│ - hasToolCalls 검사                             │
│ - ContentBubble (text content)                  │
│ - ToolCallResultBubble[] (각 tool call)         │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ ToolCallResultBubble                            │
│ - Single call + result                          │
│ - Collapse/Expand 상태                          │
│ - Error/UI Resource 자동 확장                   │
└─────────────────────────────────────────────────┘
```

### 핵심 데이터 구조

**Message Interface (src/models/chat.ts)**

```typescript
interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: MCPContent[];
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  metadata?: {
    executionTime?: number;
    [key: string]: unknown;
  };
}
```

**ToolCall Interface**

```typescript
interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}
```

**MCPContent (src/lib/mcp-types.ts)**

```typescript
type MCPContent =
  | { type: 'text'; text: string }
  | {
      type: 'resource';
      resource: { uri: string; mimeType: string; text?: string };
    }
  | { type: 'image'; data: string; mimeType: string };
```

### 그루핑 판단 기준

**현재 구현:**

- Assistant message의 `tool_calls` 배열과 `tool_call_id`로 1:N 매칭
- text content 유무는 `ContentBubble` 렌더링에만 사용

**새로운 그루핑 필요:**

- **그룹으로 묶일 조건**: 연속된 tool-only calls (text ❌, UI resource ❌)
- **그룹 분리 조건**:
  - Assistant message에 text content 있음
  - Tool result에 UI resource 있음 (`content[].type === 'resource'`)
  - 다른 role 메시지 끼어듦

## 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

1. **기능적 요구사항**
   - ✅ 연속된 tool-only calls가 하나의 ToolCallGroupBubble로 표시됨
   - ✅ 기본 상태에서 최신 4개 tool call/result만 보임
   - ✅ Gradient overlay로 이전 항목들이 숨겨진 것을 시각적으로 표시
   - ✅ "Show All" 클릭 시 전체 tool call/result 확장
   - ✅ Text content나 UI resource 포함 시 별도 그룹/bubble로 분리

2. **UI/UX 요구사항**
   - ✅ Group header에 총 tool call 개수 표시
   - ✅ Group 내 성공/실패 상태 요약 표시
   - ✅ 개별 tool call은 compact format으로 표시 (이름, 실행시간, 상태)
   - ✅ Error 발생 시 해당 tool call 자동 확장
   - ✅ 기존 ToolCallResultBubble과 시각적 일관성 유지

3. **성능 요구사항**
   - ✅ 그루핑 로직이 messages 변경 시에만 재계산 (useMemo 활용)
   - ✅ 100개 이상 tool call에서도 스크롤 성능 저하 없음

4. **호환성 요구사항**
   - ✅ 기존 단일 tool call은 기존 ToolCallResultBubble 사용
   - ✅ UI resource 포함 tool call은 독립 bubble로 표시
   - ✅ 데이터베이스 스키마 변경 없음

### 테스트 시나리오

**Scenario 1: 연속된 5개 tool-only calls**

```
Given: Assistant message with 5 tool_calls, no text content, no UI resources
When: ChatMessages renders
Then: 하나의 ToolCallGroupBubble 표시, 최신 4개 visible, gradient overlay
```

**Scenario 2: Text content 중간에 끼어듦**

```
Given:
  - Assistant message 1: 3 tool_calls (no text)
  - Assistant message 2: text content "분석 결과입니다"
  - Assistant message 3: 2 tool_calls (no text)
When: ChatMessages renders
Then:
  - Group 1: 3 tool calls
  - ContentBubble: "분석 결과입니다"
  - Group 2: 2 tool calls
```

**Scenario 3: UI Resource 포함**

```
Given: 5 tool_calls, 3번째에 UI resource 포함
When: ChatMessages renders
Then:
  - Group 1: tool_call 1, 2
  - Standalone: tool_call 3 (UI resource)
  - Group 2: tool_call 4, 5
```

**Scenario 4: Error 자동 확장**

```
Given: ToolCallGroupBubble with 10 calls, 8번째 error
When: Component mounts
Then:
  - Group collapsed (최신 4개 visible)
  - 8번째 tool call이 visible 범위에 없음
  - "Show All" 클릭 시 8번째 자동 확장 및 에러 표시
```

## 수정이 필요한 코드 및 수정부분

### 1. ChatMessages.tsx - 그루핑 로직 확장

**파일:** `src/features/chat/components/ChatMessages.tsx`

**현재 구조:**

```typescript
interface GroupedMessage {
  message: Message;
  nextMessages: Message[];
}
```

**변경 후 구조:**

```typescript
interface GroupedMessage {
  type: 'single' | 'tool_group';
  message: Message;
  nextMessages: Message[];
  toolGroup?: {
    calls: Array<{
      toolCall: ToolCall;
      toolResult?: Message;
    }>;
    hasText: boolean;
    hasUIResource: boolean;
  };
}
```

**수정 범위 (라인 65-125):**

```typescript
const groupedMessages = useMemo(() => {
  const result: GroupedMessage[] = [];

  let i = 0;
  while (i < messages.length) {
    const msg = messages[i];

    if (
      msg.role === 'assistant' &&
      msg.tool_calls &&
      msg.tool_calls.length > 0
    ) {
      // 1. Text content 확인
      const hasText = msg.content && msg.content.length > 0;

      // 2. Tool results 수집
      const toolCallIds = new Set(msg.tool_calls.map((tc) => tc.id));
      const nextToolResults: Message[] = [];

      let j = i + 1;
      while (j < messages.length && nextToolResults.length < toolCallIds.size) {
        const nextMsg = messages[j];
        if (
          nextMsg.role === 'tool' &&
          nextMsg.tool_call_id &&
          toolCallIds.has(nextMsg.tool_call_id)
        ) {
          nextToolResults.push(nextMsg);
          j++;
        } else {
          break;
        }
      }

      // 3. UI Resource 확인
      const hasUIResource = nextToolResults.some((result) =>
        result.content?.some(
          (c) => c.type === 'resource' && c.resource?.mimeType,
        ),
      );

      // 4. 그루핑 판단
      if (!hasText && !hasUIResource && msg.tool_calls.length >= 2) {
        // Tool Group으로 처리
        result.push({
          type: 'tool_group',
          message: msg,
          nextMessages: nextToolResults,
          toolGroup: {
            calls: msg.tool_calls.map((tc) => ({
              toolCall: tc,
              toolResult: nextToolResults.find((r) => r.tool_call_id === tc.id),
            })),
            hasText: false,
            hasUIResource: false,
          },
        });
      } else {
        // 기존 방식 (개별 bubble)
        result.push({
          type: 'single',
          message: msg,
          nextMessages: nextToolResults,
        });
      }

      i = j;
    } else if (msg.role !== 'tool') {
      result.push({
        type: 'single',
        message: msg,
        nextMessages: [],
      });
      i++;
    } else {
      i++;
    }
  }

  return result;
}, [messages]);
```

**렌더링 부분 수정:**

```typescript
{groupedMessages.map(({ type, message, nextMessages, toolGroup }) => (
  type === 'tool_group' && toolGroup ? (
    <ToolCallGroupBubble
      key={message.id}
      message={message}
      toolGroup={toolGroup}
    />
  ) : (
    <MessageBubble
      key={message.id}
      message={message}
      nextMessages={nextMessages}
      currentAssistantName={getAssistantNameForMessage(message)}
    />
  )
))}
```

### 2. ToolCallGroupBubble.tsx - 새 컴포넌트 생성

**파일:** `src/features/chat/ToolCallGroupBubble.tsx` (새 파일)

**주요 기능:**

- Collapsible group UI
- Bottom 4 items visible by default
- Gradient overlay for hidden items
- Group status summary (success/error count)
- Individual tool call compact display

**컴포넌트 구조:**

```typescript
import React, { useState, useMemo } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { Badge } from '@/components/ui/badge';
import { Wrench, ChevronDown, CheckCircle, XCircle } from 'lucide-react';
import { cn } from '@/lib/utils';

interface ToolCallGroupBubbleProps {
  message: Message;
  toolGroup: {
    calls: Array<{
      toolCall: ToolCall;
      toolResult?: Message;
    }>;
    hasText: boolean;
    hasUIResource: boolean;
  };
}

const VISIBLE_COUNT = 4;

export const ToolCallGroupBubble: React.FC<ToolCallGroupBubbleProps> = ({
  toolGroup,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  // Status 계산
  const { successCount, errorCount, runningCount } = useMemo(() => {
    let success = 0, error = 0, running = 0;

    toolGroup.calls.forEach(({ toolResult }) => {
      if (!toolResult) {
        running++;
      } else {
        const hasError = toolResult.content?.some(
          c => c.type === 'text' && (c.text?.startsWith('❌') || c.text?.startsWith('Error:'))
        );
        if (hasError) error++;
        else success++;
      }
    });

    return { successCount: success, errorCount: error, runningCount: running };
  }, [toolGroup.calls]);

  // Visible items 계산
  const visibleCalls = isExpanded
    ? toolGroup.calls
    : toolGroup.calls.slice(-VISIBLE_COUNT);

  const hiddenCount = toolGroup.calls.length - VISIBLE_COUNT;

  return (
    <div className="rounded-lg border border-muted bg-muted/30 mb-2">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b">
        <div className="flex items-center gap-2">
          <Wrench className="w-4 h-4 text-muted-foreground" />
          <span className="font-medium text-sm">
            Tool Executions ({toolGroup.calls.length} calls)
          </span>
        </div>
        <div className="flex items-center gap-2">
          {successCount > 0 && (
            <Badge variant="outline" className="gap-1 border-green-500 text-green-700 bg-green-50 dark:bg-green-950 dark:text-green-300">
              <CheckCircle className="w-3 h-3" />
              {successCount}
            </Badge>
          )}
          {errorCount > 0 && (
            <Badge variant="destructive" className="gap-1">
              <XCircle className="w-3 h-3" />
              {errorCount}
            </Badge>
          )}
        </div>
      </div>

      {/* Gradient Overlay (collapsed only) */}
      {!isExpanded && hiddenCount > 0 && (
        <div className="relative h-8 bg-gradient-to-b from-muted/50 to-transparent border-b border-dashed border-muted-foreground/20">
          <div className="absolute inset-0 flex items-center justify-center">
            <span className="text-xs text-muted-foreground">
              {hiddenCount} older {hiddenCount === 1 ? 'call' : 'calls'} hidden
            </span>
          </div>
        </div>
      )}

      {/* Tool Call List */}
      <div className="p-2 space-y-1">
        {visibleCalls.map(({ toolCall, toolResult }) => (
          <ToolCallCompactItem
            key={toolCall.id}
            toolCall={toolCall}
            toolResult={toolResult}
          />
        ))}
      </div>

      {/* Expand/Collapse Toggle */}
      <div
        className="flex items-center justify-center p-2 border-t cursor-pointer hover:bg-muted/50 transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <span className="text-xs text-muted-foreground">
          {isExpanded ? 'Show Less' : `Show All (${toolGroup.calls.length})`}
        </span>
        <ChevronDown
          className={cn(
            'w-3 h-3 ml-1 transition-transform',
            isExpanded && 'rotate-180'
          )}
        />
      </div>
    </div>
  );
};

// Compact tool call item component
interface ToolCallCompactItemProps {
  toolCall: ToolCall;
  toolResult?: Message;
}

const ToolCallCompactItem: React.FC<ToolCallCompactItemProps> = ({
  toolCall,
  toolResult,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  const toolName = toolCall.function.name.split('__').pop() || toolCall.function.name;

  const hasError = toolResult?.content?.some(
    c => c.type === 'text' && (c.text?.startsWith('❌') || c.text?.startsWith('Error:'))
  );

  const executionTime = toolResult?.metadata?.executionTime;

  const formatTime = (ms: number) => ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;

  return (
    <div
      className={cn(
        'rounded px-2 py-1.5 text-sm cursor-pointer transition-colors',
        hasError ? 'bg-red-50 dark:bg-red-950/30 hover:bg-red-100 dark:hover:bg-red-950/50' :
        'bg-background hover:bg-muted/50'
      )}
      onClick={() => setIsExpanded(!isExpanded)}
    >
      <div className="flex items-center gap-2">
        {!toolResult ? (
          <span className="text-blue-600 dark:text-blue-400">⏳</span>
        ) : hasError ? (
          <XCircle className="w-3 h-3 text-red-600 dark:text-red-400" />
        ) : (
          <CheckCircle className="w-3 h-3 text-green-600 dark:text-green-400" />
        )}
        <span className="flex-1 truncate">{toolName}</span>
        {executionTime && (
          <span className="text-xs text-muted-foreground">
            {formatTime(executionTime)}
          </span>
        )}
        {isExpanded && (
          <ChevronDown className="w-3 h-3 rotate-180" />
        )}
      </div>

      {/* Expanded details */}
      {isExpanded && toolResult && (
        <div className="mt-2 pt-2 border-t text-xs">
          <MessageRenderer
            content={toolResult.content}
            className="text-sm"
          />
        </div>
      )}
    </div>
  );
};
```

### 3. MessageBubbleRouter.tsx - 조건 추가

**파일:** `src/features/chat/MessageBubbleRouter.tsx`

**변경 불필요:**

- ChatMessages에서 `type === 'tool_group'`일 때 직접 `ToolCallGroupBubble` 렌더링
- MessageBubbleRouter는 기존 로직 유지

## 재사용 가능한 연관 코드

### 1. Error Detection Logic

**위치:** `src/features/chat/ToolCallResultBubble.tsx` (라인 37-42)

```typescript
const hasError =
  toolResult?.content?.some(
    (c) =>
      c.type === 'text' &&
      (c.text?.startsWith('❌') || c.text?.startsWith('Error:')),
  ) || false;
```

→ ToolCallGroupBubble의 status 계산에 재사용

### 2. UI Resource Detection

**위치:** `src/features/chat/ToolCallResultBubble.tsx` (라인 45-49)

```typescript
const hasUIResource =
  toolResult?.content?.some(
    (c) => c.type === 'resource' && c.resource?.mimeType,
  ) || false;
```

→ ChatMessages 그루핑 로직에 재사용

### 3. Execution Time Formatting

**위치:** `src/features/chat/ToolCallResultBubble.tsx` (라인 76-81)

```typescript
const formatExecutionTime = (ms: number): string => {
  if (ms < 1000) {
    return `${ms}ms`;
  }
  return `${(ms / 1000).toFixed(1)}s`;
};
```

→ ToolCallCompactItem에 재사용

### 4. Tool Name Parsing

**위치:** `src/features/chat/ToolCallResultBubble.tsx` (라인 64-65)

```typescript
const toolName =
  toolCall.function.name.split('__').pop() || toolCall.function.name;
```

→ ToolCallCompactItem에 재사용

### 5. MessageRenderer Component

**위치:** `src/components/MessageRenderer.tsx`

```typescript
interface MessageRendererProps {
  content: MCPContent[];
  className?: string;
  expandResources?: boolean;
}
```

→ ToolCallCompactItem의 확장 상태에서 결과 표시에 사용

## Test Code 추가 및 수정 가이드

### Unit Tests

**파일:** `src/features/chat/__tests__/ChatMessages.grouping.spec.tsx` (새 파일)

**테스트 케이스:**

```typescript
describe('ChatMessages Tool Call Grouping', () => {
  test('연속된 tool-only calls는 하나의 그룹으로 묶임', () => {
    const messages: Message[] = [
      {
        id: '1',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'tc1',
            type: 'function',
            function: { name: 'tool1', arguments: '{}' },
          },
          {
            id: 'tc2',
            type: 'function',
            function: { name: 'tool2', arguments: '{}' },
          },
          {
            id: 'tc3',
            type: 'function',
            function: { name: 'tool3', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        role: 'tool',
        tool_call_id: 'tc1',
        content: [{ type: 'text', text: 'result1' }],
      },
      {
        id: '3',
        role: 'tool',
        tool_call_id: 'tc2',
        content: [{ type: 'text', text: 'result2' }],
      },
      {
        id: '4',
        role: 'tool',
        tool_call_id: 'tc3',
        content: [{ type: 'text', text: 'result3' }],
      },
    ];

    const grouped = groupMessages(messages);

    expect(grouped).toHaveLength(1);
    expect(grouped[0].type).toBe('tool_group');
    expect(grouped[0].toolGroup?.calls).toHaveLength(3);
  });

  test('text content가 있으면 그룹 분리', () => {
    const messages: Message[] = [
      {
        id: '1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Starting analysis' }],
        tool_calls: [
          {
            id: 'tc1',
            type: 'function',
            function: { name: 'tool1', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        role: 'tool',
        tool_call_id: 'tc1',
        content: [{ type: 'text', text: 'result' }],
      },
    ];

    const grouped = groupMessages(messages);

    expect(grouped).toHaveLength(1);
    expect(grouped[0].type).toBe('single'); // Not grouped
  });

  test('UI resource가 있으면 그룹 분리', () => {
    const messages: Message[] = [
      {
        id: '1',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'tc1',
            type: 'function',
            function: { name: 'tool1', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        role: 'tool',
        tool_call_id: 'tc1',
        content: [
          {
            type: 'resource',
            resource: { uri: 'ui://test', mimeType: 'text/html' },
          },
        ],
      },
    ];

    const grouped = groupMessages(messages);

    expect(grouped[0].type).toBe('single');
  });

  test('단일 tool call은 그룹화하지 않음', () => {
    const messages: Message[] = [
      {
        id: '1',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'tc1',
            type: 'function',
            function: { name: 'tool1', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        role: 'tool',
        tool_call_id: 'tc1',
        content: [{ type: 'text', text: 'result' }],
      },
    ];

    const grouped = groupMessages(messages);

    expect(grouped[0].type).toBe('single'); // Single call threshold
  });
});
```

### Component Tests

**파일:** `src/features/chat/__tests__/ToolCallGroupBubble.spec.tsx` (새 파일)

```typescript
describe('ToolCallGroupBubble', () => {
  test('기본 상태에서 최신 4개만 표시', () => {
    const toolGroup = {
      calls: Array.from({ length: 10 }, (_, i) => ({
        toolCall: {
          id: `tc${i}`,
          type: 'function',
          function: { name: `tool${i}`, arguments: '{}' },
        },
        toolResult: {
          id: `tr${i}`,
          role: 'tool',
          tool_call_id: `tc${i}`,
          content: [{ type: 'text', text: `result${i}` }],
          metadata: { executionTime: 100 },
        },
      })),
      hasText: false,
      hasUIResource: false,
    };

    const { queryAllByText } = render(
      <ToolCallGroupBubble message={mockMessage} toolGroup={toolGroup} />
    );

    const visibleTools = queryAllByText(/^tool/);
    expect(visibleTools).toHaveLength(4);
  });

  test('Show All 클릭 시 전체 표시', () => {
    // ... similar setup

    const { getByText, queryAllByText } = render(
      <ToolCallGroupBubble message={mockMessage} toolGroup={toolGroup} />
    );

    fireEvent.click(getByText(/Show All/));

    const allTools = queryAllByText(/^tool/);
    expect(allTools).toHaveLength(10);
  });

  test('Error count와 success count 정확히 표시', () => {
    const toolGroup = {
      calls: [
        { toolCall: mockToolCall, toolResult: successResult },
        { toolCall: mockToolCall, toolResult: successResult },
        { toolCall: mockToolCall, toolResult: errorResult },
      ],
      hasText: false,
      hasUIResource: false,
    };

    const { getByText } = render(
      <ToolCallGroupBubble message={mockMessage} toolGroup={toolGroup} />
    );

    expect(getByText('2')).toBeInTheDocument(); // success badge
    expect(getByText('1')).toBeInTheDocument(); // error badge
  });
});
```

### Integration Tests

**파일:** `src/features/chat/__tests__/ChatMessages.integration.spec.tsx`

```typescript
describe('ChatMessages with Tool Grouping - Integration', () => {
  test('전체 플로우: 10개 tool calls → 그룹 표시 → 확장', async () => {
    const { getByText, queryAllByText } = render(
      <ChatProvider>
        <ChatMessages />
      </ChatProvider>
    );

    // Initial: 4 visible
    await waitFor(() => {
      const visible = queryAllByText(/^tool/);
      expect(visible).toHaveLength(4);
    });

    // Expand
    fireEvent.click(getByText(/Show All/));

    await waitFor(() => {
      const all = queryAllByText(/^tool/);
      expect(all).toHaveLength(10);
    });
  });
});
```

## 추가 분석 과제

### 1. 그루핑 임계값 (Threshold) 결정

**현재 제안:** 2개 이상의 tool call을 그룹화

**검토 필요:**

- 단일 tool call은 항상 개별 bubble인가?
- 2개도 그룹화? 아니면 3개 이상?
- 사용자 설정으로 제공?

**분석 방법:**

- 실제 사용 패턴 데이터 수집
- A/B 테스트 (2 vs 3 threshold)

### 2. Visible Count 최적화

**현재 제안:** 하단 4개 표시

**검토 필요:**

- 화면 크기별 최적값 (모바일 vs 데스크탑)
- 사용자 설정 가능 여부
- 동적 조정 (그룹 크기에 따라)

**분석 방법:**

- 다양한 화면 크기에서 UX 테스트
- Eye-tracking 연구

### 3. Error 발생 시 자동 스크롤

**현재 미정의:** Error tool call이 hidden 상태일 때 처리

**검토 필요:**

- 자동으로 "Show All" + 해당 항목으로 스크롤?
- Badge에 경고 표시만?
- 별도 알림?

### 4. 성능 최적화

**대량 Tool Call 시나리오:**

- 100개 이상 tool call이 하나의 그룹에 포함될 경우
- Virtualization 필요 여부 (react-window, react-virtualized)
- Lazy rendering 전략

**분석 필요:**

- 성능 프로파일링 (React DevTools Profiler)
- 메모리 사용량 측정

### 5. 접근성 (Accessibility)

**검토 필요:**

- 스크린 리더 지원 (ARIA labels)
- 키보드 네비게이션 (Tab, Enter, Space)
- Focus 관리 (확장 시 첫 항목으로 이동?)

## Clarification Q-list

### Q1: 그루핑 임계값

**질문:** 몇 개 이상의 tool call을 그룹으로 묶을까요?

**옵션:**

- A) 2개 이상
- B) 3개 이상
- C) 사용자 설정 가능

**현재 제안:** A (2개 이상)

---

### Q2: Text Content + Tool Calls 혼합 시나리오

**질문:** 하나의 assistant 메시지에 text content와 tool calls가 모두 있을 때 어떻게 처리할까요?

**시나리오:**

```json
{
  "role": "assistant",
  "content": [{ "type": "text", "text": "파일들을 분석하겠습니다" }],
  "tool_calls": [
    { "id": "tc1", "function": { "name": "readFile", "arguments": "{}" } },
    { "id": "tc2", "function": { "name": "analyzeCode", "arguments": "{}" } }
  ]
}
```

**옵션:**

- A) ContentBubble + 개별 ToolCallResultBubble (현재 방식)
- B) ContentBubble + ToolCallGroupBubble
- C) 통합된 하나의 복합 Bubble

**현재 제안:** A (기존 방식 유지, 그룹화 안 함)

---

### Q3: UI Resource 포함 Tool Call 처리

**질문:** UI resource를 포함한 tool call을 항상 독립 bubble로 분리할까요?

**시나리오:**

```
tool_call_1 (normal) ──┐
tool_call_2 (normal)   ├─ Group?
tool_call_3 (UI res)  ──┘
tool_call_4 (normal) ──→ New group?
```

**옵션:**

- A) UI resource 포함 시 항상 분리 (제안)
- B) 그룹 내에서 UI resource 처리 (확장 시에만 표시)

**현재 제안:** A (독립 bubble로 분리)

---

### Q4: Gradient Overlay 디자인

**질문:** Hidden items를 나타내는 gradient overlay의 시각적 표현은?

**옵션:**

- A) Simple gradient + text ("6 older calls hidden")
- B) Stacked cards visual effect
- C) Blur effect with count badge

**현재 제안:** A (심플한 gradient + 텍스트)

---

### Q5: Error가 Hidden 영역에 있을 때

**질문:** Error가 발생한 tool call이 collapsed 상태에서 보이지 않을 때 어떻게 알릴까요?

**옵션:**

- A) Header의 error badge만 표시
- B) 자동으로 "Show All" 확장 + 해당 항목 하이라이트
- C) 별도 경고 아이콘 + 클릭 시 해당 항목으로 스크롤

**현재 제안:** A (Header badge만, 사용자가 수동으로 확장)

---

## 구현 순서 제안

1. **Phase 1: 데이터 구조 준비 (1-2시간)**
   - `GroupedMessage` 인터페이스 확장
   - ChatMessages.tsx 그루핑 로직 구현
   - 단위 테스트 작성

2. **Phase 2: UI 컴포넌트 (2-3시간)**
   - ToolCallGroupBubble.tsx 생성
   - ToolCallCompactItem 구현
   - 스타일링 및 반응형 처리

3. **Phase 3: 통합 및 테스트 (1-2시간)**
   - ChatMessages 렌더링 로직 업데이트
   - 컴포넌트 테스트 작성
   - 통합 테스트 및 E2E 시나리오

4. **Phase 4: 최적화 및 접근성 (1시간)**
   - 성능 프로파일링
   - ARIA labels 추가
   - 키보드 네비게이션 구현

**총 예상 시간:** 5-8시간

---

## 참고 자료

- [Existing ToolCallResultBubble Implementation](../features/chat/ToolCallResultBubble.tsx)
- [ChatMessages Grouping Logic](../features/chat/components/ChatMessages.tsx#L65-L125)
- [Message Data Model](../../models/chat.ts)
- [MCP Content Types](../../lib/mcp-types.ts)
