import type { Message } from '@/models/chat';
import { AIServiceProvider } from './types';
import { getLogger } from '../logger';

const logger = getLogger('MessageNormalizer');

/**
 * A utility class for normalizing and sanitizing message objects to ensure
 * compatibility with various AI service providers.
 */
export class MessageNormalizer {
  /**
   * Sanitizes an array of messages for a specific AI service provider.
   * This is the main entry point for message normalization. It applies provider-specific
   * transformations, such as fixing tool call chains for Anthropic.
   *
   * @param messages The array of messages to sanitize.
   * @param targetProvider The target AI service provider.
   * @returns A new array of sanitized messages.
   */
  static sanitizeMessagesForProvider(
    messages: Message[],
    targetProvider: AIServiceProvider,
  ): Message[] {
    // First pass: handle tool call relationships
    let processedMessages = messages;

    // Anthropic: existing validation (unchanged)
    if (targetProvider === AIServiceProvider.Anthropic) {
      processedMessages = this.fixAnthropicToolCallChain(messages);
    }

    // OpenAI-family: apply tool-call/tool-response pairing validation
    if (
      [
        AIServiceProvider.OpenAI,
        AIServiceProvider.Groq,
        AIServiceProvider.Cerebras,
        AIServiceProvider.Fireworks,
      ].includes(targetProvider)
    ) {
      processedMessages = this.ensureToolCallPairing(processedMessages);
    }

    // Second pass: sanitize individual messages
    return processedMessages
      .map((msg) => this.sanitizeSingleMessage(msg, targetProvider))
      .filter((msg) => msg !== null) as Message[];
  }

  /**
   * Ensures tool-call/tool-response pairing for OpenAI-family providers.
   * Similar to fixAnthropicToolCallChain but tailored for OpenAI API requirements.
   *
   * OpenAI API requires that every 'tool' role message must correspond to
   * a tool_call in a preceding assistant message. This function:
   * 1. Collects all tool_call ids from assistant messages
   * 2. Maps tool responses by tool_call_id
   * 3. Removes orphaned tool messages (no matching tool_call)
   * 4. Removes unmatched tool_calls from assistant messages
   * 5. Preserves message ordering and role semantics
   *
   * @param messages - Array of messages to validate
   * @returns Sanitized array with valid tool-call pairings only
   * @private
   */
  private static ensureToolCallPairing(messages: Message[]): Message[] {
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
      if (
        msg.role === 'tool' &&
        msg.tool_call_id &&
        validToolCallIds.has(msg.tool_call_id)
      ) {
        completedToolCallIds.add(msg.tool_call_id);
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

          logger.warn(
            'Removing incomplete tool_calls from assistant message for OpenAI',
            {
              messageId: msg.id,
              removedToolIds: removedIds,
              completedCount: completedToolCalls.length,
              totalCount: msg.tool_calls.length,
            },
          );
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
          logger.debug('Skipping orphaned tool message for OpenAI', {
            messageId: msg.id,
            toolCallId: msg.tool_call_id,
          });
          continue;
        }
      }

