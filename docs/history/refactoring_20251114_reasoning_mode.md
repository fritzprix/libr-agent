# Reasoning Mode 구현 리팩토링 계획

**날짜**: 2025-11-14  
**작업자**: AI Agent  
**브랜치**: dev/0.2.0

> **용어 통일**: `thinking` → `reasoning`  
> API에서 제공하는 파라미터명이 대부분 `reasoning`을 사용하므로 (OpenRouter: `reasoning`, OpenAI: `reasoning_effort`, Gemini: `reasoning` via OpenAI endpoint), 코드 전반에서 `reasoning`으로 용어를 통일합니다.

---

## 1. 작업의 목적

AI 모델의 Reasoning 모드를 사용자가 **대화별로 임시 활성화**할 수 있는 기능 구현. 이를 통해:

- **비용 절감**: 복잡한 문제에만 선택적으로 사용 (reasoning 토큰은 일반 토큰보다 비쌈)
- **투명성 확보**: 사용자가 AI의 사고 과정(reasoning process)을 확인 가능
- **성능 최적화**: 단순 질문에는 일반 모드, 복잡한 문제에는 reasoning 모드 활용

### 핵심 요구사항

1. **전역 설정 금지**: Settings가 아닌 ChatContext 내 임시 상태로 관리
2. **동적 탐지**: 정적 JSON 설정 파일 없이 런타임에 모델 capability 확인
3. **다중 Provider 지원**: Ollama, OpenAI, Anthropic, Gemini 등 통합 처리
4. **Zero-Maintenance**: OpenRouter API를 메타데이터 소스로 활용

---

## 2. 현재의 상태 / 문제점

### 2.1 하드코딩된 Reasoning 파라미터

**파일**: `src/lib/ai-service/ollama.ts`

```typescript
// Line 189: 하드코딩됨
const requestOptions: ChatRequest & { stream: true } = {
  model,
  messages: ollamaMessages as OllamaMessage[],
  stream: true,
  think: true, // ❌ 항상 활성화됨, 사용자 제어 불가
  tools: ollamaTools,
  // ...
};
```

**문제점**:

- 모든 Ollama 요청에서 reasoning 모드(`think` 파라미터)가 강제 활성화
- 사용자가 비용 관리 불가
- 단순 질문에도 불필요한 reasoning 토큰 소비

### 2.2 Provider별 Reasoning 파라미터 불일치

각 Provider가 서로 다른 파라미터명과 값 타입 사용 (하지만 대부분 `reasoning` 용어 사용):

| Provider  | 파라미터명                      | 값 타입                                  | 예시                       |
| --------- | ------------------------------- | ---------------------------------------- | -------------------------- |
| Ollama    | `think`                         | `boolean \| 'low' \| 'medium' \| 'high'` | `think: 'medium'`          |
| OpenAI    | `reasoning_effort`              | `'low' \| 'medium' \| 'high'`            | `reasoning_effort: 'high'` |
| Anthropic | `extended_thinking`             | `boolean`                                | `extended_thinking: true`  |
| Gemini    | `thinkingConfig.thinkingBudget` | `-1 \| 0 \| number`                      | `thinkingBudget: 1024`     |

**문제점**:

- Provider별로 별도 로직 필요
- 통일된 인터페이스 부재
- 신규 Provider 추가 시 매번 수정 필요

### 2.3 Capability 탐지 메커니즘 부재

현재 시스템은 모델이 reasoning을 지원하는지 **알 수 없음**:

- `gpt-3.5-turbo`에 `reasoning_effort` 파라미터 전송 시 API 에러 발생 가능
- Ollama의 일반 모델(예: `llama3.2`)에 `think: true` 전송 시 무시됨 (reasoning 미지원 모델)
- UI에서 지원 여부를 표시할 방법 없음

### 2.4 기존 구현된 동적 탐지의 한계

**파일**: `src/lib/ai-service/model-capabilities.ts` (이미 일부 구현됨)

```typescript
// Ollama API를 통한 동적 탐지 (✅ 좋은 접근)
export async function fetchOllamaModelInfo(
  modelName: string,
  apiBase: string = 'http://localhost:11434',
): Promise<{ thinking: boolean; contextWindow?: number } | null>;

// 하지만 다른 Provider는 패턴 매칭에 의존 (❌ 유지보수 부담)
const FALLBACK_THINKING_PATTERNS: Record<string, string[]> = {
  [AIServiceProvider.OpenAI]: ['o1', 'o3', 'o4'],
  // ...
};
```

