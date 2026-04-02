import { createId } from '@paralleldrive/cuid2';
import { AIServiceProvider, TokenUsage } from './types';
import {
  type Message,
  type RustMessage,
  rustMessageToMessage,
} from '@/models/chat';
import { MCPContent } from '@/lib/mcp';
import { formatNumber } from '@/lib/utils';

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
      ? Math.min((cached / usage.promptTokens) * 100, 100).toFixed(0)
      : undefined;

  return {
    input: formatNumber(usage.promptTokens),
    output: formatNumber(usage.completionTokens),
    total: formatNumber(usage.totalTokens),
    cacheHit: cacheHitPercent ? `${cacheHitPercent}%` : undefined,
    speed: usage.details?.evalDuration
      ? `${((usage.completionTokens / usage.details.evalDuration) * 1000).toFixed(1)} t/s`
      : undefined,
  };
}

/**
 * Detects unrecoverable billing/spending-cap quota failures.
 * Transient rate-limit 429s may also use RESOURCE_EXHAUSTED, so we only
 * classify them as non-retryable when the message explicitly indicates spending.
 * @param error The error object or message returned by the provider.
 */
export function isSpendingCapError(error: unknown): boolean {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'object' &&
          error !== null &&
          'message' in error &&
          typeof error.message === 'string'
        ? error.message
        : String(error);
  return (
    message.includes('spending cap') ||
    (message.includes('RESOURCE_EXHAUSTED') && message.includes('spending'))
  );
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

export function extractStructuredToolResult(
  message: Message,
): Record<string, unknown> | null {
  const metadata = message.metadata;
  if (typeof metadata !== 'object' || metadata === null) {
    return null;
  }

  const structuredContent = metadata.structuredContent;
  if (
    typeof structuredContent !== 'object' ||
    structuredContent === null ||
    Array.isArray(structuredContent)
  ) {
    return null;
  }

  return structuredContent as Record<string, unknown>;
}

export function formatToolResultForLlm(message: Message): string {
  const structuredResult = extractStructuredToolResult(message);
  if (structuredResult) {
    return safeJsonStringify(structuredResult);
  }

  return processMessageContent(message.content);
}

export function parseToolResultForLlm(
  message: Message,
): Record<string, unknown> {
  const structuredResult = extractStructuredToolResult(message);
  if (structuredResult) {
    return structuredResult;
  }

  const text = processMessageContent(message.content);
  const parsed = tryParse<Record<string, unknown>>(text);
  if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
    return parsed;
  }

  return { result: text };
}

/**
 * Processes an array of `MCPContent` parts for a multimodal LLM,
 * handling both text and image content.
 */
type MediaItem = {
  data?: string;
  uri?: string;
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
      case 'image': {
        const mediaItem = item as MediaItem;
        const data = mediaItem.data || mediaItem.source?.data;
        if (data) {
          return {
            type: 'image',
            image: data,
            mimeType: mediaItem.mimeType || mediaItem.source?.mimeType,
          };
        }

        const uri = mediaItem.uri || mediaItem.source?.uri;
        return {
          type: 'text',
          text: `[unresolved image omitted from multimodal request: ${uri || 'missing-uri'}]`,
        };
      }
      case 'audio': {
        const mediaItem = item as MediaItem;
        const data = mediaItem.data || mediaItem.source?.data;
        if (data) {
          return {
            type: 'audio',
            audio: data,
            mimeType: mediaItem.mimeType || mediaItem.source?.mimeType,
          };
        }

        const uri = mediaItem.uri || mediaItem.source?.uri;
        return {
          type: 'text',
          text: `[unresolved audio omitted from multimodal request: ${uri || 'missing-uri'}]`,
        };
      }
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

/**
 * Recursively ensures that all nodes in a JSON schema have a 'type' field.
 * This is required by many AI providers (OpenAI, Anthropic, Gemini, etc.)
 * to prevent validation errors when the MCP server provides an incomplete schema.
 * @param schema The JSON schema part to fix.
 * @returns A new schema object with 'type' fields added where missing.
 */
export function ensureSchemaTypeField(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  if (!schema || typeof schema !== 'object') {
    return { type: 'object', properties: {} };
  }

  const result = { ...schema };

  // Ensure root schema has type field
  if (!result.type) {
    // Infer type from structure
    if (result.properties && typeof result.properties === 'object') {
      result.type = 'object';
    } else if (result.items) {
      result.type = 'array';
    } else {
      result.type = 'object'; // default fallback
    }
  }

  // Handle array-type type fields (convert to single type)
  if (Array.isArray(result.type)) {
    // Prioritize the first non-null type
    const nonNullType = (result.type as string[]).find((t) => t !== 'null');
    result.type = nonNullType || 'string';
  }

  // Recursively ensure properties have type fields
  if (result.properties && typeof result.properties === 'object') {
    const properties = result.properties as Record<string, unknown>;
    const fixedProperties: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(properties)) {
      if (typeof value === 'object' && value !== null) {
        fixedProperties[key] = ensureSchemaTypeField(
          value as Record<string, unknown>,
        );
      } else {
        fixedProperties[key] = value;
      }
    }
    result.properties = fixedProperties;
  }

  // Recursively ensure array items have type fields
  if (result.items) {
    if (Array.isArray(result.items)) {
      result.items = result.items.map((item) =>
        typeof item === 'object' && item !== null
          ? ensureSchemaTypeField(item as Record<string, unknown>)
          : item,
      );
    } else if (typeof result.items === 'object' && result.items !== null) {
      result.items = ensureSchemaTypeField(
        result.items as Record<string, unknown>,
      );
    }
  }

  return result;
}
