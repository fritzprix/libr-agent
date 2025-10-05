import { createId } from '@paralleldrive/cuid2';

/**
 * Safely parse JSON string into a value or return undefined on failure.
 */
export function tryParse<T = unknown>(input?: string): T | undefined {
  if (!input) return undefined;
  try {
    return JSON.parse(input) as T;
  } catch {
    return undefined;
  }
}

/**
 * Safely stringify a value to JSON. Falls back to '{}' on failure.
 */
export function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value ?? {});
  } catch {
    return '{}';
  }
}

/**
 * Create a normalized tool_call object expected by the rest of the codebase.
 */
export function formatToolCall(id: string, name: string, args: unknown) {
  return {
    id,
    function: {
      name,
      arguments: safeJsonStringify(args),
    },
  };
}

/**
 * Generate a short unique tool call id. Prefixed for readability.
 */
export function generateToolCallId(): string {
  return `tool_${createId()}`;
}
