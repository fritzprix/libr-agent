# Reasoning Mode Implementation Status

## Overview

LibrAgent supports reasoning modes across multiple AI providers using **dynamic model capability detection** instead of hardcoded patterns. This ensures maintainability and extensibility as new models are released.

## Architecture

### Dynamic Capability Detection

Instead of hardcoding model patterns (e.g., `/^o[134](-|$)/` for OpenAI), the system uses a multi-tier approach:

1. **OpenRouter Metadata API** (Primary): Queries OpenRouter's model metadata for reasoning support
2. **Provider-Specific APIs** (Secondary): For Ollama, queries `/api/show` endpoint
3. **Minimal Fallback Patterns** (Tertiary): Only for most common model families
4. **24-hour Caching**: Avoids repeated API calls

**Key Function**: `supportsThinking(modelName, provider, options)`

- Location: `src/lib/ai-service/model-capabilities.ts`
- Returns: `Promise<boolean>`
- Used by: All AI services (Ollama, Anthropic, OpenAI, etc.)

### Benefits

✅ **No Hardcoding**: New models automatically detected via API
✅ **Unified Source**: OpenRouter serves as metadata database
✅ **Fallback Safety**: Works even if APIs fail
✅ **Performance**: 24-hour cache minimizes API calls
✅ **Extensibility**: Easy to add new providers

## Provider Support Matrix

| Provider      | Parameter Name      | Values                                   | Models                   | Status            |
| ------------- | ------------------- | ---------------------------------------- | ------------------------ | ----------------- |
| **Ollama**    | `think`             | `boolean` or `'low'`/`'medium'`/`'high'` | qwen3, deepseek-r1, etc. | ✅ Implemented    |
| **Anthropic** | `extended_thinking` | `boolean`                                | Claude 3.5+, Claude 4    | ✅ Implemented    |
| **OpenAI**    | `reasoning_effort`  | `'low'`/`'medium'`/`'high'`              | o1, o3, o4 series        | ✅ Implemented    |
| **Gemini**    | `thinkingBudget`    | TBD                                      | Gemini 2.5 Pro/Flash     | ⏳ To be verified |

## Implementation Details

### 1. Ollama Service (`ollama.ts`)

**Lines 177-230**: Think parameter preparation and fallback logic

```typescript
// Prepare reasoning parameter based on config
// Ollama expects: think: true | 'low' | 'medium' | 'high'
let thinkParam: boolean | 'low' | 'medium' | 'high' | undefined;
if (config.enableReasoning) {
  // Check if model actually supports thinking via API
  const modelSupportsThinking = await supportsThinking(
    model,
    AIServiceProvider.Ollama,
    { apiBase: this.host },
  );

  if (modelSupportsThinking) {
    thinkParam = config.reasoningEffort || true;
    logger.info('Ollama thinking mode enabled', {
      model,
      thinkParam,
      detectedViaAPI: modelSupportsThinking,
    });
  } else {
    logger.debug('Model may not support thinking, but will try anyway', {
      model,
    });
    // Still send the parameter - Ollama will ignore if not supported
    thinkParam = config.reasoningEffort || true;
  }
}

// API call with automatic retry and fallback
try {
  stream = await this.ollamaClient.chat({
    model: modelName,
    messages: ollamaMessages,
    stream: true,
    options: { ...defaultOptions, ...customOptions },
    ...(thinkParam !== undefined && { think: thinkParam }),
    tools: tools as Tool[],
  });
} catch (error) {
  // Fallback: Try with boolean true if granular levels not supported
  if (thinkParam !== true) {
    stream = await this.ollamaClient.chat({
      /* ...same params... */
      think: true,
    });
  }
}
```

**Key Features**:

- ✅ Dynamic API-based capability detection via `/api/show`
- ✅ Automatic fallback from granular levels to boolean
- ✅ Thinking field extraction and streaming
- ✅ Compatible with qwen3, deepseek-r1, and future models
- ✅ Handles empty content chunks (thinking-only)

---

