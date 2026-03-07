# Message Context Management

## Overview

LibrAgent manages the LLM context window through a **dual-constraint selection** mechanism: a count-based sliding window combined with a token-budget limit. The goal is to send the most relevant recent messages without exceeding the model's context window or causing API validation errors.

---

## Files Involved

| File                                       | Role                                                                                 |
| ------------------------------------------ | ------------------------------------------------------------------------------------ |
| `src/lib/token-utils.ts`                   | Core selection algorithm (`selectMessagesWithinContext`, `batchToolCallsInMessages`) |
| `src/context/llm/useLLMExecution.ts`       | Caller — computes token budget and invokes selection                                 |
| `src/lib/services/settings-service.ts`     | `Settings.windowSize` definition (default: **20**)                                   |
| `src/lib/ai-service/message-normalizer.ts` | Post-selection sanitization (tool pairing, field stripping)                          |
| `src/lib/ai-service/sanitizer.ts`          | JSON field safety (tool arguments, thinking content)                                 |

---

## Selection Pipeline

```
Input: messages[]  (full conversation history)
         │
         ▼
[1] batchToolCallsInMessages()
    Split assistant messages that exceed maxToolCallsPerMessage (default: 4)
    SKIP if message has thinkingSignature (Gemini atomic turn requirement)
         │
         ▼
[2] Token Budget Calculation
    baseLimit  = min(maxTokens, contextWindow × 0.9)
    reserved   = systemPromptTokens + toolsTokens + pinnedMessageTokens
    tokenLimit = max(1024, baseLimit − reserved)
         │
         ▼
[3] Pin First User Message
    Always include messages[0] if role === 'user' (context anchor)
         │
         ▼
[4] Backward Iteration (newest → oldest)
    For each message:
    ├── [TOKEN CHECK]   totalTokens + msgTokens > tokenLimit → STOP
    └── [COUNT CHECK]   selected.length ≥ windowSize − 1      → STOP
         │
         ▼
[5] removeIncompleteToolChains()   (Anthropic / Gemini / OpenAI / OpenRouter / Groq)
    Remove tool result messages with no matching tool call in selected set
    Trim tool_calls from assistant messages with no corresponding result
         │
         ▼
[6] prependPinnedMessage()
    If pinned message role === 'user' AND first selected role === 'user'
    → Merge content with separator to avoid consecutive user messages
         │
         ▼
[7] MessageNormalizer.sanitizeMessagesForProvider()
    + sanitizeMessage() (JSON escape, thinking field cleanup)
         │
         ▼
Output: contextMessages[]  (sent to LLM)
```

---

## Configuration

Defined in `Settings` (`src/lib/services/settings-service.ts`):

| Setting                           | Type     | Default | Description                                            |
| --------------------------------- | -------- | ------- | ------------------------------------------------------ |
| `windowSize`                      | `number` | `20`    | Maximum number of messages to include (count limit)    |
| `advanced.defaultMaxOutputTokens` | `number` | `8192`  | Reserved output budget, subtracted from context window |

`windowSize` is configurable in-app via **Settings → Chat Interface → Message Window Size**.

---

## Token Budget Formula

```
contextWindow         ← from llmConfigManager or dynamic detection (OpenRouter API / Ollama API)
reservedOutput        = defaultMaxOutputTokens + 100   (safety buffer)
safeInputTokenLimit   = contextWindow − reservedOutput

tokenLimit (in selectMessagesWithinContext)
  = max(1024, safeInputTokenLimit − systemPromptTokens − toolsTokens − pinnedMessageTokens)
```

If no model info is available, falls back to `contextWindow = 64K`.

---

## Provider-Specific Behaviour

| Provider                                    | Tool-chain integrity check            | Batching cap                    |
| ------------------------------------------- | ------------------------------------- | ------------------------------- |
| OpenAI, Anthropic, Gemini, Groq, OpenRouter | ✅ Yes — `removeIncompleteToolChains` | 4 tool calls/turn (Gemini: 100) |
| Ollama, Fireworks, Cerebras                 | ❌ No — break on token overflow       | 4 tool calls/turn               |

Gemini uses `maxToolCallsPerMessage: 100` to prevent splitting turns with a `thinkingSignature`.

---

## Known Limitations