**문제점**:

- Ollama 외 Provider는 여전히 정적 패턴 의존
- 새로운 모델(예: `o5`, `claude-4`) 출시 시 코드 수정 필요
- OpenRouter API를 활용하면 모든 Provider 통합 가능

---

## 3. 관련 코드의 구조 및 동작 방식 Summary

### 3.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      UI Layer                                │
│  ChatHeader.tsx                                              │
│  - Brain icon toggle (thinkingEnabled 상태 표시)             │
│  - canUseThinking이 true일 때만 표시                          │
└────────────────┬────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────┐
│                  State Management                            │
│  ChatContext.tsx                                             │
│  - thinkingEnabled: boolean (default: false)                 │
│  - canUseThinking: boolean (computed from supportsThinking)  │
│  - toggleThinking(): void                                    │
└────────────────┬────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────┐
│                  Service Layer                               │
│  AIService implementations                                   │
│  - ollama.ts, openai.ts, anthropic.ts, gemini.ts            │
│  - streamChat() receives config.enableThinking               │
│  - Converts to provider-specific parameters                  │
└────────────────┬────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────┐
│              Capability Detection                            │
│  model-capabilities.ts                                       │
│  Tier 1: OpenRouter metadata API (all providers except Ollama)│
│  Tier 2: Provider API (Ollama /api/show)                    │
│  Tier 3: Fallback patterns (offline/error cases)            │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Data Flow

**사용자 Brain 아이콘 클릭 → UI 업데이트 → API 호출 전달 과정**:

```typescript
1. User clicks Brain icon in ChatHeader
   ↓
2. ChatContext.toggleThinking() called
   ↓
3. thinkingEnabled state updated (true/false)
   ↓
4. User submits message
   ↓
5. ChatContext.submit() builds AIServiceConfig
   config = {
     enableThinking: thinkingEnabled,
     thinkingLevel: 'medium', // or user-selected
     // ...
   }
   ↓
6. AIService.streamChat(messages, { config })
   ↓
7. Provider-specific parameter mapping:
   - Ollama: { think: 'medium' }
   - OpenAI: { reasoning_effort: 'medium' }
   - Anthropic: { extended_thinking: true }
   ↓
8. API request sent with thinking parameters
```

### 3.3 기존 구현 현황

#### ✅ 이미 완료된 부분

1. **OpenRouter metadata fetcher** (`src/lib/ai-service/openrouter-metadata.ts`)
   - `fetchOpenRouterModels()`: 400+ 모델 메타데이터 조회
   - `supportsReasoningViaOpenRouter()`: `supported_parameters`에서 "reasoning" 확인
   - 24시간 캐싱으로 API 호출 최소화

2. **model-capabilities.ts 리팩토링**
   - `supportsThinking()`: 3-tier 탐지 전략 구현
   - Tier 1: OpenRouter API (우선)
   - Tier 2: Ollama `/api/show` (Ollama 전용)
   - Tier 3: 최소 패턴 매칭 (fallback)

#### ⏳ 진행 중/미완료 부분

1. **AIServiceConfig 타입 정의** (types.ts) - ❌ Undo됨
2. **Provider별 thinking 파라미터 적용** - ❌ Undo됨
3. **ChatContext 상태 통합** - 미작업
4. **ChatHeader UI 구현** - 미작업

---

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1 성공 기준

✅ **기능적 요구사항**:

- [ ] 사용자가 ChatHeader에서 Brain 아이콘 클릭 시 thinking 모드 토글 가능
- [ ] thinking 미지원 모델에서는 Brain 아이콘 숨김
- [ ] Ollama, OpenAI, Anthropic 각 Provider에 올바른 파라미터 전달
- [ ] 대화별 상태 관리 (전역 설정 아님)

✅ **비기능적 요구사항**:

- [ ] OpenRouter API 장애 시에도 fallback으로 동작
- [ ] 24시간 캐싱으로 네트워크 호출 최소화
- [ ] 타입 안전성 보장 (TypeScript strict mode)
- [ ] 기존 대화에 영향 없음 (backward compatible)

### 4.2 테스트 시나리오

#### Scenario 1: Ollama Thinking 모드 활성화

