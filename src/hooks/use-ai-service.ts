import { Message, ToolCall } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useMemo, useState } from 'react';
import { useAssistantContext } from '../context/AssistantContext';
import { AIServiceConfig, AIServiceFactory } from '../lib/ai-service';
import { getLogger } from '../lib/logger';
import { useScheduledCallback } from './use-scheduled-callback';
import { useSettings } from './use-settings';
import { prepareMessagesForLLM } from '../lib/message-preprocessor';

const logger = getLogger('useAIService');

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

export const useAIService = (config?: AIServiceConfig) => {
  const {
    value: {
      preferredModel: { model, provider },
      apiKeys,
    },
  } = useSettings();
  const [response, setResponse] = useState<Message | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const serviceInstance = useMemo(
    () =>
      AIServiceFactory.getService(provider, apiKeys[provider], {
        defaultModel: model,
        maxRetries: 3,
        maxTokens: 4096,
      }),
    [provider, apiKeys, model],
  );
  const { getCurrent: getCurrentAssistant } = useAssistantContext();


  const submit = useCallback(
    async (messages: Message[]): Promise<Message> => {
      setIsLoading(true);
      setError(null);
      setResponse(null);

      const availableTools = [
      ].filter(Boolean);

      // Get extension services and their tools  

      const allTools = [...availableTools];

      let currentResponseId = createId();
      let fullContent = '';
      let thinking = '';
      let toolCalls: ToolCall[] = [];
      let finalMessage: Message | null = null;


      try {
        // Preprocess messages to include attachment information
        const processedMessages = await prepareMessagesForLLM(messages);

        const stream = serviceInstance.streamChat(processedMessages, {
          modelName: model,
          systemPrompt: [
            getCurrentAssistant()?.systemPrompt || DEFAULT_SYSTEM_PROMPT,
          ].join('\n\n'),
          availableTools: allTools,
          config: config,
        });

        for await (const chunk of stream) {
          const parsedChunk = JSON.parse(chunk);

          if (parsedChunk.thinking) {
            thinking += parsedChunk.thinking;
          }
          if (parsedChunk.tool_calls) {
            parsedChunk.tool_calls.forEach(
              (toolCallChunk: ToolCall & { index: number }) => {
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
              },
            );
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

        finalMessage = {
          id: currentResponseId,
          content: fullContent,
          thinking,
          role: 'assistant',
          isStreaming: false,
          tool_calls: toolCalls,
          sessionId: messages[0]?.sessionId || '', // Add sessionId
        };
        logger.info('message : ', { finalMessage });
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
    [
      model,
      provider,
      apiKeys,
      config,
      serviceInstance,
      getCurrentAssistant
    ],
  );

  const scheduledSubmit = useScheduledCallback(submit, [submit]);

  return { response, isLoading, error, submit: scheduledSubmit };
};
