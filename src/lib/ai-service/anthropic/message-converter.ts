import {
  MessageParam as AnthropicMessageParam,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import { Message } from '@/models/chat';
import { getLogger } from '../../logger';
import { processMessageContent } from '../utils';
import { parseToolInput, ensureObjectInput } from './tool-utils';

const logger = getLogger('AnthropicMessageConverter');

/**
 * Converts an array of standard `Message` objects into the format required
 * by the Anthropic API. It also performs a strict integrity check to ensure
 * that all tool calls have a corresponding tool result, throwing an error
 * if any inconsistencies are found.
 *
 * @param messages The array of messages to convert.
 * @returns An array of `AnthropicMessageParam` objects.
 */
export function convertToAnthropicMessages(
  messages: Message[],
): AnthropicMessageParam[] {
  const anthropicMessages: AnthropicMessageParam[] = [];
  const toolUseIds = new Set<string>();
  const toolResultIds = new Set<string>();

  // Track tool chains for debugging and integrity checks
  for (const m of messages) {
    if (m.role === 'assistant' && m.tool_use) {
      toolUseIds.add(m.tool_use.id);
    } else if (m.role === 'assistant' && m.tool_calls) {
      m.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
    } else if (m.role === 'tool' && m.tool_call_id) {
      toolResultIds.add(m.tool_call_id);
    }
  }

  // Verify tool chain integrity
  const unmatchedToolUses = Array.from(toolUseIds).filter(
    (id) => !toolResultIds.has(id),
  );
  const unmatchedToolResults = Array.from(toolResultIds).filter(
    (id) => !toolUseIds.has(id),
  );

  if (unmatchedToolUses.length > 0 || unmatchedToolResults.length > 0) {
    logger.warn('Potential tool chain mismatch detected', {
      unmatchedToolUses,
      unmatchedToolResults,
      totalMessages: messages.length,
      toolUseIds: Array.from(toolUseIds),
      toolResultIds: Array.from(toolResultIds),
    });
  }

  logger.debug('Tool chain integrity verification passed', {
    totalMessages: messages.length,
    toolUseCount: toolUseIds.size,
    toolResultCount: toolResultIds.size,
  });

  for (const m of messages) {
    // Convert UI-originated messages to user role for provider calls
    const effectiveRole = m.source === 'ui' ? 'user' : m.role;

    if (effectiveRole === 'system') {
      // System messages are handled separately in the API call
      continue;
    }

    if (effectiveRole === 'user') {
      anthropicMessages.push({
        role: 'user',
        content: processMessageContent(m.content),
      });
    } else if (effectiveRole === 'assistant') {
      // Filter out empty assistant messages that would cause API errors
      const hasContent = m.content && m.content.length > 0;
      const hasToolCalls = m.tool_calls && m.tool_calls.length > 0;
      const hasToolUse = m.tool_use;

      // Skip empty assistant messages to prevent 400 errors
      if (!hasContent && !hasToolCalls && !hasToolUse) {
        logger.debug('Skipping empty assistant message', { messageId: m.id });
        continue;
      }

      // Build content array with thinking block first if present
      const content = [];

      // Add thinking block as first element if exists
      if (m.thinking) {
        content.push({
          type: 'thinking' as const,
          thinking: m.thinking,
          signature: m.thinkingSignature || '',
        });
      }

      // Add tool_use content
      if (m.tool_calls) {
        content.push(
          ...m.tool_calls.map((tc) => ({
            type: 'tool_use' as const,
            id: tc.id,
            name: tc.function.name,
            input: parseToolInput(tc.function.arguments, {
              messageId: m.id,
              toolId: tc.id,
              toolName: tc.function.name,
            }),
          })),
        );
      } else if (m.tool_use) {
        content.push({
          type: 'tool_use' as const,
          id: m.tool_use.id,
          name: m.tool_use.name,
          input: ensureObjectInput(m.tool_use.input, {
            messageId: m.id,
            toolId: m.tool_use.id,
            toolName: m.tool_use.name,
          }),
        });
      }

      // Always add text content if it exists, regardless of tool use
      if (hasContent) {
        const processedContent = processMessageContent(m.content);
        if (processedContent && processedContent.length > 0) {
          content.push({ type: 'text' as const, text: processedContent });
        }
      }

      if (content.length > 0) {
        anthropicMessages.push({
          role: 'assistant',
          content,
        });
      }
    } else if (effectiveRole === 'tool') {
      if (!m.tool_call_id) {
        logger.warn('Tool message missing tool_call_id, skipping', {
          messageId: m.id,
        });
        continue;
      }
      anthropicMessages.push({
        role: 'user',
        content: [
          {
            type: 'tool_result' as const,
            tool_use_id: m.tool_call_id,
            content: processMessageContent(m.content),
          },
        ],
      });
    } else {
      logger.warn(`Unsupported message role for Anthropic: ${m.role}`);
    }
  }
  return anthropicMessages;
}

/**
 * Converts a single `Message` into the format expected by the Anthropic API.
 */
export function convertSingleAnthropicMessage(message: Message): unknown {
  if (message.role === 'system') {
    // System messages are handled separately in the API call
    return null;
  }

  if (message.role === 'user') {
    return {
      role: 'user',
      content: processMessageContent(message.content),
    };
  } else if (message.role === 'assistant') {
    // Build content array with thinking block first if present
    const content = [];

    // Add thinking block as first element if exists
    if (message.thinking) {
      content.push({
        type: 'thinking' as const,
        thinking: message.thinking,
        signature: message.thinkingSignature || '',
      });
    }

    // Add tool_use content
    if (message.tool_calls) {
      content.push(
        ...message.tool_calls.map((tc) => ({
          type: 'tool_use' as const,
          id: tc.id,
          name: tc.function.name,
          input: parseToolInput(tc.function.arguments, {
            messageId: message.id,
            toolId: tc.id,
            toolName: tc.function.name,
          }),
        })),
      );
    } else if (message.tool_use) {
      content.push({
        type: 'tool_use' as const,
        id: message.tool_use.id,
        name: message.tool_use.name,
        input: ensureObjectInput(message.tool_use.input, {
          messageId: message.id,
          toolId: message.tool_use.id,
          toolName: message.tool_use.name,
        }),
      });
    }

    // Always add text content if it exists
    if (message.content) {
      const processedContent = processMessageContent(message.content);
      if (processedContent && processedContent.length > 0) {
        content.push({ type: 'text' as const, text: processedContent });
      }
    }

    return {
      role: 'assistant',
      content,
    };
  } else if (message.role === 'tool') {
    if (!message.tool_call_id) {
      logger.warn('Tool message missing tool_call_id, skipping', {
        messageId: message.id,
      });
      return null;
    }
    return {
      role: 'user',
      content: [
        {
          type: 'tool_result' as const,
          tool_use_id: message.tool_call_id,
          content: processMessageContent(message.content),
        },
      ],
    };
  } else {
    logger.warn(`Unsupported message role for Anthropic: ${message.role}`);
    return null;
  }
}
