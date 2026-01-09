# Streaming & Function Parsing Enhancements

## Executive Summary

This document elaborates on potential improvements for streaming and function parsing across all AI providers (Ollama, OpenAI, Anthropic, Gemini). While the current implementation is **production-ready** with robust error handling, these enhancements would further improve reliability, observability, and developer experience.

---

## Table of Contents

1. [Improvement 1: Ollama Partial JSON Accumulation](#improvement-1-ollama-partial-json-accumulation)
2. [Improvement 2: OpenAI Validation Layer](#improvement-2-openai-validation-layer)
3. [Improvement 3: Standardized Error Codes](#improvement-3-standardized-error-codes)
4. [Improvement 4: Enhanced Observability](#improvement-4-enhanced-observability)
5. [Improvement 5: Retry & Circuit Breaker Patterns](#improvement-5-retry--circuit-breaker-patterns)
6. [Implementation Priority](#implementation-priority)

---

## Improvement 1: Ollama Partial JSON Accumulation

### Current State

**File:** `src/lib/ai-service/ollama-core.ts` (Lines 449-465)

```typescript
if (message.tool_calls && Array.isArray(message.tool_calls)) {
  result.tool_calls = message.tool_calls.map((tc) => {
    const callId = tc.id || generateToolCallId();
    const args =
      typeof tc.function.arguments === 'string'
        ? (tryParse<Record<string, unknown>>(tc.function.arguments) ?? {})
        : tc.function.arguments;

    const formatted = formatToolCall(callId, tc.function.name, args);
    return { ...formatted, type: 'function' as const };
  });
}
```

**Assumption:** Each chunk contains **complete** tool call JSON.

### Problem

Some Ollama models (especially large context models) may stream tool calls across **multiple chunks**, similar to Anthropic's behavior. Current implementation assumes complete JSON per chunk, which could cause:

1. **Silent failures** if JSON is split mid-stream
2. **Empty tool calls** if `tryParse` fails on partial JSON
3. **Lost tool invocations** leading to agent workflow failures

### Proposed Solution

Implement **Anthropic-style accumulator pattern** for Ollama:

```typescript
/**
 * Extended ToolCallAccumulator for Ollama streaming
 * Handles partial JSON accumulation across multiple chunks
 */
interface OllamaToolCallAccumulator {
  id: string;
  name: string;
  partialJson: string;
  index: number;
  yielded: boolean;
  lastChunkTime: number; // Track timing for timeout detection
}

// In processChunk function
const toolCallAccumulators = new Map<number, OllamaToolCallAccumulator>();
const MAX_ACCUMULATOR_AGE_MS = 30_000; // 30 seconds timeout

export function processChunk(
  chunk: unknown,
  logger: Logger = noopLogger,
  accumulators?: Map<number, OllamaToolCallAccumulator>,
): ProcessedChunk | null {
  // ... existing code ...

  if (message.tool_calls && Array.isArray(message.tool_calls)) {
    const processedToolCalls: Array<{
      id: string;
      type: 'function';
      function: { name: string; arguments: string };
    }> = [];

    for (const [idx, tc] of message.tool_calls.entries()) {
      const callId = tc.id || generateToolCallId();

      // Get or create accumulator
      let accumulator = accumulators?.get(idx);
      if (!accumulator) {
        accumulator = {
          id: callId,
          name: tc.function.name,
          partialJson: '',
          index: idx,
          yielded: false,
          lastChunkTime: Date.now(),
        };
        accumulators?.set(idx, accumulator);
      }

      // Check for timeout (stale accumulator)
      const age = Date.now() - accumulator.lastChunkTime;
      if (age > MAX_ACCUMULATOR_AGE_MS) {
        logger.warn('Tool call accumulator timeout, discarding', {
          id: accumulator.id,
          name: accumulator.name,
          ageMs: age,
        });
        accumulators?.delete(idx);
        continue;
      }

      // Update timestamp
      accumulator.lastChunkTime = Date.now();

      // Handle string arguments (potential partial JSON)
      if (typeof tc.function.arguments === 'string') {
        accumulator.partialJson += tc.function.arguments;

        // Buffer size limit
        if (accumulator.partialJson.length > 200_000) {
          logger.error('Tool call JSON exceeded buffer limit', {
            id: accumulator.id,
            name: accumulator.name,
            length: accumulator.partialJson.length,
          });
          accumulators?.delete(idx);
          continue;
        }

        // Attempt to parse accumulated JSON
        const trimmedJson = accumulator.partialJson.trim();
        try {
          const parsed = JSON.parse(trimmedJson) as Record<string, unknown>;

          // Success! Yield the tool call
          if (!accumulator.yielded) {
            const formatted = formatToolCall(callId, tc.function.name, parsed);
            processedToolCalls.push({
              ...formatted,
              type: 'function' as const,
            });
            accumulator.yielded = true;
            logger.info('Tool call successfully parsed from accumulated JSON', {
              id: callId,
              name: tc.function.name,
              jsonLength: trimmedJson.length,
            });
          }
        } catch (parseError) {
          // JSON still incomplete, continue accumulating
          logger.debug('JSON incomplete, waiting for more chunks', {
            id: callId,
            name: tc.function.name,
            currentLength: accumulator.partialJson.length,
          });
        }
      } else {
        // Already parsed object (complete)
        const formatted = formatToolCall(
          callId,
          tc.function.name,
          tc.function.arguments,
        );
        processedToolCalls.push({
          ...formatted,
          type: 'function' as const,
        });
        logger.debug('Tool call already parsed', {
          id: callId,
          name: tc.function.name,
        });
      }
    }

    if (processedToolCalls.length > 0) {
      result.tool_calls = processedToolCalls;
    }
  }

  return result;
}
```

**Usage in ollama.ts:**

```typescript
protected async *doStreamChat(
  messages: Message[],
  options: StreamChatOptions = {},
): AsyncGenerator<string, void, void> {
  // ... existing code ...

  const toolCallAccumulators = new Map<number, OllamaToolCallAccumulator>();

  for await (const chunk of stream) {
    if (this.getAbortSignal().aborted) {
      this.logger.debug('Stream aborted during iteration');
      break;
    }

    const processedChunk = processChunk(chunk, coreLogger, toolCallAccumulators);

    // ... rest of yielding logic ...
  }

  // Cleanup: Check for incomplete tool calls
  if (toolCallAccumulators.size > 0) {
    for (const [idx, accumulator] of toolCallAccumulators.entries()) {
      if (!accumulator.yielded) {
        logger.warn('Incomplete tool call at stream end', {
          id: accumulator.id,
          name: accumulator.name,
          partialJson: accumulator.partialJson.substring(0, 200),
        });
      }
    }
    toolCallAccumulators.clear();
  }
}
```

### Benefits

1. **Robustness:** Handles split JSON across chunks
2. **Safety:** Buffer limits prevent memory exhaustion
3. **Observability:** Clear logging for debugging
4. **Consistency:** Matches Anthropic's proven pattern

### Trade-offs

- **Complexity:** Additional state management
- **Memory:** Accumulator overhead (mitigated by size limits)
- **Latency:** Slight delay waiting for complete JSON

---

## Improvement 2: OpenAI Validation Layer

### Current State

**File:** `src/lib/ai-service/openai.ts` (Lines 261-268)

```typescript
if (chunk.choices[0]?.delta?.tool_calls) {
  yield JSON.stringify({
    tool_calls: chunk.choices[0].delta.tool_calls,
  });
} else if (chunk.choices[0]?.delta?.content) {
  yield JSON.stringify({
    content: chunk.choices[0]?.delta?.content || '',
  });
}
```

**Assumption:** OpenAI SDK **always** provides valid, complete tool calls.

### Problem

While the OpenAI SDK is highly reliable, **defensive programming** suggests validating external data:

1. **SDK bugs:** Edge cases in beta/experimental models
2. **Network corruption:** Rare but possible
3. **Type mismatches:** TypeScript types don't guarantee runtime validity

### Proposed Solution

Add **lightweight validation layer** without sacrificing performance:

```typescript
/**
 * Validates OpenAI tool call structure
 * @returns true if valid, false otherwise
 */
function validateOpenAIToolCall(toolCall: unknown): toolCall is {
  id: string;
  type: 'function';
  function: { name: string; arguments: string };
} {
  if (!toolCall || typeof toolCall !== 'object') return false;

  const tc = toolCall as Record<string, unknown>;

  return (
    typeof tc.id === 'string' &&
    tc.id.length > 0 &&
    tc.type === 'function' &&
    typeof tc.function === 'object' &&
    tc.function !== null &&
    typeof (tc.function as Record<string, unknown>).name === 'string' &&
    typeof (tc.function as Record<string, unknown>).arguments === 'string'
  );
}

/**
 * Sanitizes OpenAI tool calls with validation and normalization
 */
function sanitizeOpenAIToolCalls(
  toolCalls: unknown[],
  logger: Logger,
): Array<{
  id: string;
  type: 'function';
  function: { name: string; arguments: string };
}> {
  const validToolCalls: Array<{
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }> = [];

  for (const [idx, tc] of toolCalls.entries()) {
    if (!validateOpenAIToolCall(tc)) {
      logger.warn('Invalid tool call from OpenAI SDK', {
        index: idx,
        toolCall: tc,
        validation: {
          hasId: typeof (tc as Record<string, unknown>)?.id === 'string',
          hasType: (tc as Record<string, unknown>)?.type === 'function',
          hasFunction:
            typeof (tc as Record<string, unknown>)?.function === 'object',
        },
      });
      continue;
    }

    // Validate JSON arguments
    const funcObj = tc.function;
    try {
      JSON.parse(funcObj.arguments); // Ensure valid JSON
      validToolCalls.push(tc);
    } catch (jsonError) {
      logger.error('Tool call has invalid JSON arguments', {
        id: tc.id,
        name: funcObj.name,
        arguments: funcObj.arguments.substring(0, 200),
        error: jsonError,
      });

      // Attempt recovery: wrap in empty object
      logger.info('Attempting tool call recovery with empty arguments', {
        id: tc.id,
        name: funcObj.name,
      });
      validToolCalls.push({
        ...tc,
        function: {
          ...funcObj,
          arguments: '{}',
        },
      });
    }
  }

  return validToolCalls;
}
```

**Integration:**

```typescript
protected async *doStreamChat(
  messages: Message[],
  options: { /* ... */ } = {},
): AsyncGenerator<string, void, void> {
  // ... existing code ...

  for await (const chunk of completion) {
    // ... abort checks ...

    if (chunk.choices[0]?.delta?.tool_calls) {
      const rawToolCalls = chunk.choices[0].delta.tool_calls;
      const validToolCalls = sanitizeOpenAIToolCalls(
        Array.isArray(rawToolCalls) ? rawToolCalls : [rawToolCalls],
        logger,
      );

      if (validToolCalls.length > 0) {
        yield JSON.stringify({ tool_calls: validToolCalls });
      } else {
        logger.warn('All tool calls filtered out due to validation failures', {
          rawCount: Array.isArray(rawToolCalls) ? rawToolCalls.length : 1,
        });
      }
    } else if (chunk.choices[0]?.delta?.content) {
      yield JSON.stringify({
        content: chunk.choices[0]?.delta?.content || '',
      });
    }
  }
}
```

### Benefits

1. **Defense in depth:** Catches SDK bugs early
2. **Graceful degradation:** Recovers from invalid JSON
3. **Observability:** Detailed logging of failures
4. **Type safety:** Runtime validation matches TypeScript types

### Trade-offs

- **Performance:** Minimal (~1-2ms per tool call)
- **Complexity:** Additional validation code
- **False positives:** May filter valid edge cases (mitigated by logging)

---

## Improvement 3: Standardized Error Codes

### Current State

Errors are logged with **free-form messages**:

```typescript
logger.error('Failed to parse final tool call JSON', { error, partialJson });
logger.warn('Tool message missing tool_call_id');
logger.error('Tool call input exceeded maximum buffered length');
```

### Problem

1. **Inconsistent error handling:** Hard to programmatically detect error types
2. **Difficult monitoring:** No structured error metrics
3. **Poor debugging:** Text search required to find error patterns

### Proposed Solution

Define **standardized error taxonomy** with codes:

```typescript
/**
 * Standardized error codes for streaming and function parsing
 * Format: {PROVIDER}_{CATEGORY}_{SPECIFIC}
 */
export enum StreamingErrorCode {
  // Generic errors
  STREAM_ABORTED = 'STREAM_ABORTED',
  STREAM_TIMEOUT = 'STREAM_TIMEOUT',
  STREAM_CONNECTION_LOST = 'STREAM_CONNECTION_LOST',

  // JSON parsing errors
  JSON_PARSE_FAILED = 'JSON_PARSE_FAILED',
  JSON_INCOMPLETE = 'JSON_INCOMPLETE',
  JSON_BUFFER_OVERFLOW = 'JSON_BUFFER_OVERFLOW',
  JSON_EMPTY = 'JSON_EMPTY',

  // Tool call errors
  TOOL_CALL_INVALID_STRUCTURE = 'TOOL_CALL_INVALID_STRUCTURE',
  TOOL_CALL_MISSING_ID = 'TOOL_CALL_MISSING_ID',
  TOOL_CALL_MISSING_NAME = 'TOOL_CALL_MISSING_NAME',
  TOOL_CALL_MISSING_ARGUMENTS = 'TOOL_CALL_MISSING_ARGUMENTS',
  TOOL_CALL_ACCUMULATOR_TIMEOUT = 'TOOL_CALL_ACCUMULATOR_TIMEOUT',
  TOOL_CALL_DUPLICATE_YIELD = 'TOOL_CALL_DUPLICATE_YIELD',

  // Provider-specific
  OLLAMA_CHUNK_MALFORMED = 'OLLAMA_CHUNK_MALFORMED',
  ANTHROPIC_DELTA_INVALID = 'ANTHROPIC_DELTA_INVALID',
  OPENAI_SDK_VALIDATION_FAILED = 'OPENAI_SDK_VALIDATION_FAILED',
  GEMINI_FUNCTION_CALL_UNEXPECTED = 'GEMINI_FUNCTION_CALL_UNEXPECTED',
}

/**
 * Structured error object for streaming errors
 */
export interface StreamingError {
  code: StreamingErrorCode;
  message: string;
  provider: AIServiceProvider;
  context: {
    toolId?: string;
    toolName?: string;
    chunkIndex?: number;
    partialData?: string;
    timestamp: number;
  };
  recoverable: boolean;
  retryable: boolean;
}

/**
 * Create a standardized streaming error
 */
export function createStreamingError(
  code: StreamingErrorCode,
  provider: AIServiceProvider,
  message: string,
  context: Partial<StreamingError['context']> = {},
  recoverable = false,
  retryable = false,
): StreamingError {
  return {
    code,
    message,
    provider,
    context: {
      ...context,
      timestamp: Date.now(),
    },
    recoverable,
    retryable,
  };
}
```

**Usage Example (Anthropic):**

```typescript
if (accumulator.partialJson.length > MAX_PARTIAL_TOOL_INPUT_LENGTH) {
  const error = createStreamingError(
    StreamingErrorCode.JSON_BUFFER_OVERFLOW,
    AIServiceProvider.Anthropic,
    'Tool call JSON exceeded buffer limit',
    {
      toolId: accumulator.id,
      toolName: accumulator.name,
      partialData: accumulator.partialJson.substring(0, 100),
    },
    false, // not recoverable
    false, // not retryable
  );

  logger.error('Streaming error occurred', error);

  // Optional: Emit error metric
  this.emitErrorMetric(error);

  toolCallAccumulators.delete(chunk.index);
  continue;
}
```

**Error Handler:**

```typescript
/**
 * Centralized error handler for streaming errors
 */
export class StreamingErrorHandler {
  private errorCounts = new Map<StreamingErrorCode, number>();
  private logger: Logger;

  constructor(logger: Logger) {
    this.logger = logger;
  }

  handle(error: StreamingError): void {
    // Track error frequency
    const count = (this.errorCounts.get(error.code) || 0) + 1;
    this.errorCounts.set(error.code, count);

    // Log with severity based on recoverability
    if (error.recoverable) {
      this.logger.warn('Recoverable streaming error', {
        ...error,
        errorCount: count,
      });
    } else {
      this.logger.error('Non-recoverable streaming error', {
        ...error,
        errorCount: count,
      });
    }

    // Circuit breaker logic
    if (count > 10) {
      this.logger.error('Error threshold exceeded, circuit breaker triggered', {
        code: error.code,
        count,
      });
      throw new Error(`Circuit breaker: ${error.code} occurred ${count} times`);
    }
  }

  getErrorStats(): Record<string, number> {
    return Object.fromEntries(this.errorCounts);
  }

  reset(): void {
    this.errorCounts.clear();
  }
}
```

### Benefits

1. **Structured monitoring:** Error metrics by code
2. **Programmatic handling:** Switch/case on error codes
3. **Better debugging:** Filter logs by specific codes
4. **Circuit breaker:** Automatic failure detection

### Trade-offs

- **Verbosity:** More boilerplate code
- **Maintenance:** Error taxonomy must be kept current

---

## Improvement 4: Enhanced Observability

### Current State

Logging is **ad-hoc** across providers with varying detail levels.

### Proposed Solution

Implement **structured logging with context propagation**:

```typescript
/**
 * Streaming context for tracing and debugging
 */
export interface StreamingContext {
  sessionId: string;
  requestId: string;
  provider: AIServiceProvider;
  model: string;
  startTime: number;
  metrics: {
    chunksReceived: number;
    toolCallsDetected: number;
    errorsEncountered: number;
    bytesReceived: number;
  };
}

/**
 * Enhanced logger with streaming context
 */
export class StreamingLogger {
  private context: StreamingContext;
  private baseLogger: Logger;

  constructor(baseLogger: Logger, context: StreamingContext) {
    this.baseLogger = baseLogger;
    this.context = context;
  }

  debug(message: string, meta: Record<string, unknown> = {}): void {
    this.baseLogger.debug(message, {
      ...this.context,
      ...meta,
      elapsed: Date.now() - this.context.startTime,
    });
  }

  info(message: string, meta: Record<string, unknown> = {}): void {
    this.baseLogger.info(message, {
      ...this.context,
      ...meta,
      elapsed: Date.now() - this.context.startTime,
    });
  }

  warn(message: string, meta: Record<string, unknown> = {}): void {
    this.baseLogger.warn(message, {
      ...this.context,
      ...meta,
      elapsed: Date.now() - this.context.startTime,
    });
  }

  error(message: string, meta: Record<string, unknown> = {}): void {
    this.baseLogger.error(message, {
      ...this.context,
      ...meta,
      elapsed: Date.now() - this.context.startTime,
    });
  }

  updateMetrics(updates: Partial<StreamingContext['metrics']>): void {
    Object.assign(this.context.metrics, updates);
  }
}
```

**Usage:**

```typescript
protected async *doStreamChat(
  messages: Message[],
  options: StreamChatOptions = {},
): AsyncGenerator<string, void, void> {
  const streamingContext: StreamingContext = {
    sessionId: generateSessionId(),
    requestId: generateRequestId(),
    provider: this.getProvider(),
    model: options.modelName || 'default',
    startTime: Date.now(),
    metrics: {
      chunksReceived: 0,
      toolCallsDetected: 0,
      errorsEncountered: 0,
      bytesReceived: 0,
    },
  };

  const streamLogger = new StreamingLogger(logger, streamingContext);

  streamLogger.info('Stream initiated', {
    messageCount: messages.length,
    toolsAvailable: options.availableTools?.length || 0,
  });

  try {
    for await (const chunk of stream) {
      streamLogger.updateMetrics({
        chunksReceived: streamingContext.metrics.chunksReceived + 1,
        bytesReceived: streamingContext.metrics.bytesReceived + JSON.stringify(chunk).length,
      });

      const processedChunk = processChunk(chunk, streamLogger);

      if (processedChunk?.tool_calls) {
        streamLogger.updateMetrics({
          toolCallsDetected: streamingContext.metrics.toolCallsDetected + processedChunk.tool_calls.length,
        });
      }

      // ... yield logic ...
    }

    streamLogger.info('Stream completed successfully', {
      totalChunks: streamingContext.metrics.chunksReceived,
      totalToolCalls: streamingContext.metrics.toolCallsDetected,
      duration: Date.now() - streamingContext.startTime,
    });
  } catch (error) {
    streamLogger.updateMetrics({
      errorsEncountered: streamingContext.metrics.errorsEncountered + 1,
    });
    streamLogger.error('Stream failed', { error });
    throw error;
  }
}
```

### Benefits

1. **Request tracing:** Follow requests across components
2. **Performance analysis:** Per-request metrics
3. **Debugging:** Complete context in every log
4. **Analytics:** Aggregate metrics by provider/model

---

## Improvement 5: Retry & Circuit Breaker Patterns

### Current State

Basic retry logic exists in `BaseAIService.withRetry()`, but no **circuit breaker** for repeated failures.

### Proposed Solution

```typescript
/**
 * Circuit breaker states
 */
enum CircuitState {
  CLOSED = 'CLOSED', // Normal operation
  OPEN = 'OPEN', // Blocking requests
  HALF_OPEN = 'HALF_OPEN', // Testing recovery
}

/**
 * Circuit breaker for streaming operations
 */
export class StreamingCircuitBreaker {
  private state: CircuitState = CircuitState.CLOSED;
  private failureCount = 0;
  private lastFailureTime = 0;
  private successCount = 0;

  private readonly failureThreshold = 5;
  private readonly resetTimeout = 60_000; // 1 minute
  private readonly halfOpenSuccessThreshold = 2;

  async execute<T>(
    operation: () => Promise<T>,
    fallback?: () => T,
  ): Promise<T> {
    if (this.state === CircuitState.OPEN) {
      // Check if we should transition to HALF_OPEN
      if (Date.now() - this.lastFailureTime >= this.resetTimeout) {
        this.state = CircuitState.HALF_OPEN;
        this.successCount = 0;
        logger.info('Circuit breaker transitioning to HALF_OPEN');
      } else {
        // Still open, reject immediately
        logger.warn('Circuit breaker OPEN, rejecting request');
        if (fallback) return fallback();
        throw new Error('Circuit breaker is OPEN');
      }
    }

    try {
      const result = await operation();
      this.onSuccess();
      return result;
    } catch (error) {
      this.onFailure();
      if (fallback) return fallback();
      throw error;
    }
  }

  private onSuccess(): void {
    this.failureCount = 0;

    if (this.state === CircuitState.HALF_OPEN) {
      this.successCount++;
      if (this.successCount >= this.halfOpenSuccessThreshold) {
        this.state = CircuitState.CLOSED;
        logger.info('Circuit breaker CLOSED after recovery');
      }
    }
  }

  private onFailure(): void {
    this.failureCount++;
    this.lastFailureTime = Date.now();

    if (this.failureCount >= this.failureThreshold) {
      this.state = CircuitState.OPEN;
      logger.error('Circuit breaker OPEN due to repeated failures', {
        failureCount: this.failureCount,
      });
    }
  }

  getState(): CircuitState {
    return this.state;
  }

  reset(): void {
    this.state = CircuitState.CLOSED;
    this.failureCount = 0;
    this.successCount = 0;
  }
}
```

---

## Implementation Priority

### Phase 1: High Priority (Immediate Value)

1. **Standardized Error Codes** - Foundation for monitoring
2. **Ollama Partial JSON Accumulation** - Fixes potential production issue

### Phase 2: Medium Priority (Incremental Improvement)

3. **Enhanced Observability** - Better debugging experience
4. **OpenAI Validation Layer** - Defense in depth

### Phase 3: Low Priority (Advanced Features)

5. **Circuit Breaker Patterns** - Prevent cascade failures

---

## Conclusion

These improvements follow **industry best practices** for streaming APIs and error handling:

- **Defense in depth:** Multiple validation layers
- **Graceful degradation:** Fallbacks at every level
- **Observability:** Structured logging and metrics
- **Resilience:** Circuit breakers and retry logic

The current implementation is **production-ready**, but these enhancements would provide **enterprise-grade reliability** and **superior debugging experience**.
