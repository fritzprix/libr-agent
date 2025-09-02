import type { Message } from '@/models/chat';
import { AIServiceProvider } from './types';
import { getLogger } from '../logger';

const logger = getLogger('MessageNormalizer');

export class MessageNormalizer {
  /**
   * Sanitize messages for specific vendor to ensure API compatibility
   */
  static sanitizeMessagesForProvider(
    messages: Message[],
    targetProvider: AIServiceProvider,
  ): Message[] {
    // First pass: handle tool call relationships for Anthropic
    let processedMessages = messages;
    if (targetProvider === AIServiceProvider.Anthropic) {
      processedMessages = this.fixAnthropicToolCallChain(messages);
    }

    // Second pass: sanitize individual messages
    return processedMessages
      .map((msg) => this.sanitizeSingleMessage(msg, targetProvider))
      .filter((msg) => msg !== null) as Message[];
  }

  /**
   * Fix Anthropic tool call chain to ensure proper order and relationships
   * Enhanced to handle tool chain tail management for complete integrity
   */
  private static fixAnthropicToolCallChain(messages: Message[]): Message[] {
    const result: Message[] = [];
    const pendingToolUseIds = new Set<string>();
    const completedToolUseIds = new Set<string>();

    // 1단계: 모든 tool_use ID 수집
    for (const msg of messages) {
      if (msg.role === 'assistant' && msg.tool_calls) {
        msg.tool_calls.forEach((tc) => pendingToolUseIds.add(tc.id));
      }
    }

    // 2단계: tool_result로 완료된 tool_use 식별
    for (const msg of messages) {
      if (
        msg.role === 'tool' &&
        msg.tool_call_id &&
        pendingToolUseIds.has(msg.tool_call_id)
      ) {
        completedToolUseIds.add(msg.tool_call_id);
      }
    }

    // 3단계: 완전한 체인만 포함하여 메시지 재구성
    for (const msg of messages) {
      const processedMsg = { ...msg };

      if (msg.role === 'assistant' && msg.tool_calls) {
        // 완료되지 않은 tool_calls 제거
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
          // Anthropic용 tool_use 설정 (첫 번째 tool만)
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
        // 완료된 tool_use에 대응하는 tool_result만 포함
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

    // 대화 시작 부분의 tool 메시지 제거
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