```typescript
// Given: qwen2.5:latest 모델 선택
const canThink = await supportsThinking('qwen2.5:latest', 'ollama');
// Expected: true (Ollama API /api/show 확인)

// When: Brain 아이콘 클릭 → thinkingEnabled = true
// Then: API 요청에 think: 'medium' 포함
```

#### Scenario 2: OpenAI o1 모델

```typescript
// Given: o1-preview 모델 선택
const canThink = await supportsThinking('o1-preview', 'openai');
// Expected: true (OpenRouter metadata 확인)

// When: thinkingEnabled = true, thinkingLevel = 'high'
// Then: API 요청에 reasoning_effort: 'high' 포함
```

#### Scenario 3: 일반 모델 (thinking 미지원)

```typescript
// Given: gpt-3.5-turbo 모델 선택
const canThink = await supportsThinking('gpt-3.5-turbo', 'openai');
// Expected: false (OpenRouter metadata에 "reasoning" 없음)

// Then: Brain 아이콘 표시 안 됨
// And: config.enableThinking 무시 (파라미터 전송 안 함)
```

---

## 5. 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 5.1 AIServiceConfig 타입 확장

**파일**: `src/lib/ai-service/types.ts`

**현재 코드**:

```typescript
export interface AIServiceConfig {
  timeout?: number;
  maxRetries?: number;
  retryDelay?: number;
  defaultModel?: string;
  maxTokens?: number;
  temperature?: number;
  tools?: MCPTool[];
}
```

**수정 후**:

```typescript
export interface AIServiceConfig {
  timeout?: number;
  maxRetries?: number;
  retryDelay?: number;
  defaultModel?: string;
  maxTokens?: number;
  temperature?: number;
  tools?: MCPTool[];

  /**
   * Enable thinking/reasoning mode for supported models.
   * Per-conversation temporary setting.
   *
   * Provider behaviors:
   * - Ollama: think: true | 'low' | 'medium' | 'high'
   * - OpenAI: reasoning_effort: 'low' | 'medium' | 'high' (o1/o3/o4 only)
   * - Anthropic: extended_thinking: true (Claude 3.5+)
   * - Gemini: thinkingConfig.thinkingBudget: -1 | 0 | number (tokens)
   */
  enableThinking?: boolean;

  /**
   * Reasoning depth level when thinking is enabled.
   * - 'low': Fast, minimal reasoning
   * - 'medium': Balanced (default)
   * - 'high': Deep reasoning (higher cost)
   */
  thinkingLevel?: 'low' | 'medium' | 'high';
}
```

### 5.2 Ollama Service 수정

**파일**: `src/lib/ai-service/ollama.ts`

**현재 코드** (Line 185-198):

```typescript
const requestOptions: ChatRequest & { stream: true } = {
  model,
  messages: ollamaMessages as OllamaMessage[],
  stream: true,
  think: true, // ❌ 하드코딩
  tools: ollamaTools,
  keep_alive: '5m',
  options: {
    temperature: config.temperature || 0.7,
    num_predict: config.maxTokens || 4096,
  },
};
```

**수정 후**:

```typescript
// Prepare thinking parameter based on config
// Ollama expects: think: true | 'low' | 'medium' | 'high'
let thinkParam: boolean | 'low' | 'medium' | 'high' | undefined;
if (config.enableThinking) {
  thinkParam = config.thinkingLevel || true; // Default to true if no level
}

logger.info('Ollama API call:', {
  model,
  messagesCount: ollamaMessages.length,
  thinkingEnabled: config.enableThinking,
  thinkingLevel: config.thinkingLevel,
  toolsCount: ollamaTools.length,
});

const requestOptions: ChatRequest & { stream: true } = {
  model,
  messages: ollamaMessages as OllamaMessage[],
  stream: true,
  ...(thinkParam && { think: thinkParam }), // ✅ Conditional inclusion
  tools: ollamaTools,
  keep_alive: '5m',
  options: {
    temperature: config.temperature || 0.7,
    num_predict: config.maxTokens || 4096,
  },
};
```

### 5.3 OpenAI Service 수정

**파일**: `src/lib/ai-service/openai.ts`

**현재 코드** (Line 193-210):