```typescript
const message = chunk.message as {
  content?: string;
  thinking?: string; // Ollama reasoning mode
  tool_calls?: Array<...>;
};

// Extract thinking content
if (message.thinking && typeof message.thinking === 'string') {
  result.thinking = message.thinking;
  logger.debug('Thinking extracted from chunk', {
    thinkingLength: message.thinking.length,
    chunkIndex: chunkIndex++,
  });
}

// Return if we have meaningful data
if (result.content || result.thinking || result.tool_calls) {
  return JSON.stringify(result);
}
```

**Key Features**:

- ✅ Automatic fallback from granular levels to boolean
- ✅ Thinking field extraction and streaming
- ✅ Compatible with qwen3, deepseek-r1 models
- ✅ Handles empty content chunks (thinking-only)

---

### 2. Anthropic Service (`anthropic.ts`)

**Lines 219-228**: Extended thinking for Claude 3.5+

````typescript
### 2. Anthropic Service (`anthropic.ts`)

**Lines 13**: Import dynamic capability detection

```typescript
import { supportsThinking } from './model-capabilities';
````

**Lines 212-230**: Dynamic model capability check

```typescript
// Check if model supports extended thinking via dynamic capability detection
const model = options.modelName || this.getDefaultModel();
let extendedThinking: boolean | undefined;
if (config.enableReasoning) {
  const modelSupportsThinking = await supportsThinking(
    model,
    AIServiceProvider.Anthropic,
  );
  if (modelSupportsThinking) {
    extendedThinking = true;
    logger.info('Anthropic extended thinking enabled', { model });
  } else {
    logger.debug(
      'Model does not support extended thinking, ignoring enableReasoning flag',
      { model },
    );
  }
}

const stream = this.anthropic.messages.stream(
  {
    model: model,
    max_tokens: config.maxTokens!,
    messages: anthropicMessages,
    system: options.systemPrompt,
    ...(extendedThinking && { extended_thinking: extendedThinking }),
    tools: tools as AnthropicTool[],
  },
  { signal: this.getAbortSignal() },
);
```

**Key Features**:

- ✅ No hardcoded regex patterns
- ✅ Dynamic API-based detection
- ✅ Graceful fallback if model doesn't support reasoning
- ✅ Applied to any Claude model that supports extended_thinking

---

### 3. OpenAI Service (`openai.ts`)

**Lines 9**: Import dynamic capability detection

```typescript
import { supportsThinking } from './model-capabilities';
```

**Lines 195-217**: Dynamic reasoning effort preparation

```typescript
// Prepare reasoning_effort for reasoning models
// Check model capability dynamically instead of hardcoded patterns
let reasoningEffort: 'low' | 'medium' | 'high' | undefined;
if (config.enableReasoning && config.reasoningEffort) {
  const modelSupportsThinking = await supportsThinking(modelName, provider);
  if (modelSupportsThinking) {
    reasoningEffort = config.reasoningEffort;
    logger.info('OpenAI reasoning mode enabled', {
      model: modelName,
      reasoning_effort: reasoningEffort,
    });
  } else {
    logger.debug(
      'Model does not support reasoning, ignoring enableReasoning flag',
      { model: modelName },
    );
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
    },
    { signal: this.getAbortSignal() },
  ),
);
```

**Key Features**:

- ✅ No regex patterns like `/^o[134](-|$)/`
- ✅ Works with o1, o3, o4 and future reasoning models
- ✅ Automatic detection via OpenRouter metadata
- ✅ Granular effort levels: `'low'`, `'medium'`, `'high'`

---

### 1. Ollama Service (`ollama.ts`)

**Lines 15**: Import dynamic capability detection

```typescript
import { supportsThinking } from './model-capabilities';
```

**Lines 169-195**: Enhanced thinking parameter with API check

````

**Key Features**:
- ✅ Automatic model version detection
- ✅ Boolean-only parameter (no granular levels)
- ✅ Applied to Claude 3.5, Claude 4 series
- ✅ Integrated with existing streaming pipeline

---

### 3. OpenAI Service (`openai.ts`)

**Lines 195-210**: Reasoning effort for o-series models

```typescript
// Prepare reasoning_effort for o1/o3/o4 models
let reasoningEffort: 'low' | 'medium' | 'high' | undefined;
if (config.enableReasoning && config.reasoningEffort) {
  const isReasoningModel = /^o[134](-|$)/.test(modelName);
  if (isReasoningModel) {
    reasoningEffort = config.reasoningEffort;
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
    },
    { signal: this.getAbortSignal() },
  ),
);
````

