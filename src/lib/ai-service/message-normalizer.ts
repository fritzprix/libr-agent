import type { Message } from '@/models/chat';
import { AIServiceProvider } from './types';
import { getLogger } from '../logger';

const logger = getLogger('MessageNormalizer');

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
   * Sanitizes an array of messages for a specific AI service provider.
   * This is the main entry point for message normalization. It executes a pipeline of
   * validation and sanitization steps to prevent API errors (e.g., 400 Bad Request).
   *
   * Pipeline:
   * 1. Filter System Errors: Remove network/API errors but keep tool execution failures.
   * 2. Validate Tool Pairing: Ensure every tool response has a matching tool call.
   * 3. Provider Sanitization: Apply specific rules (e.g., removing 'thinking' fields).
   *
   * @param messages The array of messages to sanitize.
   * @param targetProvider The target AI service provider.
   * @returns A new array of sanitized messages ready for API submission.
   */
  static sanitizeMessagesForProvider(
    messages: Message[],
    targetProvider: AIServiceProvider,
  ): Message[] {
    // Zero pass: filter out system errors (prevents polluting context)
    // but preserve tool execution errors which are part of the conversation flow
    const validMessages = this.filterSystemErrors(messages);

    // First pass: handle tool call relationships (Common for all providers)
    const processedMessages = this.validateToolCallPairing(validMessages);

    // Second pass: sanitize individual messages
    return processedMessages
      .map((msg) => this.sanitizeSingleMessage(msg, targetProvider))
      .filter((msg) => msg !== null) as Message[];
  }

  /**
   * Filters out messages containing system-level errors (AI_SERVICE_ERROR)
   * while preserving tool execution errors (TOOL_EXECUTION_ERROR).
   *
   * - System Errors (Network/API): Filtered to prevent polluting the context with transient failures.
   * - Tool Execution Errors: Preserved because the AI needs to know a tool failed to attempt self-correction.
   *
   * @param messages The array of messages to filter.
   * @returns A new array of messages without system errors.
   * @private
   */
  private static filterSystemErrors(messages: Message[]): Message[] {
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
   * @private
   */
  private static validateToolCallPairing(messages: Message[]): Message[] {
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
      const hasContent =
        processedMsg.content && processedMsg.content.length > 0;
      const hasToolCalls =
        processedMsg.tool_calls && processedMsg.tool_calls.length > 0;
      // ✅ FIX: Also check thinking field to allow thinking-only messages (Spec requirement)
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

  /**
   * Sanitizes a single message based on the target provider.
   * This acts as a dispatcher to the provider-specific sanitization methods.
   * @param message The message to sanitize.
   * @param targetProvider The target AI service provider.
   * @returns The sanitized message, or null if the message should be filtered out.
   * @private
   */
  private static sanitizeSingleMessage(
    message: Message,
    targetProvider: AIServiceProvider,
  ): Message | null {
    const sanitized = { ...message };

    switch (targetProvider) {
      case AIServiceProvider.Anthropic:
        return this.sanitizeForAnthropic(sanitized);
      case AIServiceProvider.OpenAI:
      case AIServiceProvider.Groq:
      case AIServiceProvider.Cerebras:
      case AIServiceProvider.Fireworks:
        return this.sanitizeForOpenAIFamily(sanitized);
      case AIServiceProvider.Gemini:
        return this.sanitizeForGemini(sanitized);
      case AIServiceProvider.Ollama:
        return this.sanitizeForOllama(sanitized);
      case AIServiceProvider.Empty:
        return sanitized; // No sanitization needed for empty provider
      default:
        logger.warn(`Unknown provider for sanitization: ${targetProvider}`);
        return sanitized;
    }
  }

  /**
   * Sanitizes a message for the Anthropic provider.
   * It filters out tool messages that are missing a `tool_call_id`.
   * Note: tool_calls to tool_use conversion is handled by AnthropicService.
   * @param message The message to sanitize.
   * @returns The sanitized message, or null if it should be filtered.
   * @private
   */
  private static sanitizeForAnthropic(message: Message): Message | null {
    // Filter out tool messages without tool_call_id
    if (message.role === 'tool' && !message.tool_call_id) {
      logger.debug('Filtering out tool message without tool_call_id', {
        messageId: message.id,
      });
      return null;
    }

    return message;
  }

  /**
   * Sanitizes a message for OpenAI-compatible providers (OpenAI, Groq, etc.).
   * It removes thinking-related fields and converts `tool_use` to the standard `tool_calls` format.
   * @param message The message to sanitize.
   * @returns The sanitized message.
   * @private
   */
  private static sanitizeForOpenAIFamily(message: Message): Message {
    // Remove thinking-related fields that OpenAI family doesn't support
    if (message.thinking) {
      logger.debug('Removing thinking field for OpenAI family', {
        messageId: message.id,
      });
      delete message.thinking;
    }
    if (message.thinkingSignature) {
      delete message.thinkingSignature;
    }

    // Convert tool_use to tool_calls for OpenAI family
    if (message.tool_use && !message.tool_calls) {
      message.tool_calls = [
        {
          id: message.tool_use.id,
          type: 'function',
          function: {
            name: message.tool_use.name,
            arguments: JSON.stringify(message.tool_use.input),
          },
        },
      ];
      logger.debug('Converted tool_use to tool_calls for OpenAI family', {
        messageId: message.id,
        toolName: message.tool_use.name,
      });
      delete message.tool_use;
    }

    return message;
  }

  /**
   * Sanitizes a message for the Gemini provider.
   * It removes unsupported fields like `thinking` and `tool_use`.
   * @param message The message to sanitize.
   * @returns The sanitized message.
   * @private
   */
  private static sanitizeForGemini(message: Message): Message {
    // Remove thinking fields that Gemini doesn't support
    if (message.thinking) {
      logger.debug('Removing thinking field for Gemini', {
        messageId: message.id,
      });
      delete message.thinking;
    }
    if (message.thinkingSignature) {
      delete message.thinkingSignature;
    }

    // Gemini-specific tool handling would be implemented here
    // For now, just remove unsupported fields
    if (message.tool_use) {
      logger.debug('Removing tool_use field for Gemini (not yet implemented)', {
        messageId: message.id,
      });
      delete message.tool_use;
    }

    return message;
  }

  /**
   * Sanitizes a message for the Ollama provider.
   * It removes thinking-related fields and ensures tool calls are in the standard format.
   * @param message The message to sanitize.
   * @returns The sanitized message.
   * @private
   */
  private static sanitizeForOllama(message: Message): Message {
    // Remove thinking fields that Ollama doesn't support
    if (message.thinking) {
      logger.debug('Removing thinking field for Ollama', {
        messageId: message.id,
      });
      delete message.thinking;
    }
    if (message.thinkingSignature) {
      delete message.thinkingSignature;
    }

    // Convert tool_use to tool_calls if needed (Ollama typically follows OpenAI format)
    if (message.tool_use && !message.tool_calls) {
      message.tool_calls = [
        {
          id: message.tool_use.id,
          type: 'function',
          function: {
            name: message.tool_use.name,
            arguments: JSON.stringify(message.tool_use.input),
          },
        },
      ];
      logger.debug('Converted tool_use to tool_calls for Ollama', {
        messageId: message.id,
        toolName: message.tool_use.name,
      });
      delete message.tool_use;
    }

    return message;
  }
}
