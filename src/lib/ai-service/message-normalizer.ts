import type { Message } from '@/models/chat';
import type { AIMessageSanitizationService } from './types';
import { getLogger } from '../logger';
import type { MCPContent } from '@/lib/mcp';

const logger = getLogger('MessageNormalizer');

/**
 * Filters out messages containing system-level errors (AI_SERVICE_ERROR)
 * while preserving tool execution errors (TOOL_EXECUTION_ERROR).
 *
 * - System Errors (Network/API): Filtered to prevent polluting the context with transient failures.
 * - Tool Execution Errors: Preserved because the AI needs to know a tool failed to attempt self-correction.
 *
 * @param messages The array of messages to filter.
 * @returns A new array of messages without system errors.
 */
export function filterSystemErrors(messages: Message[]): Message[] {
  return messages.filter((msg) => {
    if (!msg.error) return true;

    // Keep tool execution errors as they are part of the conversation context
    // The AI needs to know that a tool failed to execute
    if (msg.role === 'tool' || msg.error.type === 'TOOL_EXECUTION_ERROR') {
      return true;
    }

    // Filter out system errors (network issues, API errors, etc.)
    // These should not be part of the conversation history sent to the AI
    logger.debug('Filtering out system error message', {
      messageId: msg.id,
      errorType: msg.error.type,
    });
    return false;
  });
}

/**
 * Ensures strict tool-call/tool-response pairing for all providers.
 *
 * Critical for OpenAI and Anthropic, which enforce a strict 1:1 mapping:
 * - Every 'tool' message MUST have a preceding 'assistant' message with a matching `tool_call_id`.
 * - Every `tool_call` in an 'assistant' message MUST have a following 'tool' message.
 *
 * This function prevents "400 Bad Request" errors by:
 * 1. Removing orphaned tool responses (no matching call).
 * 2. Removing incomplete tool calls (no matching response) from assistant messages.
 *
 * @param messages - Array of messages to validate
 * @returns Sanitized array with valid tool-call pairings only
 */
export function validateToolCallPairing(messages: Message[]): Message[] {
  const result: Message[] = [];
  const validToolCallIds = new Set<string>();
  const completedToolCallIds = new Set<string>();

  // Step 1: Collect all tool_call ids from assistant messages
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls?.length) {
      msg.tool_calls.forEach((tc) => {
        if (tc.id) {
          validToolCallIds.add(tc.id);
        }
      });
    }
  }

  // Step 2: Identify completed tool_calls by finding matching tool responses
  for (const msg of messages) {
    if (msg.role === 'tool') {
      if (msg.tool_call_id && validToolCallIds.has(msg.tool_call_id)) {
        completedToolCallIds.add(msg.tool_call_id);
      } else {
        logger.warn('Tool message validation failed', {
          messageId: msg.id,
          toolCallId: msg.tool_call_id,
          reason: !msg.tool_call_id
            ? 'Missing tool_call_id'
            : 'tool_call_id not found in assistant messages',
          knownToolCallIds: Array.from(validToolCallIds),
        });
      }
    }
  }

  // Step 3: Reconstruct message list with only valid pairings
  for (const msg of messages) {
    const processedMsg = { ...msg };

    if (msg.role === 'assistant' && msg.tool_calls?.length) {
      // Keep only tool_calls that have matching tool responses
      const completedToolCalls = msg.tool_calls.filter((tc) =>
        completedToolCallIds.has(tc.id),
      );

      if (completedToolCalls.length !== msg.tool_calls.length) {
        const removedIds = msg.tool_calls
          .filter((tc) => !completedToolCallIds.has(tc.id))
          .map((tc) => tc.id);

        logger.warn('Removing incomplete tool_calls from assistant message', {
          messageId: msg.id,
          removedToolIds: removedIds,
          completedCount: completedToolCalls.length,
          totalCount: msg.tool_calls.length,
        });
      }

      if (completedToolCalls.length > 0) {
        processedMsg.tool_calls = completedToolCalls;
      } else {
        // No valid tool_calls, remove the field but keep message
        delete processedMsg.tool_calls;
      }
    } else if (msg.role === 'tool') {
      // Only include tool messages that have matching tool_calls
      if (!msg.tool_call_id || !completedToolCallIds.has(msg.tool_call_id)) {
        logger.debug('Skipping orphaned tool message', {
          messageId: msg.id,
          toolCallId: msg.tool_call_id,
        });
        continue;
      }
    }

    // Check if message is now empty (no content, no tool_calls, no thinking)
    // This prevents sending invalid messages to the API (e.g. 400 Bad Request)
    const hasContent = processedMsg.content && processedMsg.content.length > 0;
    const hasToolCalls =
      processedMsg.tool_calls && processedMsg.tool_calls.length > 0;
    const hasThinking =
      processedMsg.thinking && processedMsg.thinking.length > 0;

    if (!hasContent && !hasToolCalls && !hasThinking) {
      logger.warn(
        'Removing truly empty message after sanitization (no content, tool_calls, or thinking)',
        {
          messageId: msg.id,
          role: msg.role,
          originalContent: msg.content,
          originalToolCallsCount: msg.tool_calls?.length ?? 0,
        },
      );
      continue;
    }

    result.push(processedMsg);
  }

  // Remove any tool messages from the beginning of conversation
  while (result.length > 0 && result[0].role === 'tool') {
    logger.warn('Removing tool message from beginning of conversation', {
      messageId: result[0].id,
    });
    result.shift();
  }

  logger.info('Tool call pairing validation completed', {
    originalMessages: messages.length,
    processedMessages: result.length,
    validToolCalls: validToolCallIds.size,
    completedToolCalls: completedToolCallIds.size,
  });

  return result;
}

