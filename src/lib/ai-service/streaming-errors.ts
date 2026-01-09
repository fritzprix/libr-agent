/**
 * Standardized Error Handling for Streaming Operations
 *
 * This module provides a unified error taxonomy and handling for
 * streaming AI service interactions across all providers.
 */

import type { AIServiceProvider } from './types';
import type { Logger } from './ollama-core';

/**
 * Standardized error codes for streaming and function parsing
 * Format: {CATEGORY}_{SPECIFIC}
 */
export enum StreamingErrorCode {
  // Generic stream errors
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
 * Context information for streaming errors
 */
export interface StreamingErrorContext {
  toolId?: string;
  toolName?: string;
  chunkIndex?: number;
  partialData?: string;
  timestamp: number;
  [key: string]: unknown;
}

/**
 * Structured error object for streaming errors
 */
export interface StreamingError {
  code: StreamingErrorCode;
  message: string;
  provider: AIServiceProvider;
  context: StreamingErrorContext;
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
  context: Partial<StreamingErrorContext> = {},
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

/**
 * Centralized error handler for streaming errors
 */
export class StreamingErrorHandler {
  private errorCounts = new Map<StreamingErrorCode, number>();
  private logger: Logger;

  constructor(logger: Logger) {
    this.logger = logger;
  }

  /**
   * Handle a streaming error with logging and tracking
   */
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

  /**
   * Get error statistics
   */
  getErrorStats(): Record<string, number> {
    return Object.fromEntries(this.errorCounts);
  }

  /**
   * Reset error counts
   */
  reset(): void {
    this.errorCounts.clear();
  }
}
