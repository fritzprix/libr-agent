import type { Message } from '@/models/chat';

/**
 * Checks if a tool result message contains an error.
 * Uses the Message.error property for type-safe error detection.
 * Falls back to text pattern matching for backward compatibility.
 */
export function hasToolCallError(toolResult?: Message): boolean {
  // Primary: Check for structured error property
  if (toolResult?.error) {
    return true;
  }

  // Fallback: Check text patterns for backward compatibility
  return (
    toolResult?.content?.some(
      (c) =>
        c.type === 'text' &&
        (c.text?.startsWith('❌') || c.text?.startsWith('Error:')),
    ) || false
  );
}

/**
 * Checks if a tool result message contains a UI resource.
 * UI resources are content items with type 'resource' and a mimeType.
 */
export function hasUIResource(toolResult?: Message): boolean {
  return (
    toolResult?.content?.some(
      (c) => c.type === 'resource' && c.resource?.mimeType,
    ) || false
  );
}

/**
 * Parses a tool name by removing the server prefix.
 * Example: "server__toolName" -> "toolName"
 */
export function parseToolName(fullToolName: string): string {
  return fullToolName.split('__').pop() || fullToolName;
}

/**
 * Safely parses tool call arguments from JSON string.
 * Returns parsed object or wraps raw string on parse error.
 */
export function parseToolArguments(
  argumentsString: string,
): Record<string, unknown> {
  try {
    return JSON.parse(argumentsString) as Record<string, unknown>;
  } catch {
    return { raw: argumentsString };
  }
}

/**
 * Formats execution time in milliseconds to human-readable string.
 * Returns "Xms" for times under 1 second, "X.Xs" otherwise.
 */
export function formatExecutionTime(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`;
  }
  return `${(ms / 1000).toFixed(1)}s`;
}
