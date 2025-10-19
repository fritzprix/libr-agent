/**
 * Defines a standardized structure for classified AI service errors.
 */
export interface ErrorClassification {
  /** A user-friendly message to display in the UI. */
  displayMessage: string;
  /** The type of the error (e.g., 'NETWORK_ERROR', 'AUTHENTICATION_ERROR'). */
  type: string;
  /** Indicates whether the operation can be retried. */
  recoverable: boolean;
  /** Contains detailed information about the error for debugging. */
  details: {
    /** The original error object that was thrown. */
    originalError: unknown;
    /** A specific error code, if available. */
    errorCode?: string;
    /** The ISO timestamp of when the error occurred. */
    timestamp: string;
    /** Any additional context provided when the error was classified. */
    context?: Record<string, unknown>;
  };
}

/**
 * Classifies a given error from an AI service into a standardized format.
 * It inspects the error message to determine the type of error (e.g., network, authentication).
 *
 * @param error The error object to classify.
 * @param context Optional additional context about the error.
 * @returns An `ErrorClassification` object with details about the error.
 */
export const classifyAIServiceError = (
  error: unknown,
  context?: Record<string, unknown>,
): ErrorClassification => {
  const timestamp = new Date().toISOString();

  // MALFORMED_FUNCTION_CALL error
  if (
    error instanceof Error &&
    error.message.includes('MALFORMED_FUNCTION_CALL')
  ) {
    return {
      displayMessage:
        'I encountered an issue while trying to use tools. Let me try again without tools.',
      type: 'MALFORMED_FUNCTION_CALL',
      recoverable: true,
      details: {
        originalError: error,
        errorCode: 'MALFORMED_FUNCTION_CALL',
        timestamp,
        context,
      },
    };
  }

  // JSON parsing error
  if (
    error instanceof Error &&
    error.message.includes('Incomplete JSON segment')
  ) {
    return {
      displayMessage:
        'I had trouble processing the response. Please try again.',
      type: 'JSON_PARSING_ERROR',
      recoverable: true,
      details: {
        originalError: error,
        errorCode: 'INCOMPLETE_JSON',
        timestamp,
        context,
      },
    };
  }

  // Network error
  if (
    error instanceof Error &&
    (error.message.includes('network') ||
      error.message.includes('fetch') ||
      error.message.includes('Failed to fetch'))
  ) {
    return {
      displayMessage:
        'Network connection issue. Please check your connection and try again.',
      type: 'NETWORK_ERROR',
      recoverable: true,
      details: {
        originalError: error,
        errorCode: 'NETWORK_FAILURE',
        timestamp,
        context,
      },
    };
  }

  // API key related error
  if (
    error instanceof Error &&
    (error.message.includes('API key') ||
      error.message.includes('authentication') ||
      error.message.includes('401') ||
      error.message.includes('403'))
  ) {
    return {
      displayMessage:
        'Authentication issue. Please check your API key configuration.',
      type: 'AUTHENTICATION_ERROR',
      recoverable: false,
      details: {
        originalError: error,
        errorCode: 'AUTH_FAILURE',
        timestamp,
        context,
      },
    };
  }

  // Rate limit error
  if (
    error instanceof Error &&
    (error.message.includes('rate limit') ||
      error.message.includes('429') ||
      error.message.includes('quota'))
  ) {
    return {
      displayMessage:
        'Rate limit exceeded. Please wait a moment and try again.',
      type: 'RATE_LIMIT_ERROR',
      recoverable: true,
      details: {
        originalError: error,
        errorCode: 'RATE_LIMIT',
        timestamp,
        context,
      },
    };
  }

  // Other unknown errors
  return {
    displayMessage: 'Something went wrong. Please try again.',
    type: 'UNKNOWN_ERROR',
    recoverable: true,
    details: {
      originalError: error,
      errorCode: 'UNKNOWN',
      timestamp,
      context,
    },
  };
};

/**
 * Creates an error message for AI service failures.
 * @param messageId The ID for the error message.
 * @param sessionId The session ID the error occurred in.
 * @param threadId The thread ID the error occurred in (defaults to sessionId).
 * @param error The error object to classify and include in the message.
 * @param context Optional additional context about the error.
 * @returns A message object formatted as an error.
 */
export const createErrorMessage = (
  messageId: string,
  sessionId: string,
  threadId: string,
  error: unknown,
  context?: Record<string, unknown>,
) => {
  const errorClassification = classifyAIServiceError(error, context);

  return {
    id: messageId,
    content: [],
    role: 'assistant' as const,
    sessionId,
    threadId: threadId || sessionId, // Default to top thread
    isStreaming: false,
    error: errorClassification,
  };
};