1. **Drop-only, no compression** — messages outside the window are discarded entirely, not summarised.
2. **Single pinned anchor** — only the first user message is anchored; intermediate goal/summary messages can be dropped.
3. **No provider-aware budgeting** — Anthropic prompt caching and Gemini's 2M token window are not exploited differently.
4. **Count-based window is coarse** — a window of 20 may include 5 large tool results or 20 short user turns; token budget is the real guard.
5. **Estimation only** — `estimateTokensBPE` uses `cl100k_base` for all providers; actual token counts differ for Anthropic, Gemini, and Ollama.

---

## Proposed Design: Async Compact-Based Context Management

### Summary of Proposal

- Context length 설정: 모델 정보 우선, 없으면 8K/16K/32K/64K/128K/256K 슬라이더 fallback
- 입력 토큰이 컨텍스트 윈도우의 **90%를 초과**하면 비동기 compact 실행
- compact 진행 중에도 streamChat은 정상 처리
- compact 완료 후: `[pinned, compacted_summary, recent_messages]` 구조로 재구성
- compact 완료 전 100% 도달 시: compact 완료를 대기

---

### Assessment

전반적으로 옳은 방향입니다. 특히 **prompt cache와의 정렬**이 핵심 장점입니다. Anthropic의 `cache_control` breakpoint는 "prefix가 안정적일 때" 캐시 히트를 냅니다. compact된 summary가 최신 메시지 앞에 고정되면 캐시 히트율이 크게 올라갑니다.

다만 구체적인 구현 단계에서 해결해야 할 문제들이 있습니다.

---

### Issues to Address

#### 1. 100% 대기는 UX 블로킹

> "compact가 완료되지 않았는데 100%가 되면 compact를 기다림"

이 경우 사용자는 응답 없음 상태를 봅니다. 완전한 블로킹보다 **graceful fallback** 이 필요합니다.

**권고:** compact 대기 중 100% 도달 시, 기존 `selectMessagesWithinContext` 슬라이딩 윈도우를 임시 fallback으로 사용하고, compact 완료 후 다음 요청부터 적용합니다.

#### 2. compact 트리거를 90%보다 낮게

90% 트리거는 compact가 완료되기 전에 100%에 도달할 여유가 거의 없습니다. compact 호출 자체도 LLM 요청이기 때문에 수 초에서 수십 초가 걸릴 수 있습니다.

**권고:** 트리거를 **75~80%** 로 낮춥니다. 이 구간에서 compact가 완료되면, 100% 블로킹 상황이 사실상 발생하지 않습니다.

#### 3. re-compact 문제 (lossy × lossy)

compact 결과 자체가 메시지 스택에 들어가면, 다음 compact 사이클에서 그 summary가 또 compact의 대상이 됩니다. 요약의 요약은 품질이 급격히 저하됩니다.

**권고:** compact된 메시지에 `compacted: true` 플래그를 추가하고, compact 대상 선택 시 해당 메시지는 항상 제외합니다. 즉, compact는 `compacted: false`인 메시지에만 적용합니다.

```typescript
// Message 모델에 추가 필요
interface Message {
  // ... 기존 필드
  compacted?: boolean; // compact summary임을 표시
}
```

#### 4. Tool chain / thinking signature 무결성

현재 `batchToolCallsInMessages`는 `thinkingSignature`가 있는 메시지의 분할을 금지합니다. compact도 동일한 제약이 필요합니다.

compact LLM에 보낼 "compaction target messages"를 선택할 때:

- `tool_calls`가 있는 assistant 메시지와 그 tool result들은 **반드시 하나의 묶음으로** compact해야 합니다 (중간에 자르면 Anthropic/Gemini API 오류 발생).
- `thinkingSignature`가 있는 turn 전체를 atomic하게 처리합니다.

**권고:** compact 경계를 항상 완전한 turn 단위(user → assistant → [tool → tool_result]\*) 로 정렬합니다.

#### 5. compact에 사용할 모델

현재 사용자의 primary model로 compact를 실행하면:

- Claude Opus, GPT-4o 등 고비용 모델 사용 시 compact 비용이 큼
- compact latency가 primary stream 지연으로 이어질 수 있음

**권고:** compact 전용 모델을 별도로 설정할 수 있게 합니다 (기본값: 같은 provider의 가장 저렴한 모델, 또는 인냉할 수 있게 하기 settings에 추가). compact는 요약 품질보다 속도/비용이 중요합니다.

#### 6. Anthropic prompt cache breakpoint 배치

Anthropic에서 최대 캐시 히트를 얻으려면 `cache_control: { type: "ephemeral" }` breakpoint를 compact summary 메시지 바로 뒤에 배치해야 합니다. 현재 `AnthropicService`는 이 필드를 취급하지 않습니다.

