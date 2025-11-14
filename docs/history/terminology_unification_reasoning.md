# 용어 통일: thinking → reasoning

**날짜**: 2025-11-14  
**작업자**: AI Agent  
**브랜치**: dev/0.2.0

## 결정 사항

AI 모델의 사고 과정 관련 기능의 용어를 **`thinking`에서 `reasoning`으로 통일**

## 근거

1. **API 표준 용어**: 대부분의 Provider가 `reasoning` 용어 사용
   - OpenRouter: `supported_parameters: ["reasoning"]`
   - OpenAI: `reasoning_effort` 파라미터
   - Gemini (OpenAI 호환): `reasoning_effort` 지원
   - DeepSeek: `include_reasoning` 파라미터

2. **일관성**: Ollama만 `think` 사용, 나머지는 모두 `reasoning` 계열
   - Anthropic: `extended_thinking` (예외적으로 thinking 사용)
   - Ollama: `think` (내부적으로 reasoning 수행)

3. **명확성**: `reasoning`이 AI의 추론 과정을 더 명확하게 표현

## 변경 범위

### 1. 타입 정의 (`src/lib/ai-service/types.ts`)

```typescript
// Before
enableThinking?: boolean;
thinkingLevel?: 'low' | 'medium' | 'high';

// After
enableReasoning?: boolean;
reasoningEffort?: 'low' | 'medium' | 'high';
```

### 2. 함수명 (`src/lib/ai-service/model-capabilities.ts`)

```typescript
// Before
supportsThinking();
getRecommendedThinkingLevel();

// After
supportsReasoning();
getRecommendedReasoningLevel();
```

### 3. 상태 변수 (ChatContext, 향후 구현)

```typescript
// Before
thinkingEnabled;
canUseThinking;
toggleThinking();

// After
reasoningEnabled;
canUseReasoning;
toggleReasoning();
```

### 4. 문서

- `refactoring_20251114_thinking_mode.md` → `refactoring_20251114_reasoning_mode.md`
- 문서 내 모든 `thinking` → `reasoning` 자동 변경 (sed 명령 사용)

## Provider별 파라미터 매핑

| Provider  | Internal Name     | API Parameter       | 비고                    |
| --------- | ----------------- | ------------------- | ----------------------- |
| Ollama    | `enableReasoning` | `think`             | Ollama만 다른 이름 사용 |
| OpenAI    | `reasoningEffort` | `reasoning_effort`  | ✅ 직접 매핑            |
| Anthropic | `enableReasoning` | `extended_thinking` | thinking 용어 사용      |
| Gemini    | `reasoningEffort` | `thinkingBudget`    | 토큰 수로 변환          |

## 구현 시 주의사항

1. **Provider별 변환 로직 필요**:

   ```typescript
   // ollama.ts
   const ollamaParams = {
     think: config.enableReasoning
       ? mapReasoningEffort(config.reasoningEffort)
       : undefined,
   };

   // openai.ts
   const openaiParams = {
     reasoning_effort: config.enableReasoning
       ? config.reasoningEffort
       : undefined,
   };
   ```

2. **UI 표시**: 사용자에게는 "Reasoning Mode"로 표시
   - Brain 아이콘 tooltip: "Enable Reasoning Mode"
   - 비용 경고: "Reasoning uses more tokens"

3. **하위 호환성**: 기존 코드에 `thinking` 관련 필드 없으므로 영향 없음

## 완료 상태

- ✅ `types.ts` 타입 정의 업데이트
- ✅ `model-capabilities.ts` 함수명 변경
- ✅ 리팩토링 계획 문서 업데이트 및 파일명 변경
- ✅ TodoList 업데이트
- ⏳ Provider 서비스 구현 (다음 단계)
- ⏳ ChatContext 통합 (다음 단계)
- ⏳ UI 구현 (다음 단계)
