import { Message, ToolCall } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useMemo, useState } from 'react';
import { AIServiceConfig, AIServiceFactory } from '../lib/ai-service';
import { getLogger } from '../lib/logger';
import { useSettings } from './use-settings';
import { prepareMessagesForLLM } from '../lib/message-preprocessor';

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
    ): Promise<Message> => {
      setIsLoading(true);
      setError(null);
      setResponse(null);

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

        // Context enforcement: Truncate messages to fit the context window
        const contextMessages = selectMessagesWithinContext(
          processedMessages,
          provider,
          model,
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
            sessionId: messages[0]?.sessionId || '', // Add sessionId
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
            sessionId: messages[0]?.sessionId || '',
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
            sessionId: messages[0]?.sessionId || '', // Add sessionId
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
        setResponse((prev) => {
          if (prev) {
            return { ...prev, isStreaming: false, thinking: undefined };
          }
          return null;
        });
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [model, provider, config, serviceInstance],
  );

  return { response, isLoading, error, submit };
};