```typescript
const completion = await this.withRetry(() =>
  this.openai.chat.completions.create(
    {
      model: modelName,
      messages: openaiMessages,
      max_completion_tokens: config.maxTokens,
      stream: true,
      tools: tools as OpenAIChatCompletionTool[],
      tool_choice: !options.availableTools?.length
        ? undefined
        : options.forceToolUse
          ? 'required'
          : 'auto',
    },
    { signal: this.getAbortSignal() },
  ),
);
```

**수정 후**:

```typescript
// Prepare reasoning_effort for o1/o3/o4 models
let reasoningEffort: 'low' | 'medium' | 'high' | undefined;
if (config.enableThinking && config.thinkingLevel) {
  // OpenAI o-series models use reasoning_effort parameter
  // Only applies to o1-*, o3-*, o4-* models
  const isReasoningModel = /^o[134](-|$)/.test(modelName);
  if (isReasoningModel) {
    reasoningEffort = config.thinkingLevel;
    logger.info('OpenAI reasoning mode enabled', {
      model: modelName,
      reasoning_effort: reasoningEffort,
    });
  }
}

const completion = await this.withRetry(() =>
  this.openai.chat.completions.create(
    {
      model: modelName,
      messages: openaiMessages,
      max_completion_tokens: config.maxTokens,
      stream: true,
      ...(reasoningEffort && { reasoning_effort: reasoningEffort }),
      tools: tools as OpenAIChatCompletionTool[],
      tool_choice: !options.availableTools?.length
        ? undefined
        : options.forceToolUse
          ? 'required'
          : 'auto',
    },
    { signal: this.getAbortSignal() },
  ),
);
```

### 5.4 Anthropic Service 수정

**파일**: `src/lib/ai-service/anthropic.ts`

**현재 코드** (Line 216-228):

```typescript
const stream = this.anthropic.messages.stream(
  {
    model: options.modelName || this.getDefaultModel(),
    max_tokens: config.maxTokens!,
    messages: anthropicMessages,
    system: options.systemPrompt,
    tools: tools as AnthropicTool[],
    ...(options.forceToolUse &&
      options.availableTools?.length && { tool_choice: { type: 'any' } }),
  },
  { signal: this.getAbortSignal() },
);
```

**수정 후**:

```typescript
// Anthropic Claude 3.5+ supports extended_thinking (boolean only)
let extendedThinking: boolean | undefined;
if (config.enableThinking) {
  const model = options.modelName || this.getDefaultModel();
  // Check if Claude 3.5 or higher
  const isClaude35Plus = /claude-3\.[5-9]|claude-[4-9]/.test(model);
  if (isClaude35Plus) {
    extendedThinking = true;
    logger.info('Anthropic extended thinking enabled', { model });
  }
}

const stream = this.anthropic.messages.stream(
  {
    model: options.modelName || this.getDefaultModel(),
    max_tokens: config.maxTokens!,
    messages: anthropicMessages,
    system: options.systemPrompt,
    ...(extendedThinking && { extended_thinking: extendedThinking }),
    tools: tools as AnthropicTool[],
    ...(options.forceToolUse &&
      options.availableTools?.length && { tool_choice: { type: 'any' } }),
  },
  { signal: this.getAbortSignal() },
);
```

### 5.5 ChatContext 상태 추가

**파일**: `src/context/ChatContext.tsx`

**추가할 State**:

```typescript
// --- STATE CONTEXT ---
interface ChatStateContextValue {
  isLoading: boolean;
  isToolExecuting: boolean;
  messages: Message[];
  pendingCancel: boolean;
  error: Message['error'] | null;
  agenticMode: boolean;

  // ✅ NEW: Thinking mode state
  thinkingEnabled: boolean;
  canUseThinking: boolean; // Computed from supportsThinking()
}

// --- ACTIONS CONTEXT ---
interface ChatActionsContextValue {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
  cancel: () => void;
  addToMessageQueue: (message: Partial<Message>) => void;
  retryMessage: () => Promise<void>;
  setAgenticMode: (enabled: boolean) => void;

  // ✅ NEW: Thinking toggle
  toggleThinking: () => void;
}
```

**추가할 Implementation**:

