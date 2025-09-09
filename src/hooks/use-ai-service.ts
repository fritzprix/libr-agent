import { Message, ToolCall } from '@/models/chat';
import { MCPContent } from '@/lib/mcp-types';
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

// 스트리밍 청크 검증 및 복구
const validateStreamChunk = (chunk: string): string => {
  if (!chunk || typeof chunk !== 'string') {
    return '{"content": ""}';
  }

  // 빈 문자열이나 공백만 있는 경우
  if (chunk.trim() === '') {
    return '{"content": ""}';
  }

  // 이미 유효한 JSON인지 확인
  try {
    JSON.parse(chunk);
    return chunk;
  } catch {
    // JSON이 불완전한 경우 복구 시도
    const trimmedChunk = chunk.trim();

    // 중괄호로 시작하지만 끝나지 않는 경우
    if (trimmedChunk.startsWith('{') && !trimmedChunk.endsWith('}')) {
      logger.debug('Incomplete JSON chunk detected, attempting recovery', {
        originalLength: trimmedChunk.length,
        chunk: trimmedChunk.substring(0, 100) + '...',
      });

      // 간단한 복구: 누락된 닫는 중괄호 추가
      const recovered = trimmedChunk + '}';
      try {
        JSON.parse(recovered);
        return recovered;
      } catch {
        // 복구 실패시 안전한 기본값 반환
        return `{"content": ${JSON.stringify(trimmedChunk)}}`;
      }
    }

    // JSON이 아닌 일반 텍스트인 경우
    return `{"content": ${JSON.stringify(trimmedChunk)}}`;
  }
};

// MCPContent 안전성 처리
const sanitizeContent = (content: MCPContent): MCPContent => {
  if (content.type === 'text') {
    return {
      ...content,
      text: sanitizeJsonField(content.text),
    };
  }
  return content; // 다른 타입은 그대로 유지
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

  // content 배열 처리
  if (sanitized.content) {
    sanitized.content = sanitized.content.map(sanitizeContent);
  }

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
            const validatedChunk = validateStreamChunk(chunk);
            parsedChunk = JSON.parse(validatedChunk);
          } catch (parseError) {
            // Final fallback: treat as plain text content
            logger.warn('Failed to parse chunk even after validation', {
              originalChunk: chunk.substring(0, 100) + '...',
              chunkType: typeof chunk,
              error:
                parseError instanceof Error
                  ? parseError.message
                  : 'Unknown parse error',
            });
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
