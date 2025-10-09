import { Message, ToolCall } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useMemo, useState } from 'react';
import { AIServiceConfig, AIServiceFactory } from '../lib/ai-service';
import { getLogger } from '../lib/logger';
import { useSettings } from './use-settings';
import { prepareMessagesForLLM } from '../lib/message-preprocessor';
import { createErrorMessage } from '../lib/ai-service/error-handler';

import { selectMessagesWithinContext } from '@/lib/token-utils';
import { stringToMCPContentArray } from '@/lib/utils';

const logger = getLogger('useAIService');

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

// JSON 필드 안전성 검증 및 escape 처리
const sanitizeJsonField = (value: string): string => {
  try {
    JSON.parse(value);
    return value; // 유효한 JSON이면 그대로 반환
  } catch {
    return JSON.stringify(value); // malformed면 escape된 문자열로 변환
  }
};

// ToolCall 안전성 처리
const sanitizeToolCall = (toolCall: ToolCall): ToolCall => {
  return {
    ...toolCall,
    function: {
      ...toolCall.function,
      arguments: sanitizeJsonField(toolCall.function.arguments),
    },
  };
};

// Message 전체 안전성 처리
const sanitizeMessage = (message: Message): Message => {
  const sanitized = { ...message };

  // tool_calls 처리
  if (sanitized.tool_calls) {
    sanitized.tool_calls = sanitized.tool_calls.map(sanitizeToolCall);
  }

  // thinking 내용 처리
  if (sanitized.thinking) {
    sanitized.thinking = sanitizeJsonField(sanitized.thinking);
  }

  return sanitized;
};

/**
 * Validates that all tool_calls have a corresponding tool response.
 * A valid pair is an assistant message with tool_calls followed immediately by a tool message.
 */
function allToolUsePairsAreValid(messages: Message[]): boolean {
  for (let i = 0; i < messages.length; i++) {
    const message = messages[i];
    if (
      message.role === 'assistant' &&
      message.tool_calls &&
      message.tool_calls.length > 0
    ) {
      const nextMessage = messages[i + 1];
      if (!nextMessage || nextMessage.role !== 'tool') {
        return false; // Found an assistant tool call without a following tool response
      }
    }
  }
  return true;
}

/**
 * Removes incomplete tool_calls/tool response pairs from the message history.
 * It iterates through the messages and removes any assistant messages with tool_calls
 * that are not immediately followed by a tool response message.
 */
function removeInvalidToolUseAndToolResponse(messages: Message[]): Message[] {
  const validMessages: Message[] = [];
  let i = 0;
  while (i < messages.length) {
    const currentMessage = messages[i];
    if (
      currentMessage.role === 'assistant' &&
      currentMessage.tool_calls &&
      currentMessage.tool_calls.length > 0
    ) {
      const nextMessage = messages[i + 1];
      if (nextMessage && nextMessage.role === 'tool') {
        // This is a valid pair, keep both
        validMessages.push(currentMessage);
        validMessages.push(nextMessage);
        i += 2; // Skip the next message as it's part of the pair
      } else {
        // This is a dangling tool call, skip the current message
        logger.debug('Removing dangling tool call message', {
          messageId: currentMessage.id,
        });
        i++;
      }
    } else {
      validMessages.push(currentMessage);
      i++;
    }
  }
  return validMessages;
}

