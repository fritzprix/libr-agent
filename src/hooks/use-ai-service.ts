import { Message, ToolCall } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useMemo, useState } from 'react';
import {
  AIServiceConfig,
  AIServiceFactory,
  AIServiceProvider,
} from '../lib/ai-service';
import { getLogger } from '../lib/logger';
import { useSettings } from './use-settings';
import { prepareMessagesForLLM } from '../lib/message-preprocessor';
import {
  createErrorMessage,
  classifyAIServiceError,
} from '../lib/ai-service/error-handler';

import { selectMessagesWithinContext } from '@/lib/token-utils';
import { stringToMCPContentArray } from '@/lib/utils';
import { deduplicateToolCallPairs } from '@/lib/message-deduplicator';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';

const logger = getLogger('useAIService');

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

// Throttling constants
const MIN_REQUEST_INTERVAL = 500; // 0.5s minimum interval between requests
let lastRequestTime = 0;

const throttleRequest = async () => {
  const now = Date.now();
  const timeSinceLastRequest = now - lastRequestTime;
  if (timeSinceLastRequest < MIN_REQUEST_INTERVAL) {
    const waitTime = MIN_REQUEST_INTERVAL - timeSinceLastRequest;
    logger.debug(`Throttling request for ${waitTime}ms`);
    await new Promise((resolve) => setTimeout(resolve, waitTime));
  }
  lastRequestTime = Date.now();
};