```typescript
export function ChatProvider({ children }: ChatProviderProps) {
  // ... existing state ...

  const [thinkingEnabled, setThinkingEnabled] = useState(false);
  const [canUseThinking, setCanUseThinking] = useState(false);

  // Check if current model supports thinking
  useEffect(() => {
    const checkThinkingSupport = async () => {
      if (!currentAssistant?.model?.name || !currentAssistant?.provider) {
        setCanUseThinking(false);
        return;
      }

      const supports = await supportsThinking(
        currentAssistant.model.name,
        currentAssistant.provider,
      );
      setCanUseThinking(supports);

      // Auto-disable if model doesn't support
      if (!supports && thinkingEnabled) {
        setThinkingEnabled(false);
      }
    };

    checkThinkingSupport();
  }, [currentAssistant?.model?.name, currentAssistant?.provider]);

  const toggleThinking = useCallback(() => {
    if (!canUseThinking) {
      logger.warn('Thinking mode not supported for current model');
      return;
    }
    setThinkingEnabled(prev => !prev);
  }, [canUseThinking]);

  // Modify submit() to include thinking config
  const submit = useCallback(async (...) => {
    // ... existing code ...

    const config: AIServiceConfig = {
      maxTokens: settingValue?.maxTokens,
      temperature: settingValue?.temperature,
      enableThinking: thinkingEnabled, // ✅ Add thinking config
      thinkingLevel: 'medium', // or from user selection
    };

    // ... rest of submit logic ...
  }, [thinkingEnabled, /* other deps */]);

  // ... rest of provider implementation ...
}
```

### 5.6 ChatHeader UI 추가

**파일**: `src/features/chat/components/ChatHeader.tsx`

**추가할 코드**:

```typescript
import { Brain } from 'lucide-react';
import { useChatState, useChatActions } from '@/context/ChatContext';

export function ChatHeader({ children, assistantName }: ChatHeaderProps) {
  // ... existing code ...

  const { thinkingEnabled, canUseThinking } = useChatState();
  const { toggleThinking } = useChatActions();

  return (
    <TerminalHeader>
      <div className="flex items-center justify-between w-full">
        {/* ... existing left side ... */}

        <div className="flex items-center gap-2">
          {/* ✅ NEW: Thinking mode toggle */}
          {canUseThinking && (
            <Button
              variant="ghost"
              size="sm"
              onClick={toggleThinking}
              title={`Thinking Mode: ${thinkingEnabled ? 'ON (Deep reasoning enabled - higher cost)' : 'OFF (Standard mode)'}`}
              className="h-6 px-2"
            >
              <Brain
                className={`h-4 w-4 ${thinkingEnabled ? 'text-purple-400' : ''}`}
              />
            </Button>
          )}

          {/* ... existing buttons (Agent, Workspace, Planning) ... */}
        </div>
      </div>
    </TerminalHeader>
  );
}
```

---

## 6. 재사용 가능한 연관 코드

### 6.1 이미 구현된 유틸리티

**파일**: `src/lib/ai-service/openrouter-metadata.ts`

- `fetchOpenRouterModels()`: 400+ 모델 메타데이터 조회
- `findModelMetadata(modelId, provider)`: 모델 ID로 메타데이터 검색
- `supportsReasoningViaOpenRouter(modelId, provider)`: reasoning 지원 여부 확인
- `getReasoningTokenCost(modelId, provider)`: reasoning 토큰 비용 조회

**재사용 방법**:

```typescript
// ChatHeader에서 비용 정보 표시
const cost = await getReasoningTokenCost(modelName, provider);
if (cost) {
  // Tooltip: "Reasoning tokens: $X.XX per million"
}
```

### 6.2 Provider별 파라미터 매핑 참고

**참고 문서**:

- Ollama: `think` parameter - https://ollama.com/docs/api#think-parameter
- OpenAI: `reasoning_effort` - (Context7에서 조회 중)
- Anthropic: `extended_thinking` - Anthropic API docs
- Gemini: (확인 필요)

---

## 7. Test Code 추가 및 수정 필요 부분

### 7.1 Unit Tests

**파일**: `src/lib/ai-service/__tests__/model-capabilities.test.ts`

```typescript
describe('supportsThinking', () => {
  it('should detect Ollama thinking via API', async () => {
    // Mock fetch for /api/show
    global.fetch = jest.fn(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            modelfile: 'PARAMETER think true',
          }),
      }),
    );

    const result = await supportsThinking(
      'qwen2.5:latest',
      AIServiceProvider.Ollama,
    );
    expect(result).toBe(true);
  });

  it('should fallback to OpenRouter for OpenAI models', async () => {
    // Mock OpenRouter API
    // ...
    const result = await supportsThinking(
      'o1-preview',
      AIServiceProvider.OpenAI,
    );
    expect(result).toBe(true);
  });

  it('should use fallback patterns when APIs fail', async () => {
    global.fetch = jest.fn(() => Promise.reject('Network error'));

    const result = await supportsThinking('o1-mini', AIServiceProvider.OpenAI);
    expect(result).toBe(true); // Fallback pattern match
  });
});
```