**권고:** compact summary 메시지에 provider-specific metadata 필드를 추가하고, `AnthropicService.convertToAnthropicMessages()`에서 해당 메시지 뒤에 `cache_control` breakpoint를 삽입합니다.

---

### Revised Data Flow

```
매 streamChat 호출 전:

[토큰 측정]
  │
  ├── < 75%  → 정상 진행
  │
  ├── 75~99%  → compact 비동기 시작 (이미 진행 중이면 skip)
  │             현재 요청은 기존 스택으로 정상 처리
  │
  └── ≥ 100% → compact 완료 여부 확인
                ├── 완료됨 → 재구성된 스택 사용
                └── 미완    → 슬라이딩 윈도우 fallback 사용 (블로킹 없음)


compact 완료 후 메시지 스택 재구성:

  [system]                         ← 항상 고정
  [pinned_first_user]              ← 항상 고정 (현재 구조 유지)
  [compacted_summary, compacted:true]  ← compact 결과, breakpoint 배치 지점
  [recent_messages...]             ← compact에 포함되지 않은 최신 메시지들
```

---

### Context Length Settings UI

제안한 8K → 256K fallback 슬라이더는 타당합니다. 다만 두 가지를 추가합니다:

1. 모델 정보가 있을 경우 슬라이더 대신 **모델 정보 기반 자동값 표시** + override 허용
2. 현재 `getContextWindow()`는 async (OpenRouter API 조회)이므로, UI에서 로딩 상태 처리 필요. 캐시 히트 시 즉시 표시, miss 시 fallback 먼저 보여주고 나중에 갱신하는 구조가 적합합니다.

---

---

## Prompt Cache Analysis: Dynamic Elements vs. Stable Prefix

### Current System Prompt Assembly Order

`build_system_prompt()` in `src-tauri/src/agent/llm/prompt.rs` 기준:

```
[1] Agent Identity & Strategy        ← agent_config.system_prompt
[2] Workspace Instructions           ← agents.md / CLAUDE.md / soul.md (파일 내용)
[3] Session Context                  ← session.metadata.name
[4] ContextRegistry providers        ← registry.build_context() 결과
    ├── TimeLocationContextProvider  (priority: 5, 가장 먼저)
    └── (기타 providers, priority 순)
[5] Service Contexts                 ← proxy.get_service_contexts() 결과
    ├── bootstrap   (플랫폼 정보)
    ├── planning    (현재 goal, todo list)
    ├── browser     (현재 URL, 세션)
    ├── mcp_manager (연결된 MCP 서버 목록)
    ├── assistant   (에이전트 설명)
    └── skills      (사용 가능한 skills)
```

### Volatility Map

| 섹션                                           | 위치     | 변경 주기                 | Cache 관점         |
| ---------------------------------------------- | -------- | ------------------------- | ------------------ |
| [1] Agent Identity                             | 앞       | 에이전트 편집 시          | ✅ Stable          |
| [2] Workspace Instructions                     | 앞       | 파일 변경 시              | ✅ Stable          |
| [3] Session Context (name)                     | 앞       | 세션당 1회                | ✅ Stable          |
| [4] **TimeLocation** (priority 5, **첫 번째**) | **중간** | **매 초** (HH:MM:SS 포함) | 🔴 **Kills cache** |
| [5] planning                                   | 뒤       | 매 tool call              | 🔴 Volatile        |
| [5] browser                                    | 뒤       | 매 navigation             | 🔴 Volatile        |
| [5] bootstrap                                  | 뒤       | 세션당 1회                | ✅ Stable          |
| [5] mcp_manager                                | 뒤       | MCP 연결 변경 시          | 🟡 Semi-stable     |
| [5] skills                                     | 뒤       | skills 디렉토리 변경 시   | 🟡 Semi-stable     |

### 핵심 문제

**Anthropic prompt caching은 "가장 긴 stable prefix"에 breakpoint를 명시해야 작동합니다.**

현재 구조에서 stable한 섹션은 [1], [2], [3] — 보통 수천~수만 토큰의 agent identity + workspace instructions입니다. 그런데 [4]의 `TimeLocationContextProvider`가 **초 단위 타임스탬프**를 포함하므로 매 요청마다 prefix가 달라집니다.

```
Current effective cache boundary:
  [1][2][3]  ← stable (cache 가능한 구간)
       ↓
  [4] TimeLocation "Current Time: 14:23:07"  ← 1초마다 변경
                                               ← 실질적 cache prefix 경계
  [5] planning / browser ...
```