// Types for completeText - simple one-shot text generation
type CompleteTextOptions = {
  model?: string;
  systemPrompt?: string;
  maxTokens?: number;
  onProgress?: (partial: string, isFinal?: boolean) => void;
};

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
      maxRetries: 0, // Disable internal retries in favor of controlled 429 retry
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

      // Apply client-side throttling
      await throttleRequest();

      let currentResponseId = createId();
      let fullContent = '';
      let thinking = '';
      let thinkingSignature = '';
      let toolCalls: ToolCall[] = [];
      let finalMessage: Message | null = null;

      // Retry loop for handling 429 Rate Limit errors
      let retryCount = 0;
      const MAX_RETRIES_FOR_429 = 1;
      const RETRY_DELAY_MS = 5000;

      while (true) {
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

          // Deduplicate repeated tool call/response pairs (errors AND successes)
          const deduplicatedMessages = deduplicateToolCallPairs(validMessages, {
            preserveRecentN: 3,
            minMessageCount: 10,
          });

          // Context enforcement: Truncate messages to fit the context window
          const maxTokens = config?.maxTokens ?? 4096;

          // Prepare tools JSON for token estimation
          const toolsJson = config?.tools?.length
            ? JSON.stringify(config.tools)
            : undefined;

          const contextMessages = selectMessagesWithinContext(
            deduplicatedMessages,
            provider,
            model,
            maxTokens,
            {
              systemPrompt: resolvedSystemPrompt,
              toolsJson,
            },
          );

          // Sanitize messages to prevent malformed JSON and ensure provider compatibility
          const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
            contextMessages.map(sanitizeMessage),
            provider as unknown as AIServiceProvider,
          );

          logger.info('Submitting messages to AI service', {
            model,
            systemPrompt: resolvedSystemPrompt,
            messageCount: safeMessages.length,
            retryCount,
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
            if (
              parsedChunk.tool_calls &&
              Array.isArray(parsedChunk.tool_calls)
            ) {
              // Handle both complete tool calls (Ollama, Gemini) and incremental chunks (OpenAI, Anthropic)
              (
                parsedChunk.tool_calls as (ToolCall & { index?: number })[]
              ).forEach((toolCallChunk: ToolCall & { index?: number }) => {
                const { index } = toolCallChunk;

                // If no index provided, treat as a complete tool call (Ollama/Gemini pattern)
                if (index === undefined) {
                  toolCalls.push(toolCallChunk);
                  return;
                }

                // Index-based merging for providers that send incremental chunks
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
              threadId: messages[0]?.threadId || messages[0]?.sessionId,
            };

            setResponse(finalMessage);
          }

          // Check if the response is empty to prevent API errors
          const hasContent = fullContent.trim().length > 0;
          const hasToolCalls = toolCalls.length > 0;

          if (!hasContent && !hasToolCalls && !thinking) {
            logger.debug('Empty response detected, creating error message');
            // Create a specific error for empty response so it can be handled by the UI
            const emptyResponseError = new Error('AI_SERVICE_EMPTY_RESPONSE');
            const sessionId = messages[0]?.sessionId;
            const threadId = messages[0]?.threadId || messages[0]?.sessionId;

            finalMessage = createErrorMessage(
              currentResponseId,
              sessionId,
              threadId,
              emptyResponseError,
              { model, provider },
            );
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
              threadId: messages[0]?.threadId || messages[0]?.sessionId,
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

          // Handle 429 Rate Limit Retry
          const errorClassification = classifyAIServiceError(err);
          if (
            errorClassification.type === 'RATE_LIMIT_ERROR' &&
            retryCount < MAX_RETRIES_FOR_429
          ) {
            logger.warn(
              `Rate limit exceeded. Retrying in ${RETRY_DELAY_MS}ms... (Attempt ${retryCount + 1}/${MAX_RETRIES_FOR_429})`,
            );
            retryCount++;
            await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
            // Reset state for retry
            fullContent = '';
            thinking = '';
            thinkingSignature = '';
            toolCalls = [];
            finalMessage = null;
            continue;
          }

          setError(err as Error);

          // Create error message instead of malformed content
          const sessionId = messages[0]?.sessionId;
          if (!sessionId) {
            throw new Error(
              'Cannot create error message: missing sessionId in messages',
            );
          }
          const threadId = messages[0]?.threadId || sessionId;
          const errorMessage = createErrorMessage(
            currentResponseId,
            sessionId,
            threadId,
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
      }
    },
    [model, provider, config, serviceInstance],
  );

  /**
   * Simple text completion without history or tools.
   * Generates a single response for the given prompt with no context or tool usage.
   * @param prompt - The text prompt to generate a response for
   * @param options - Optional configuration (model, systemPrompt, maxTokens, onProgress)
   * @returns Promise<Message> - The final assistant message with generated text
   */
  const completeText = useCallback(
    async (prompt: string, options?: CompleteTextOptions): Promise<Message> => {
      setIsLoading(true);
      setError(null);
      setResponse(null);

      // Retry loop for handling 429 Rate Limit errors
      let retryCount = 0;
      const MAX_RETRIES_FOR_429 = 1;
      const RETRY_DELAY_MS = 5000;

      while (true) {
        const responseId = createId();
        const ephemeralSessionId = createId(); // No persistent session needed
        let fullContent = '';
        let thinking = '';
        let thinkingSignature = '';

        try {
          // Default system prompt enforces plain text generation without tools
          const systemPrompt =
            options?.systemPrompt ??
            'Only produce plain text. Do not call or reference any tools or external APIs. Provide the answer directly.';

          // Build minimal message array: system + user prompt only (no history)
          const messages: Message[] = [
            {
              id: createId(),
              role: 'system',
              content: stringToMCPContentArray(systemPrompt),
              sessionId: ephemeralSessionId,
              threadId: ephemeralSessionId, // Use ephemeral session as thread
            },
            {
              id: createId(),
              role: 'user',
              content: stringToMCPContentArray(prompt),
              sessionId: ephemeralSessionId,
              threadId: ephemeralSessionId, // Use ephemeral session as thread
            },
          ];

          // Sanitize messages (defensive, though these are newly created)
          const safeMessages = messages.map(sanitizeMessage);

          logger.info('completeText: submitting single-prompt completion', {
            messages: safeMessages,
            promptLength: prompt.length,
            retryCount,
          });

          // Apply client-side throttling
          await throttleRequest();

          // Call streamChat with no tools
          const stream = serviceInstance.streamChat(safeMessages, {
            modelName: options?.model ?? model,
            systemPrompt,
            availableTools: [], // Never include tools for text completion
            config: { ...(config || {}), maxTokens: options?.maxTokens },
            forceToolUse: false,
          });

          // Process stream chunks
          for await (const chunk of stream) {
            let parsedChunk: Record<string, unknown>;
            try {
              parsedChunk = JSON.parse(chunk);
            } catch {
              parsedChunk = { content: String(chunk) };
            }

            if (parsedChunk.thinking) {
              thinking += parsedChunk.thinking as string;
            }
            if (parsedChunk.thinkingSignature) {
              thinkingSignature = parsedChunk.thinkingSignature as string;
            }
            // Defensive: ignore tool_calls in completeText mode
            if (parsedChunk.content) {
              fullContent += parsedChunk.content as string;
            }

            // Update progress callback
            options?.onProgress?.(fullContent, false);

            // Update streaming response state
            setResponse({
              id: responseId,
              role: 'assistant',
              content: stringToMCPContentArray(fullContent),
              isStreaming: true,
              thinking,
              thinkingSignature,
              sessionId: ephemeralSessionId,
              threadId: ephemeralSessionId, // Use ephemeral session as thread
            });
          }

          // Finalize response
          const finalContent = fullContent.trim();

          if (!finalContent && !thinking) {
            const emptyResponseError = new Error('AI_SERVICE_EMPTY_RESPONSE');
            const finalMessage = createErrorMessage(
              responseId,
              ephemeralSessionId,
              ephemeralSessionId,
              emptyResponseError,
              { model, provider },
            );
            setResponse(finalMessage);
            return finalMessage;
          }

          const finalMessage: Message = {
            id: responseId,
            role: 'assistant',
            content: stringToMCPContentArray(finalContent || ' '), // Fallback for pure thinking response
            isStreaming: false,
            thinking,
            thinkingSignature,
            sessionId: ephemeralSessionId,
            threadId: ephemeralSessionId, // Use ephemeral session as thread
          };

          options?.onProgress?.(finalContent, true);
          setResponse(finalMessage);
          return finalMessage;
        } catch (err) {
          logger.error('Error in completeText:', err);

          // Handle 429 Rate Limit Retry
          const errorClassification = classifyAIServiceError(err);
          if (
            errorClassification.type === 'RATE_LIMIT_ERROR' &&
            retryCount < MAX_RETRIES_FOR_429
          ) {
            logger.warn(
              `Rate limit exceeded in completeText. Retrying in ${RETRY_DELAY_MS}ms... (Attempt ${retryCount + 1}/${MAX_RETRIES_FOR_429})`,
            );
            retryCount++;
            await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
            continue;
          }

          setError(err as Error);

          // Create error message with text content for text completion
          const errorClassificationForMessage = classifyAIServiceError(err, {
            model: options?.model ?? model,
            provider,
            messageCount: 1,
          });

          const errorMessage: Message = {
            id: responseId,
            content: stringToMCPContentArray(
              'I apologize, but I encountered an error while processing your request.',
            ),
            role: 'assistant',
            isStreaming: false,
            thinking: '',
            thinkingSignature: '',
            tool_calls: [],
            sessionId: ephemeralSessionId,
            threadId: ephemeralSessionId, // Use ephemeral session as thread
            error: errorClassificationForMessage,
          };

          setResponse(errorMessage);
          return errorMessage;
        } finally {
          setIsLoading(false);
        }
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

  return { response, isLoading, error, submit, cancel, completeText };
};
