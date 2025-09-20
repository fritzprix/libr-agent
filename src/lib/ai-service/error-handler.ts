export interface ErrorClassification {
  displayMessage: string;
  type: string;
  recoverable: boolean;
  details: {
    originalError: unknown;
    errorCode?: string;
    timestamp: string;
    context?: Record<string, unknown>;
  };
}

export const classifyAIServiceError = (
  error: unknown,
  context?: Record<string, unknown>,
): ErrorClassification => {
  const timestamp = new Date().toISOString();

  // MALFORMED_FUNCTION_CALL 에러
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

  // JSON 파싱 에러
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

  // 네트워크 에러
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

  // API 키 관련 에러
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

  // Rate limit 에러
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

  // 기타 알 수 없는 에러
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

export const createErrorMessage = (
  messageId: string,
  sessionId: string,
  error: unknown,
  context?: Record<string, unknown>,
) => {
  const errorClassification = classifyAIServiceError(error, context);

  return {
    id: messageId,
    content: [],
    role: 'assistant' as const,
    sessionId,
    isStreaming: false,
    error: errorClassification,
  };
};