결과: **섹션 [1][2][3]을 포함한 모든 내용이 매 요청 cache miss** — `cache_control` breakpoint가 없으므로 Anthropic SDK는 현재 아무것도 캐시하지 않습니다. 설령 breakpoint를 추가해도 [4]가 먼저 나오기 때문에 stable prefix 길이가 0에 가깝습니다.

### 필요한 구조 변경

Prompt cache 효과를 얻으려면 **stable → volatile 순서**가 보장되어야 하고, `cache_control` breakpoint를 stable 구간 끝에 배치해야 합니다:

```
# 목표 시스템 프롬프트 구조

[STATIC — cache_control breakpoint 여기까지]
  [1] Agent Identity
  [2] Workspace Instructions
  [3] Session Context (name)
  [4a] Semi-stable service contexts (bootstrap, mcp_manager, skills, assistant)
─── cache_control: { type: "ephemeral" } ───────────────
[VOLATILE — 매 요청 달라지는 부분]
  [4b] TimeLocation (시간 정보)
  [5]  Planning (현재 goal, todos)
  [5]  Browser (현재 URL)
```

### 수정 방향

#### 1. TimeLocation에서 초(seconds) 제거

가장 빠른 fix. `HH:MM` 수준을 유지하면 분당 1회로 cache miss 빈도가 줄지만, 근본 해결은 아닙니다.

```rust
// time_location.rs — 현재
let current_time = format!("{:02}:{:02}:{:02} {}", now.hour(), now.minute(), now.second(), ...);
// 수정 후 (초 제거)
let current_time = format!("{:02}:{:02} {}", now.hour(), now.minute(), ...);
```

#### 2. TimeLocation 섹션을 프롬프트 **맨 끝**으로 이동

`ContextProvider.priority()` 시스템을 활용하여 volatile providers는 높은 숫자(늦은 순서)로 배치합니다:

```
TimeLocationContextProvider::priority() → 현재: 5
                                         변경 후: 1000 (가장 마지막)
```

또는 `build_system_prompt()`에서 stable/volatile 그룹을 분리하여 순서를 명시적으로 제어합니다.

#### 3. `AnthropicService`에 `cache_control` breakpoint 추가

현재 `AnthropicService`는 `cache_control` 필드를 일절 사용하지 않습니다. Stable prefix 끝 (섹션 [1]~[4a] 사이) 또는 system prompt 전체에 breakpoint를 삽입해야 합니다.

```rust
// convertToAnthropicMessages 또는 doStreamChat에서
// system prompt를 stable / volatile 두 블록으로 분할하고
// stable 블록 끝에 cache_control 삽입
system: vec![
    TextBlockParam {
        text: stable_prefix,
        cache_control: Some(CacheControlEphemeral {}),  // ← breakpoint
    },
    TextBlockParam {
        text: volatile_suffix,
        cache_control: None,
    },
]
```

Anthropic은 system prompt를 `string` 대신 `array of TextBlockParam`으로도 받을 수 있어 이 분할이 가능합니다.

### Implementation Priority

| 우선순위 | 항목                                                                             | 근거                                    |
| -------- | -------------------------------------------------------------------------------- | --------------------------------------- |
| P0       | `TimeLocationContextProvider` priority를 1000으로 변경 → 프롬프트 맨 끝으로 이동 | 현재 cache를 **완전히 무효화**하는 주범 |
| P0       | `TimeLocation` 시간 포맷에서 초(seconds) 제거                                    | P0 위의 보완책, 단독으로도 효과 있음    |
| P0       | `Message.compacted` 플래그 + re-compact 방지                                     | compact 루프 방지                       |
| P0       | compact 경계 turn-alignment (tool chain 무결성)                                  | Anthropic/Gemini API 400 방지           |
| P1       | `AnthropicService`에 stable/volatile 분할 + `cache_control` breakpoint 삽입      | 실제 cache hit 발생 조건                |
| P1       | compact 트리거를 75~80%로 조정                                                   | 100% hard blocking 방지                 |
| P1       | 100% fallback을 슬라이딩 윈도우로 (비블로킹)                                     | UX 보호                                 |
| P2       | semi-stable service contexts (bootstrap, mcp_manager)를 stable 그룹으로 분류     | cache 구간 극대화                       |
| P2       | compact 전용 모델 설정                                                           | 비용/지연 최적화                        |
| P3       | Context length 설정 UI (모델 자동감지 + 슬라이더 override)                       | 사용자 제어권                           |