### 7.2 Integration Tests

**파일**: `src/lib/ai-service/__tests__/ollama.integration.test.ts`

```typescript
describe('Ollama thinking parameter', () => {
  it('should include think parameter when enabled', async () => {
    const service = new OllamaService('http://localhost:11434');
    const config: AIServiceConfig = {
      enableThinking: true,
      thinkingLevel: 'high',
    };

    // Mock ollama.chat to capture request
    const chatSpy = jest.spyOn(service['ollamaClient'], 'chat');

    const stream = service.streamChat(messages, { config });
    await stream.next();

    expect(chatSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        think: 'high',
      }),
    );
  });

  it('should omit think parameter when disabled', async () => {
    const config: AIServiceConfig = {
      enableThinking: false,
    };

    const chatSpy = jest.spyOn(service['ollamaClient'], 'chat');
    const stream = service.streamChat(messages, { config });
    await stream.next();

    expect(chatSpy).toHaveBeenCalledWith(
      expect.not.objectContaining({
        think: expect.anything(),
      }),
    );
  });
});
```

### 7.3 E2E Tests (Playwright/Cypress)

**파일**: `e2e/thinking-mode.spec.ts`

```typescript
test('should toggle thinking mode for supported models', async ({ page }) => {
  // Navigate to chat with Ollama qwen2.5
  await page.goto('/chat');
  await page.selectOption('[data-testid=model-select]', 'qwen2.5:latest');

  // Brain icon should be visible
  const brainIcon = page.locator('[data-testid=thinking-toggle]');
  await expect(brainIcon).toBeVisible();

  // Click to enable
  await brainIcon.click();
  await expect(brainIcon).toHaveClass(/text-purple-400/);

  // Send message and verify API call
  await page.fill('[data-testid=message-input]', 'Solve x^2 + 5x + 6 = 0');
  await page.click('[data-testid=send-button]');

  // Intercept API call and verify think parameter
  const request = await page.waitForRequest('/api/chat');
  const payload = await request.postDataJSON();
  expect(payload.think).toBe('medium');
});

test('should hide brain icon for unsupported models', async ({ page }) => {
  await page.goto('/chat');
  await page.selectOption('[data-testid=model-select]', 'gpt-3.5-turbo');

  const brainIcon = page.locator('[data-testid=thinking-toggle]');
  await expect(brainIcon).not.toBeVisible();
});
```

---

## 8. 작업 추가 분석 과제

### 8.1 OpenAI reasoning_effort 파라미터 확인

**질문**: OpenAI o1/o3/o4 모델의 `reasoning_effort` 파라미터 정확한 스펙은?

**조사 방법**:

- Context7로 OpenAI Node SDK 문서 조회 (진행 중)
- OpenAI API Reference 확인
- 실제 API 테스트로 검증

**예상 결과**:

```typescript
interface ChatCompletionCreateParams {
  // ...
  reasoning_effort?: 'low' | 'medium' | 'high'; // o1/o3/o4 only
}
```

### 8.2 Gemini Thinking Mode 구현 확인 ✅

**결론**: Gemini는 **`thinkingConfig` 파라미터 방식**을 지원합니다!

**공식 API 파라미터**:

```typescript
interface GeminiThinkingConfig {
  thinkingBudget?: number; // -1 (동적) | 0 (비활성) | 양수 (토큰 수)
  includeThoughts?: boolean; // Thinking 과정 반환 여부
}

interface GenerateContentConfig {
  thinkingConfig?: GeminiThinkingConfig;
}
```

**사용 예시**:

```typescript
// Google Gen AI SDK
const response = await ai.models.generateContent({
  model: 'gemini-2.5-flash',
  contents: prompt,
  config: {
    thinkingConfig: {
      thinkingBudget: 1024,      // 고정 1024 토큰
      includeThoughts: true      // 추론 과정 포함
    }
  }
});

// OpenAI 호환 엔드포인트
const response = await openai.chat.completions.create({
  model: 'gemini-2.5-flash',
  reasoning_effort: 'low',  // 'low' | 'medium' | 'high'
  messages: [...]
});
```