export const useAIService = (config?: AIServiceConfig) => {
  const {
    value: {
      preferredModel: { model, provider },
      serviceConfigs,
    },
  } = useSettings();
  const [response, setResponse] = useState<Message | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const serviceInstance = useMemo(() => {
    const apiKey = serviceConfigs[provider]?.apiKey || '';
    return AIServiceFactory.getService(provider, apiKey, {
      defaultModel: model,
      maxRetries: 3,
      maxTokens: 4096,
      ...config,
    });
  }, [provider, serviceConfigs, model, config]);

  const submit = useCallback(
    async (
      messages: Message[],
      systemPrompt?: string | (() => Promise<string>),
      forceToolUse?: boolean,
    ): Promise<Message> => {
      setIsLoading(true);
      setError(null);
      setResponse(null);

      // Validate that messages exist and have sessionId
      if (!messages.length || !messages[0]?.sessionId) {
        throw new Error(
          'Cannot submit AI request: messages array is empty or missing sessionId',
        );
      }

      let currentResponseId = createId();
      let fullContent = '';
      let thinking = '';
      let thinkingSignature = '';
      let toolCalls: ToolCall[] = [];
      let finalMessage: Message | null = null;

      try {
        // Preprocess messages to include attachment information
        const processedMessages = await prepareMessagesForLLM(messages);

        // Evaluate systemPrompt if it's a function
        let resolvedSystemPrompt: string;
        if (typeof systemPrompt === 'function') {
          resolvedSystemPrompt = await systemPrompt();
        } else {
          resolvedSystemPrompt = systemPrompt || DEFAULT_SYSTEM_PROMPT;
        }

        // Validate and clean up tool use pairs
        let validMessages = processedMessages;
        if (!allToolUsePairsAreValid(validMessages)) {
          logger.warn(
            'Incomplete tool use pairs detected. Cleaning up messages.',
          );
          validMessages = removeInvalidToolUseAndToolResponse(validMessages);
        }

        // Context enforcement: Truncate messages to fit the context window
        const maxTokens = config?.maxTokens ?? 4096;
        const contextMessages = selectMessagesWithinContext(
          validMessages,
          provider,
          model,
          maxTokens,
        );

        // Sanitize messages to prevent malformed JSON
        const safeMessages = contextMessages.map(sanitizeMessage);

        logger.info('Submitting messages to AI service', {
          model,
          systemPrompt: resolvedSystemPrompt,
          messageCount: safeMessages.length, // Log the count of messages being sent
        });

        const stream = serviceInstance.streamChat(safeMessages, {
          modelName: model,
          systemPrompt: resolvedSystemPrompt,
          availableTools: config?.tools || [],
          config: config,
          forceToolUse: forceToolUse,
        });

        for await (const chunk of stream) {
          let parsedChunk: Record<string, unknown>;

          try {
            // Validate and potentially recover the chunk before parsing
            parsedChunk = JSON.parse(chunk);
          } catch {
            parsedChunk = { content: chunk };
          }

          if (parsedChunk.thinking) {
            thinking += parsedChunk.thinking;
          }
          if (parsedChunk.thinkingSignature) {
            thinkingSignature = parsedChunk.thinkingSignature as string;
          }
          if (parsedChunk.tool_calls && Array.isArray(parsedChunk.tool_calls)) {
            (
              parsedChunk.tool_calls as (ToolCall & { index: number })[]
            ).forEach((toolCallChunk: ToolCall & { index: number }) => {
              const { index } = toolCallChunk;
              if (index === undefined) {
                toolCalls.push(toolCallChunk);
                return;
              }

              if (toolCalls[index]) {
                if (toolCallChunk.function?.arguments) {
                  toolCalls[index].function.arguments +=
                    toolCallChunk.function.arguments;
                }
                if (toolCallChunk.id) {
                  toolCalls[index].id = toolCallChunk.id;
                }
              } else {
                toolCalls[index] = toolCallChunk;
              }
            });
            toolCalls = toolCalls.filter(Boolean);
          }
          if (parsedChunk.content) {
            fullContent += parsedChunk.content;
          }

          finalMessage = {
            id: currentResponseId,
            content: stringToMCPContentArray(fullContent),
            role: 'assistant',
            isStreaming: true,
            thinking,
            thinkingSignature,
            tool_calls: toolCalls,
            sessionId: messages[0]?.sessionId,
          };

          setResponse(finalMessage);
        }

        // Check if the response is empty to prevent API errors
        const hasContent = fullContent.trim().length > 0;
        const hasToolCalls = toolCalls.length > 0;

        if (!hasContent && !hasToolCalls) {
          logger.debug('Empty response detected, creating placeholder message');
          finalMessage = {
            id: currentResponseId,
            content: stringToMCPContentArray(
              'I apologize, but I encountered an issue and cannot provide a response at this time.',
            ),
            thinking,
            thinkingSignature,
            role: 'assistant',
            isStreaming: false,
            tool_calls: [],
            sessionId: messages[0]?.sessionId,
          };
        } else {
          finalMessage = {
            id: currentResponseId,
            content: stringToMCPContentArray(fullContent),
            thinking,
            thinkingSignature,
            role: 'assistant',
            isStreaming: false,
            tool_calls: toolCalls,
            sessionId: messages[0]?.sessionId,
          };
        }

        logger.info('Final message:', {
          finalMessage,
          hasContent,
          hasToolCalls,
          contentLength: fullContent.length,
          toolCallsCount: toolCalls.length,
        });
        setResponse(finalMessage);
        return finalMessage!;
      } catch (err) {
        logger.error('Error in useAIService stream:', err);
        setError(err as Error);

        // Create error message instead of malformed content
        const sessionId = messages[0]?.sessionId;
        if (!sessionId) {
          throw new Error(
            'Cannot create error message: missing sessionId in messages',
          );
        }
        const errorMessage = createErrorMessage(
          currentResponseId,
          sessionId,
          err,
          {
            model,
            provider,
            messageCount: messages.length,
          },
        );

        setResponse(errorMessage);
        return errorMessage;
      } finally {
        setIsLoading(false);
      }
    },
    [model, provider, config, serviceInstance],
  );

  const cancel = useCallback(() => {
    logger.info('Cancelling AI service request');
    serviceInstance.cancel();
    setIsLoading(false);
    setError(null);
  }, [serviceInstance]);

  return { response, isLoading, error, submit, cancel };
};
