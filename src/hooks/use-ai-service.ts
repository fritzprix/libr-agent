import { Message, ToolCall } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useMemo, useState } from 'react';
import { AIServiceConfig, AIServiceFactory } from '../lib/ai-service';
import { getLogger } from '../lib/logger';
import { useSettings } from './use-settings';
import { prepareMessagesForLLM } from '../lib/message-preprocessor';

import { selectMessagesWithinContext } from '@/lib/token-utils';

const logger = getLogger('useAIService');

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

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
        const safeMessages = selectMessagesWithinContext(
          processedMessages,
          provider,
          model,
        );

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
            parsedChunk = JSON.parse(chunk);
          } catch {
            // Handle non-JSON chunks (e.g., plain text tool responses)
            logger.debug('Received non-JSON chunk, treating as text content', {
              chunk: chunk.substring(0, 100) + '...',
              chunkType: typeof chunk,
            });
            parsedChunk = { content: chunk };
          }

          if (parsedChunk.thinking) {
            thinking += parsedChunk.thinking;
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
            content: fullContent,
            role: 'assistant',
            isStreaming: true,
            thinking,
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
            content:
              'I apologize, but I encountered an issue and cannot provide a response at this time.',
            thinking,
            role: 'assistant',
            isStreaming: false,
            tool_calls: [],
            sessionId: messages[0]?.sessionId || '',
          };
        } else {
          finalMessage = {
            id: currentResponseId,
            content: fullContent,
            thinking,
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
