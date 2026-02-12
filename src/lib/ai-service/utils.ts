import { createId } from '@paralleldrive/cuid2';
import { AIServiceProvider } from './types';
import {
  type Message,
  type RustMessage,
  rustMessageToMessage,
} from '@/models/chat';
import { MCPContent } from '@/lib/mcp';

/**
 * Type guard for AIServiceProvider
 */
export function isAIServiceProvider(
  value: unknown,
): value is AIServiceProvider {
  return Object.values(AIServiceProvider).includes(value as AIServiceProvider);
}

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

/**
 * Normalizes a message from Rust (camelCase) to the internal Message format (snake_case for tool fields).
 * Rust backend sends messages with camelCase fields (toolCalls, toolCallId) due to serde settings,
 * but the frontend Message interface expects snake_case (tool_calls, tool_call_id).
 */
export function normalizeRustMessage(msg: RustMessage | Message): Message {
  // Check if it's a RustMessage (has camelCase fields or numeric timestamp)
  // We check for specific RustMessage characteristics
  const candidate = msg as RustMessage;

  if (
    'toolCalls' in candidate ||
    'toolCallId' in candidate ||
    typeof candidate.createdAt === 'number'
  ) {
    return rustMessageToMessage(candidate);
  }

  // Already a Message
  return msg as Message;
}

/**
 * Calculate tokens per second from usage metrics
 */
export function calculateTokensPerSecond(
  usage: import('./types').TokenUsage,
  durationMs: number,
): number {
  if (usage.completionTokens === 0 || durationMs === 0) return 0;
  return (usage.completionTokens / durationMs) * 1000;
}

/**
 * Format usage metrics for display
 */
export function formatUsageMetrics(usage: import('./types').TokenUsage): {
  input: string;
  output: string;
  total: string;
  speed?: string;
} {
  return {
    input: usage.promptTokens.toLocaleString(),
    output: usage.completionTokens.toLocaleString(),
    total: usage.totalTokens.toLocaleString(),
    speed: usage.details?.evalDuration
      ? `${((usage.completionTokens / usage.details.evalDuration) * 1000).toFixed(1)} t/s`
      : undefined,
  };
}

/**
 * Processes an array of `MCPContent` parts into a single string,
 * extracting only the text content.
 */
export function processMessageContent(content: string | MCPContent[]): string {
  if (typeof content === 'string') {
    return content;
  }
  if (!Array.isArray(content)) {
    return '';
  }
  // Extracts only the text from the MCPContent array
  return content
    .filter((item) => item.type === 'text')
    .map((item) => (item as { text: string }).text)
    .join('\n');
}

/**
 * Processes an array of `MCPContent` parts for a multimodal LLM,
 * handling both text and image content.
 */
export function processMultiModalContent(
  content: MCPContent[],
): Array<{ type: string; text?: string; image?: string }> {
  return content.map((item) => {
    switch (item.type) {
      case 'text':
        return { type: 'text', text: (item as { text: string }).text };
      case 'image':
        return {
          type: 'image',
          image:
            (
              item as {
                data?: string;
                source?: { data?: string; uri?: string };
              }
            ).data ||
            (item as { source?: { data?: string; uri?: string } }).source
              ?.data ||
            (item as { source?: { data?: string; uri?: string } }).source?.uri,
        };
      default:
        return { type: 'text', text: `[${item.type}]` };
    }
  });
}