**Key Features**:

- ✅ Granular effort levels: `'low'`, `'medium'`, `'high'`
- ✅ Automatic model detection (o1, o3, o4 series)
- ✅ Integrated with existing retry logic
- ✅ Supports both streaming and non-streaming modes

---

## Configuration Interface

**File**: `src/lib/ai-service/types.ts`

```typescript
export interface AIServiceConfig {
  // ... existing fields ...

  /** Enable reasoning/thinking mode for supported models */
  enableReasoning?: boolean;

  /** Reasoning effort level (provider-specific interpretation) */
  reasoningEffort?: 'low' | 'medium' | 'high';
}
```

---

## API Key Management

**File**: `src/lib/ai-service/factory.ts`

```typescript
static getService(provider, apiKey, config) {
  // Check if provider requires API key
  const providerInfo = configManager.getProviders()[provider];
  const requiresApiKey = providerInfo?.requiresApiKey ?? true;

  // Use dummy key for local providers (Ollama)
  const effectiveApiKey =
    !requiresApiKey && (!apiKey || apiKey.trim().length === 0)
      ? `${provider}-local`
      : apiKey;

  // ... create service instance ...
}
```

**Configuration**: `src/config/llm-config.json`

```json
{
  "providers": {
    "ollama": {
      "name": "Ollama",
      "baseUrl": "http://127.0.0.1:11434",
      "requiresApiKey": false, // <-- Key field
      "models": {
        /* ... */
      }
    }
  }
}
```

---

## Testing

### Manual Testing (Browser Environment)

1. **Ollama**:
   - Select model: `qwen3:8b` or `deepseek-r1`
   - Enable reasoning toggle in ChatHeader
   - Send message: "What is 15 \* 24? Show your reasoning."
   - Verify thinking stream appears

2. **Anthropic**:
   - Select model: `claude-opus-4-20250514`
   - Enable reasoning toggle
   - Send complex reasoning task
   - Verify extended thinking output

3. **OpenAI**:
   - Select model: `o4-mini` or `o3`
   - Enable reasoning toggle with effort level
   - Send multi-step problem
   - Verify reasoning output

### Automated Testing (Node.js - Limited)

⚠️ **Note**: Direct Node.js testing fails due to Tauri dependencies (window object).
Testing must be done in actual Tauri app environment.

---

## Known Issues & Limitations

### Ollama

- ⚠️ **qwen3 models**: Only support `think: true` (boolean), not granular levels
- ✅ **Workaround**: Automatic fallback implemented
- ℹ️ **Thinking chunks**: Small (4-6 chars each), requires aggregation

### Anthropic

- ℹ️ **Parameter**: Boolean only, no granular control
- ℹ️ **Model support**: Claude 3.5+ and Claude 4 series
- ℹ️ **Detection**: Regex-based model version check

### OpenAI

- ℹ️ **Model support**: Only o1, o3, o4 series
- ℹ️ **Detection**: Regex pattern `/^o[134](-|$)/`
- ℹ️ **Non-streaming**: Some reasoning models may not support streaming

### General

- ⚠️ **API Key validation**: Empty keys rejected by base service
- ✅ **Solution**: Factory provides dummy keys for local providers
- ⚠️ **Cost**: Reasoning mode significantly increases token usage

---

## Future Work

1. **Gemini Integration**: Verify `thinkingBudget` parameter implementation
2. **UI Components**: Add reasoning toggle to ChatHeader with Brain icon
3. **Context State**: Add `reasoningEnabled` state to ChatContext
4. **Cost Warnings**: Display token usage warnings for reasoning mode
5. **Model Detection**: Improve heuristics for new model series
6. **Unified API**: Consider abstracting provider-specific differences

---

## References

- **Ollama API**: `think` parameter in chat endpoint
- **Anthropic SDK**: `extended_thinking` in message creation
- **OpenAI SDK**: `reasoning_effort` in chat completions
- **LLM Config**: `/src/config/llm-config.json` - `supportReasoning` flag
- **Model Capabilities**: `/src/lib/ai-service/model-capabilities.ts`
