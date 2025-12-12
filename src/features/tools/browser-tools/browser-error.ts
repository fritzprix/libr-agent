/**
 * Structured browser error types
 * These types match the Rust BrowserError enum for type-safe error handling
 */

/**
 * Browser error codes
 */
export enum BrowserErrorCode {
  SESSION_NOT_FOUND = 'SESSION_NOT_FOUND',
  SESSION_CLOSED = 'SESSION_CLOSED',
  WINDOW_NOT_FOUND = 'WINDOW_NOT_FOUND',
  ELEMENT_NOT_FOUND = 'ELEMENT_NOT_FOUND',
  ELEMENT_NOT_INTERACTABLE = 'ELEMENT_NOT_INTERACTABLE',
  NAVIGATION_FAILED = 'NAVIGATION_FAILED',
  SCRIPT_EXECUTION_FAILED = 'SCRIPT_EXECUTION_FAILED',
  TIMEOUT = 'TIMEOUT',
  LOCK_FAILED = 'LOCK_FAILED',
  INVALID_PARAMETER = 'INVALID_PARAMETER',
  UNKNOWN = 'UNKNOWN',
}

/**
 * Base interface for structured browser errors
 */
interface BaseBrowserError {
  code: BrowserErrorCode;
}

/**
 * Session not found error
 */
export interface SessionNotFoundError extends BaseBrowserError {
  code: BrowserErrorCode.SESSION_NOT_FOUND;
  context: {
    session_id: string;
  };
}

/**
 * Session closed error
 */
export interface SessionClosedError extends BaseBrowserError {
  code: BrowserErrorCode.SESSION_CLOSED;
  context: {
    session_id: string;
  };
}

/**
 * Window not found error
 */
export interface WindowNotFoundError extends BaseBrowserError {
  code: BrowserErrorCode.WINDOW_NOT_FOUND;
  context: {
    session_id: string;
  };
}

/**
 * Element not found error
 */
export interface ElementNotFoundError extends BaseBrowserError {
  code: BrowserErrorCode.ELEMENT_NOT_FOUND;
  context: {
    selector: string;
    session_id: string;
  };
}

/**
 * Element not interactable error
 */
export interface ElementNotInteractableError extends BaseBrowserError {
  code: BrowserErrorCode.ELEMENT_NOT_INTERACTABLE;
  context: {
    selector: string;
    reason: string;
    session_id: string;
  };
}

/**
 * Navigation failed error
 */
export interface NavigationFailedError extends BaseBrowserError {
  code: BrowserErrorCode.NAVIGATION_FAILED;
  context: {
    url: string;
    reason: string;
    session_id: string;
  };
}

/**
 * Script execution failed error
 */
export interface ScriptExecutionFailedError extends BaseBrowserError {
  code: BrowserErrorCode.SCRIPT_EXECUTION_FAILED;
  context: {
    reason: string;
    session_id: string;
  };
}

/**
 * Timeout error
 */
export interface TimeoutError extends BaseBrowserError {
  code: BrowserErrorCode.TIMEOUT;
  context: {
    operation: string;
    duration_ms: number;
    session_id: string;
  };
}

/**
 * Lock acquisition failed error
 */
export interface LockFailedError extends BaseBrowserError {
  code: BrowserErrorCode.LOCK_FAILED;
  context: {
    reason: string;
  };
}

/**
 * Invalid parameter error
 */
export interface InvalidParameterError extends BaseBrowserError {
  code: BrowserErrorCode.INVALID_PARAMETER;
  context: {
    parameter: string;
    reason: string;
  };
}

/**
 * Unknown error
 */
export interface UnknownError extends BaseBrowserError {
  code: BrowserErrorCode.UNKNOWN;
  context: {
    message: string;
  };
}

/**
 * Union type of all browser errors
 */
export type BrowserError =
  | SessionNotFoundError
  | SessionClosedError
  | WindowNotFoundError
  | ElementNotFoundError
  | ElementNotInteractableError
  | NavigationFailedError
  | ScriptExecutionFailedError
  | TimeoutError
  | LockFailedError
  | InvalidParameterError
  | UnknownError;

/**
 * Type guard to check if an error is a structured browser error
 */
export function isBrowserError(error: unknown): error is BrowserError {
  if (typeof error !== 'object' || error === null) {
    return false;
  }

  const err = error as Record<string, unknown>;
  return (
    typeof err.code === 'string' &&
    Object.values(BrowserErrorCode).includes(err.code as BrowserErrorCode) &&
    typeof err.context === 'object' &&
    err.context !== null
  );
}

/**
 * Parse error from backend response
 * Tries to parse as structured error, falls back to string error
 */
export function parseBrowserError(error: unknown): BrowserError | string {
  // Try to parse as JSON string first
  if (typeof error === 'string') {
    try {
      const parsed = JSON.parse(error);
      if (isBrowserError(parsed)) {
        return parsed;
      }
    } catch {
      // Not JSON, return as string
      return error;
    }
  }

  // Check if it's already a structured error
  if (isBrowserError(error)) {
    return error;
  }

  // Handle Error objects
  if (error instanceof Error) {
    try {
      const parsed = JSON.parse(error.message);
      if (isBrowserError(parsed)) {
        return parsed;
      }
    } catch {
      return error.message;
    }
  }

  // Fallback to string
  return String(error);
}

/**
 * Get user-friendly error message from browser error
 */
export function getBrowserErrorMessage(error: BrowserError): string {
  switch (error.code) {
    case BrowserErrorCode.SESSION_NOT_FOUND:
      return `Session '${error.context.session_id}' not found`;
    case BrowserErrorCode.SESSION_CLOSED:
      return `Session '${error.context.session_id}' was already closed`;
    case BrowserErrorCode.WINDOW_NOT_FOUND:
      return `Browser window for session '${error.context.session_id}' not found`;
    case BrowserErrorCode.ELEMENT_NOT_FOUND:
      return `Element with selector '${error.context.selector}' not found in session '${error.context.session_id}'`;
    case BrowserErrorCode.ELEMENT_NOT_INTERACTABLE:
      return `Element '${error.context.selector}' in session '${error.context.session_id}' is not interactable: ${error.context.reason}`;
    case BrowserErrorCode.NAVIGATION_FAILED:
      return `Navigation to '${error.context.url}' failed in session '${error.context.session_id}': ${error.context.reason}`;
    case BrowserErrorCode.SCRIPT_EXECUTION_FAILED:
      return `Script execution failed in session '${error.context.session_id}': ${error.context.reason}`;
    case BrowserErrorCode.TIMEOUT:
      return `Operation '${error.context.operation}' timed out after ${error.context.duration_ms}ms in session '${error.context.session_id}'`;
    case BrowserErrorCode.LOCK_FAILED:
      return `Failed to acquire lock: ${error.context.reason}`;
    case BrowserErrorCode.INVALID_PARAMETER:
      return `Invalid parameter '${error.context.parameter}': ${error.context.reason}`;
    case BrowserErrorCode.UNKNOWN:
      return error.context.message;
  }
}