**`thinkingBudget` 값 범위**:

- **Gemini 2.5 Pro**: 128 ~ 32,768 tokens
- **Gemini 2.5 Flash**: 0 ~ 24,576 tokens
- `0`: Thinking 완전 비활성화
- `-1`: 동적 Thinking (모델이 요청 복잡도에 따라 자동 조절)
- 양수: 고정 토큰 수

**OpenAI 호환 `reasoning_effort` 매핑**:

- `'low'` → `thinkingBudget: 1024`
- `'medium'` → `thinkingBudget: 8192`
- `'high'` → `thinkingBudget: 24576`

**공식 문서**:

- [Gemini Thinking Mode](https://ai.google.dev/gemini-api/docs/thinking-mode)
- [Gemini OpenAI Compatibility](https://ai.google.dev/gemini-api/docs/openai)

**구현 전략**:

```typescript
// gemini.ts 수정
const config: GenerateContentConfig = {
  ...baseConfig,
  thinkingConfig: serviceConfig.enableThinking
    ? {
        thinkingBudget: mapThinkingLevel(serviceConfig.thinkingLevel),
        includeThoughts: true,
      }
    : {
        thinkingBudget: 0, // 명시적 비활성화
      },
};

function mapThinkingLevel(level?: 'low' | 'medium' | 'high'): number {
  switch (level) {
    case 'low':
      return 1024;
    case 'medium':
      return 8192;
    case 'high':
      return 24576;
    default:
      return -1; // 동적 조절
  }
}
```

**필요 작업**:

- `gemini.ts`의 `GenerateContentConfig`에 `thinkingConfig` 추가
- `GeminiServiceConfig` 인터페이스에 `enableThinking`, `thinkingLevel` 추가
- `includeThoughts: true` 설정 시 응답에서 `thought` 필드 처리

### 8.3 Provider별 Thinking Level 매핑

**질문**: 각 Provider의 thinking level이 실제로 어떻게 동작하는가?

| Provider  | Low       | Medium        | High      | 비고           |
| --------- | --------- | ------------- | --------- | -------------- |
| Ollama    | 빠른 응답 | 균형잡힌 사고 | 깊은 추론 | boolean도 허용 |
| OpenAI    | ?         | ?             | ?         | 확인 필요      |
| Anthropic | N/A       | N/A           | N/A       | boolean only   |

**조사 방법**:

- 각 Provider 공식 문서 확인
- 실제 API 호출로 응답 시간/품질 비교
- 비용 차이 측정

---

## 9. Clarification Q-list

### Q1. Thinking Level UI 위치

**질문**: Thinking level('low', 'medium', 'high') 선택은 어디에서 하는가?

**옵션**:

- A) ChatHeader에 드롭다운 추가
- B) Settings 모달에 전역 설정 (❌ 요구사항 위배)
- C) Brain 아이콘 클릭 시 팝오버로 level 선택
- D) 자동 결정 (복잡도에 따라 동적 선택)

**추천**: Option C (팝오버) - 대화별 설정 + UI 간결성 유지
**답변**: Option C looks good to me

### Q2. OpenRouter API Key 요구 여부

**질문**: OpenRouter metadata API는 무료 공개 엔드포인트인가?

**Q2: OpenRouter API 활용** ✅

**테스트 결과** (`scripts/test_openrouter_access.ts`):

- ✅ `/api/v1/models` 엔드포인트가 **API key 없이 접근 가능**
- ✅ 총 **344개 모델 메타데이터** 제공 (응답 시간: ~140ms)
- ✅ **132개 모델**이 `supported_parameters`에 `reasoning` 포함
- ✅ 모델별 `pricing`, `context_length` 등 상세 메타데이터 포함

**확인된 Reasoning 지원 모델:**

- `google/gemini-2.5-flash` - reasoning, include_reasoning
- `deepseek/deepseek-r1` - reasoning, include_reasoning
- `openai/gpt-5.1` 시리즈 - reasoning
- `moonshotai/kimi-k2-thinking` - reasoning

**결론**: ✅ **OpenRouter를 primary source로 사용 가능**

