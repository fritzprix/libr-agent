import type {
  ContentBlockParam,
  MessageParam as AnthropicMessageParam,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import { getLogger } from '../../logger';
import { processMessageContent } from '../utils';
import {
  buildAnthropicToolResultBlocks,
  formatAnthropicContent,
} from './format';
import {
  ensureAnthropicObjectInput,
  parseAnthropicToolInput,
} from './tool-input';

const logger = getLogger('AnthropicMessageConverter');
const MIN_STABLE_MESSAGES_FOR_EXTRA_BREAKPOINT = 8;

function isAnthropicSyntheticSessionContextMessage(message: Message): boolean {
  return message.role === 'user' && message.source === 'session-context';
}

function applyCacheBreakpoint(
  anthropicMessage: AnthropicMessageParam | undefined,
): void {
  if (!anthropicMessage) {
    return;
  }

  if (typeof anthropicMessage.content === 'string') {
    if (anthropicMessage.content.length === 0) {
      return;
    }

    anthropicMessage.content = [
      {
        type: 'text',
        text: anthropicMessage.content,
        cache_control: { type: 'ephemeral' },
      },
    ];
    return;
  }

  if (anthropicMessage.content.length === 0) {
    return;
  }

  for (
    let blockIndex = anthropicMessage.content.length - 1;
    blockIndex >= 0;
    blockIndex -= 1
  ) {
    const block: ContentBlockParam = anthropicMessage.content[blockIndex];

    switch (block.type) {
      case 'thinking':
      case 'redacted_thinking':
        continue;
      case 'text':
      case 'image':
      case 'document':
      case 'search_result':
      case 'tool_use':
      case 'tool_result':
      case 'server_tool_use':
      case 'web_search_tool_result':
        anthropicMessage.content[blockIndex] = {
          ...block,
          cache_control: { type: 'ephemeral' },
        };
        return;
    }
  }
}

function applyLongConversationCacheBreakpoint(
  anthropicMessages: AnthropicMessageParam[],
  lastStableMessageIndex: number,
): void {
  if (lastStableMessageIndex + 1 < MIN_STABLE_MESSAGES_FOR_EXTRA_BREAKPOINT) {
    return;
  }

  const midpointIndex = Math.floor(lastStableMessageIndex / 2);
  if (midpointIndex <= 0 || midpointIndex >= lastStableMessageIndex) {
    return;
  }

  applyCacheBreakpoint(anthropicMessages[midpointIndex]);
}

function logToolChainIntegrity(messages: Message[]): void {
  const toolUseIds = new Set<string>();
  const toolResultIds = new Set<string>();

  for (const message of messages) {
    if (message.role === 'assistant' && message.tool_use) {
      toolUseIds.add(message.tool_use.id);
    } else if (message.role === 'assistant' && message.tool_calls) {
      message.tool_calls.forEach((toolCall) => toolUseIds.add(toolCall.id));
    } else if (message.role === 'tool' && message.tool_call_id) {
      toolResultIds.add(message.tool_call_id);
    }
  }

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
}

export function convertToAnthropicMessages(
  messages: Message[],
  systemPrompt?: string,
): AnthropicMessageParam[] {
  void systemPrompt;

  logToolChainIntegrity(messages);

  const anthropicMessages: AnthropicMessageParam[] = [];
  let pendingToolResults: ReturnType<
    typeof buildAnthropicToolResultBlocks
  >['content'] = [];
  let hasSyntheticSessionContextTail = false;

  const flushPendingToolResults = () => {
    if (pendingToolResults.length === 0) {
      return;
    }

    anthropicMessages.push({
      role: 'user',
      content: pendingToolResults,
    });
    pendingToolResults = [];
  };

  for (const message of messages) {
    const effectiveRole = message.source === 'ui' ? 'user' : message.role;

    if (effectiveRole === 'system') {
      continue;
    }

    if (effectiveRole === 'user') {
      if (isAnthropicSyntheticSessionContextMessage(message)) {
        flushPendingToolResults();
        applyCacheBreakpoint(anthropicMessages[anthropicMessages.length - 1]);
        hasSyntheticSessionContextTail = true;
      }

      flushPendingToolResults();
      anthropicMessages.push({
        role: 'user',
        content: formatAnthropicContent(message.content as MCPContent[]),
      });
      continue;
    }

    if (effectiveRole === 'assistant') {
      flushPendingToolResults();
      const hasContent = message.content && message.content.length > 0;
      const hasToolCalls = message.tool_calls && message.tool_calls.length > 0;
      const hasToolUse = message.tool_use;

      if (!hasContent && !hasToolCalls && !hasToolUse) {
        logger.debug('Skipping empty assistant message', {
          messageId: message.id,
        });
        continue;
      }

      const content = [];

      if (message.thinking) {
        content.push({
          type: 'thinking' as const,
          thinking: message.thinking,
          signature: message.thinkingSignature || '',
        });
      }

      if (hasContent) {
        const processedContent = processMessageContent(
          message.content as MCPContent[],
        );
        if (processedContent.length > 0) {
          content.push({ type: 'text' as const, text: processedContent });
        }
      }

      if (message.tool_calls) {
        content.push(
          ...message.tool_calls.map((toolCall) => ({
            type: 'tool_use' as const,
            id: toolCall.id,
            name: toolCall.function.name,
            input: parseAnthropicToolInput(toolCall.function.arguments, {
              messageId: message.id,
              toolId: toolCall.id,
              toolName: toolCall.function.name,
            }),
          })),
        );
      } else if (message.tool_use) {
        content.push({
          type: 'tool_use' as const,
          id: message.tool_use.id,
          name: message.tool_use.name,
          input: ensureAnthropicObjectInput(message.tool_use.input, {
            messageId: message.id,
            toolId: message.tool_use.id,
            toolName: message.tool_use.name,
          }),
        });
      }

      if (content.length > 0) {
        anthropicMessages.push({
          role: 'assistant',
          content,
        });
      }
      continue;
    }

    if (effectiveRole === 'tool') {
      if (!message.tool_call_id) {
        logger.warn('Tool message missing tool_call_id, skipping', {
          messageId: message.id,
        });
        continue;
      }

      pendingToolResults.push(
        ...buildAnthropicToolResultBlocks(
          message,
          message.tool_call_id,
          message.id,
          logger,
        ).content,
      );
      continue;
    }

    logger.warn(`Unsupported message role for Anthropic: ${message.role}`);
  }

  flushPendingToolResults();

  const lastStableMessageIndex = hasSyntheticSessionContextTail
    ? anthropicMessages.length - 2
    : anthropicMessages.length - 1;

  applyLongConversationCacheBreakpoint(
    anthropicMessages,
    lastStableMessageIndex,
  );
  applyCacheBreakpoint(anthropicMessages[lastStableMessageIndex]);

  return anthropicMessages;
}
