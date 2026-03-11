import { createId } from '@paralleldrive/cuid2';
import { AIServiceProvider, TokenUsage } from './types';
import {
  type Message,
  type RustMessage,
  rustMessageToMessage,
} from '@/models/chat';
import { MCPContent } from '@/lib/mcp';

/**
 * Type guard for AIServiceProvider
 * @param value A dynamic value to check.
 */
export function isAIServiceProvider(
  value: unknown,
): value is AIServiceProvider {
  return Object.values(AIServiceProvider).includes(value as AIServiceProvider);
}

/**
 * Safely parse JSON string into a value or return undefined on failure.
 * @param input Raw input string to attempt to parse.
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
 * @param value The value to JSON-stringify; falls back to '{}' if serialization fails.
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
 * @param id The tool call identifier.
 * @param name The name of the tool being called.
 * @param args Parsed tool arguments payload.
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
 * @param msg The raw message object from Rust/UI.
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
 * @param usage Output usage stats returned by a model provider.
 * @param durationMs The total duration in milliseconds that generation took.
 */
export function calculateTokensPerSecond(
  usage: TokenUsage,
  durationMs: number,
): number {
  if (usage.completionTokens === 0 || durationMs === 0) return 0;
  return (usage.completionTokens / durationMs) * 1000;
}

/**
 * Format usage metrics for display
 * @param usage Output usage stats returned by a model provider.
 */
export function formatUsageMetrics(usage: TokenUsage): {
  input: string;
  output: string;
  total: string;
  cacheHit?: string;
  speed?: string;
} {
  const cached =
    usage.cachedPromptTokens ?? usage.details?.cacheReadInputTokens;
  const cacheHitPercent =
    cached !== undefined && usage.promptTokens > 0
      ? ((cached / usage.promptTokens) * 100).toFixed(0)
      : undefined;

  return {
    input: usage.promptTokens.toLocaleString(),
    output: usage.completionTokens.toLocaleString(),
    total: usage.totalTokens.toLocaleString(),
    cacheHit: cacheHitPercent ? `${cacheHitPercent}%` : undefined,
    speed: usage.details?.evalDuration
      ? `${((usage.completionTokens / usage.details.evalDuration) * 1000).toFixed(1)} t/s`
      : undefined,
  };
}

/**
 * Processes an array of `MCPContent` parts into a single string,
 * extracting only the text content.
 * @param content The content of the message.
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
type MediaItem = {
  data?: string;
  mimeType?: string;
  source?: { data?: string; uri?: string; mimeType?: string };
};

export function processMultiModalContent(content: MCPContent[]): Array<{
  type: string;
  text?: string;
  image?: string;
  audio?: string;
  mimeType?: string;
}> {
  return content.map((item) => {
    switch (item.type) {
      case 'text':
        return { type: 'text', text: (item as { text: string }).text };
      case 'image':
        return {
          type: 'image',
          image: (item as MediaItem).data || (item as MediaItem).source?.data,
          mimeType:
            (item as MediaItem).mimeType ||
            (item as MediaItem).source?.mimeType,
        };
      case 'audio':
        return {
          type: 'audio',
          audio: (item as MediaItem).data || (item as MediaItem).source?.data,
          mimeType:
            (item as MediaItem).mimeType ||
            (item as MediaItem).source?.mimeType,
        };
      default:
        return { type: 'text', text: `[${item.type}]` };
    }
  });
}

/**
 * Extracts image and audio items from a MCPContent array.
 * Used by provider conversion loops to identify media that requires special handling
 * since tool result messages can only carry text in the standard API format.
 * @param content The full content array from a tool result message.
 * @returns Only the image and audio MCPContent items.
 */
export function extractMediaContent(content: MCPContent[]): MCPContent[] {
  return content.filter((c) => c.type === 'image' || c.type === 'audio');
}