- 장점: 인증 불필요, 빠른 응답, 통합 메타데이터
- 제한: 일부 최신 모델(o3-mini)의 메타데이터가 아직 미반영될 수 있음
- 전략: OpenRouter → Provider API → Fallback 패턴 유지
- Rate limit: 테스트 결과 제한 없음 (24시간 캐싱으로 충분)

### Q3. Thinking Mode 비용 경고 표시

**질문**: Thinking mode 활성화 시 비용 경고를 어떻게 표시할 것인가?

**옵션**:

- A) Brain 아이콘 hover tooltip에 표시
- B) 첫 활성화 시 모달 경고
- C) ChatHeader에 작은 뱃지 ("💰 High cost mode")
- D) 표시 안 함 (사용자 책임)

**추천**: Option A + B (tooltip + 첫 활성화 모달)
**답변**: Option A + B looks good to me

### Q4. Backward Compatibility

**질문**: 기존 대화에서 thinking mode 설정이 없는 경우 기본값은?

**현재 상태**:

- Ollama는 `think: true`로 하드코딩되어 있음
- 변경 시 기존 대화의 동작이 바뀔 수 있음

**옵션**:

- A) 모든 대화에서 기본값 `false` (비용 절감 우선)
- B) 모델별로 기본값 다르게 (o1/qwen은 true, 나머지 false)
- C) Migration script로 기존 대화 설정 추가

**추천**: Option A (명시적 opt-in 방식)
**답변**: Option A looks good

### Q5. Multi-Provider Support 우선순위

**질문**: 모든 Provider를 한 번에 구현할 것인가, 단계적으로 구현할 것인가?

**Phase 1** (필수):

- Ollama (이미 `think` 파라미터 사용 중)
- OpenAI (o1/o3/o4 모델 수요 높음)

**Phase 2** (선택):

- Anthropic (Claude 3.5+)
- Gemini (Gemini 2.0+)

**Phase 3** (미정):

- Groq, Fireworks, Cerebras (thinking 지원 여부 확인 필요)

**추천**: Phase 1부터 구현하고 나머지는 점진적 추가
**답변**: Ollama, Gemini, Anthropic, OpenAI까지는 모두 구현

---

## 10. Implementation Timeline

### Week 1: Foundation

- [ ] Q1-Q5 의사결정 완료
- [ ] OpenAI reasoning_effort 파라미터 확인 (Context7 조회 완료 후)
- [ ] AIServiceConfig 타입 정의 최종화
- [ ] Unit test 틀 작성

### Week 2: Core Implementation

- [ ] Ollama service 수정 및 테스트
- [ ] OpenAI service 수정 및 테스트
- [ ] ChatContext 상태 통합
- [ ] Integration tests 작성

### Week 3: UI & Polish

- [ ] ChatHeader Brain 아이콘 추가
- [ ] Thinking level selector 구현 (Q1 결정에 따라)
- [ ] Cost warning tooltip/modal (Q3 결정에 따라)
- [ ] E2E tests 작성

### Week 4: Extended Support

- [ ] Anthropic extended_thinking 구현
- [ ] Gemini thinking mode 조사 및 구현
- [ ] OpenRouter API fallback 정책 최적화
- [ ] 문서화 및 코드 리뷰

---

## 11. Risk Mitigation

### Risk 1: OpenRouter API 장애

**완화 전략**:

- 24시간 캐싱으로 재시도 빈도 감소
- Tier 2 (Provider API), Tier 3 (패턴 매칭) fallback
- 캐시 무효화 시에도 stale cache 사용

### Risk 2: Provider API 스펙 변경

**완화 전략**:

- OpenRouter metadata를 primary source로 사용
- Version-specific documentation 참조
- API 에러 발생 시 graceful degradation (thinking 비활성화)

### Risk 3: 비용 폭증

**완화 전략**:

- 기본값 `false` (opt-in 방식)
- 비용 경고 tooltip/modal
- Settings에 월간 비용 추적 추가 (future)

---

## 12. References

- [Ollama API Documentation](https://ollama.com/docs/api)
- [OpenRouter Models API](https://openrouter.ai/docs/overview/models)
- [Anthropic Messages API](https://docs.anthropic.com/claude/reference/messages)
- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat) (Context7 조회 중)

---

**작성 완료일**: 2025-11-14  
**최종 검토자**: (할당 필요)  
**승인 상태**: Draft