      result.push(processedMsg);
    }

    // Remove any tool messages from the beginning of conversation
    while (result.length > 0 && result[0].role === 'tool') {
      logger.warn(
        'Removing tool message from beginning of conversation for OpenAI',
        {
          messageId: result[0].id,
        },
      );
      result.shift();
    }

    logger.info('OpenAI tool call pairing validation completed', {
      originalMessages: messages.length,
      processedMessages: result.length,
      validToolCalls: validToolCallIds.size,
      completedToolCalls: completedToolCallIds.size,
    });

    return result;
  }

  /**
   * Fixes the tool call chain for Anthropic models by ensuring that every `tool_calls`
   * message from the assistant is followed by a corresponding `tool` result message.
   * It removes any incomplete tool call chains to maintain API compatibility.
   *
   * @param messages The array of messages to process.
   * @returns A new array of messages with a valid tool call chain for Anthropic.
   * @private
   */
  private static fixAnthropicToolCallChain(messages: Message[]): Message[] {
    const result: Message[] = [];
    const pendingToolUseIds = new Set<string>();
    const completedToolUseIds = new Set<string>();

    // Step 1: Collect all tool_use IDs from assistant messages
    for (const msg of messages) {
      if (msg.role === 'assistant' && msg.tool_calls) {
        msg.tool_calls.forEach((tc) => pendingToolUseIds.add(tc.id));
      }
    }

    // Step 2: Identify completed tool_uses by finding matching tool_results
    for (const msg of messages) {
      if (
        msg.role === 'tool' &&
        msg.tool_call_id &&
        pendingToolUseIds.has(msg.tool_call_id)
      ) {
        completedToolUseIds.add(msg.tool_call_id);
      }
    }

    // Step 3: Reconstruct the message list, including only complete chains
    for (const msg of messages) {
      const processedMsg = { ...msg };

      if (msg.role === 'assistant' && msg.tool_calls) {
        // Remove any tool_calls that were not completed
        const completedToolCalls = msg.tool_calls.filter((tc) =>
          completedToolUseIds.has(tc.id),
        );

        if (completedToolCalls.length !== msg.tool_calls.length) {
          const removedIds = msg.tool_calls
            .filter((tc) => !completedToolUseIds.has(tc.id))
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
          // Set the legacy tool_use field for Anthropic (uses the first tool call)
          const firstToolCall = completedToolCalls[0];
          try {
            processedMsg.tool_use = {
              id: firstToolCall.id,
              name: firstToolCall.function.name,
              input: JSON.parse(firstToolCall.function.arguments),
            };
          } catch (error) {
            logger.error('Failed to parse tool_call arguments for tool_use', {
              messageId: msg.id,
              toolCallId: firstToolCall.id,
              error,
              arguments: firstToolCall.function.arguments,
            });
          }
        } else {
          delete processedMsg.tool_calls;
          delete processedMsg.tool_use;
        }
      } else if (msg.role === 'tool' && msg.tool_call_id) {
        // Only include tool_results that correspond to a completed tool_use
        if (!completedToolUseIds.has(msg.tool_call_id)) {
          logger.debug('Skipping tool_result for incomplete tool_use', {
            messageId: msg.id,
            toolCallId: msg.tool_call_id,
          });
          continue;
        }
      }

      result.push(processedMsg);
    }

    // Remove any tool messages from the beginning of the conversation
    while (result.length > 0 && result[0].role === 'tool') {
      logger.warn('Removing tool message from beginning of conversation', {
        messageId: result[0].id,
      });
      result.shift();
    }

    logger.info('Anthropic tool chain tail management completed', {
      originalMessages: messages.length,
      processedMessages: result.length,
      pendingToolUses: pendingToolUseIds.size,
      completedToolUses: completedToolUseIds.size,
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
   * It converts `tool_calls` to the legacy `tool_use` format and filters out
   * tool messages that are missing a `tool_call_id`.
   * @param message The message to sanitize.
   * @returns The sanitized message, or null if it should be filtered.
   * @private
   */
  private static sanitizeForAnthropic(message: Message): Message | null {
    // Convert tool_calls to tool_use for Anthropic
    if (message.tool_calls && !message.tool_use) {
      const firstToolCall = message.tool_calls[0];
      if (firstToolCall) {
        try {
          message.tool_use = {
            id: firstToolCall.id,
            name: firstToolCall.function.name,
            input: JSON.parse(firstToolCall.function.arguments),
          };
          logger.debug('Converted tool_calls to tool_use for Anthropic', {
            messageId: message.id,
            toolName: firstToolCall.function.name,
          });
        } catch (error) {
          logger.error('Failed to parse tool_call arguments', {
            messageId: message.id,
            error,
            arguments: firstToolCall.function.arguments,
          });
        }
      }
      delete message.tool_calls;
    }

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