interface MalformedToolCallIssue {
  toolCallId: string;
  toolName: string;
  reason: string;
  argumentsPreview: string;
}

function truncateToolArgumentPreview(rawArguments: string): string {
  const normalized = rawArguments.replace(/\s+/g, ' ').trim();
  return normalized.length > 160
    ? `${normalized.slice(0, 157)}...`
    : normalized;
}

function inspectToolCallArguments(
  rawArguments: string,
): { valid: true } | { valid: false; reason: string } {
  try {
    const parsed = JSON.parse(rawArguments);
    if (
      typeof parsed !== 'object' ||
      parsed === null ||
      Array.isArray(parsed)
    ) {
      return {
        valid: false,
        reason: 'arguments must decode to a JSON object',
      };
    }

    return { valid: true };
  } catch (error) {
    return {
      valid: false,
      reason: error instanceof Error ? error.message : String(error),
    };
  }
}

function buildMalformedToolCallRepairContent(
  issues: MalformedToolCallIssue[],
): MCPContent[] {
  const lines = issues.map(
    ({ toolCallId, toolName, reason, argumentsPreview }) =>
      `- Tool call "${toolName}" (id: ${toolCallId}) was omitted because its arguments were invalid (${reason}). Original arguments preview: ${argumentsPreview}`,
  );

  return [
    {
      type: 'text',
      text: [
        '[Sanitizer note: invalid tool call arguments were removed from conversation history to prevent repeated 400 Bad Request failures.]',
        ...lines,
        'Treat each omitted tool call as a failed attempt. If you retry, emit a valid JSON object for function.arguments and double-check quotes, commas, braces, and nesting before sending the next tool call.',
      ].join('\n'),
    },
  ];
}

/**
 * Removes assistant tool calls whose `function.arguments` would poison future
 * provider requests, then replaces them with explicit assistant text so the
 * model can recover instead of resending the same malformed payload forever.
 */
export function repairMalformedToolCalls(messages: Message[]): Message[] {
  return messages.map((message) => {
    if (message.role !== 'assistant' || !message.tool_calls?.length) {
      return message;
    }

    const validToolCalls = [];
    const malformedIssues: MalformedToolCallIssue[] = [];

    for (const toolCall of message.tool_calls) {
      const inspection = inspectToolCallArguments(toolCall.function.arguments);
      if (inspection.valid) {
        validToolCalls.push(toolCall);
        continue;
      }

      malformedIssues.push({
        toolCallId: toolCall.id,
        toolName: toolCall.function.name,
        reason: inspection.reason,
        argumentsPreview: truncateToolArgumentPreview(
          toolCall.function.arguments,
        ),
      });
    }

    if (malformedIssues.length === 0) {
      return message;
    }

    logger.warn('Repairing malformed tool call arguments in message history', {
      messageId: message.id,
      malformedToolCallIds: malformedIssues.map((issue) => issue.toolCallId),
      malformedToolCallNames: malformedIssues.map((issue) => issue.toolName),
    });

    const repairedMessage: Message = {
      ...message,
      content: [
        ...(message.content ?? []),
        ...buildMalformedToolCallRepairContent(malformedIssues),
      ],
    };

    if (validToolCalls.length > 0) {
      repairedMessage.tool_calls = validToolCalls;
    } else {
      delete repairedMessage.tool_calls;
    }

    return repairedMessage;
  });
}

/**
 * Merges consecutive user messages into a single message while preserving
 * attachments from all merged messages.
 *
 * This runs after provider-specific sanitization so tool/assistant boundaries
 * remain intact and only adjacent user messages are coalesced.
 *
 * @param messages The array of sanitized messages to merge.
 * @returns A new array where adjacent user messages have been merged.
 */
export function mergeConsecutiveUserMessages(messages: Message[]): Message[] {
  const merged: Message[] = [];

  for (const msg of messages) {
    const last = merged[merged.length - 1];

    if (last && last.role === 'user' && msg.role === 'user') {
      const lastContent = Array.isArray(last.content)
        ? (last.content as MCPContent[])
        : ([{ type: 'text', text: String(last.content) }] as MCPContent[]);
      const nextContent = Array.isArray(msg.content)
        ? (msg.content as MCPContent[])
        : ([{ type: 'text', text: String(msg.content) }] as MCPContent[]);

      last.content = [
        ...lastContent,
        { type: 'text', text: '\n\n' } as MCPContent,
        ...nextContent,
      ];

      if (msg.attachments?.length) {
        last.attachments = [...(last.attachments ?? []), ...msg.attachments];
      }

      logger.debug('Merged consecutive user messages in generic layer', {
        id1: last.id,
        id2: msg.id,
      });
    } else {
      merged.push({ ...msg });
    }
  }

  return merged;
}

/**
 * Runs the common message normalization pipeline using a concrete AI service
 * instance for provider-specific sanitization.
 *
 * @param messages The array of messages to sanitize.
 * @param service The target AI service instance.
 * @returns A new array of sanitized messages ready for API submission.
 */
export function sanitizeMessagesForService(
  messages: Message[],
  service: AIMessageSanitizationService,
): Message[] {
  return mergeConsecutiveUserMessages(service.sanitizeMessages(messages));
}

/**
 * A utility class for normalizing and sanitizing message objects to ensure
 * compatibility with various AI service providers.
 *
 * Responsibilities:
 * 1. Validation: Ensures strict 1:1 pairing between tool calls and tool responses.
 * 2. Sanitization: Removes unsupported fields (e.g., 'thinking' for OpenAI) and system errors.
 * 3. Normalization: Converts internal message formats to provider-specific structures.
 */
export class MessageNormalizer {
  /**
   * Filters out messages containing system-level errors (AI_SERVICE_ERROR)
   * while preserving tool execution errors (TOOL_EXECUTION_ERROR).
   *
   * @param messages The array of messages to filter.
   * @returns A new array of messages without system errors.
   */
  static filterSystemErrors = filterSystemErrors;
  static repairMalformedToolCalls = repairMalformedToolCalls;

  /**
   * Ensures strict tool-call/tool-response pairing for all providers.
   *
   * @param messages - Array of messages to validate
   * @returns Sanitized array with valid tool-call pairings only
   */
  static validateToolCallPairing = validateToolCallPairing;
  static mergeConsecutiveUserMessages = mergeConsecutiveUserMessages;
  static sanitizeMessagesForService = sanitizeMessagesForService;
}
