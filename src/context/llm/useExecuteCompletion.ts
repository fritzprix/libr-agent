import { useCallback, useEffect, useRef } from 'react';

import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import { reportLLMStreamingIssue } from '@/lib/backend/agent-commands';
import type {
  AIServiceConfig,
  AICompletionExecutionService,
  TokenUsage,
} from '@/lib/ai-service/types';
import {
  isParsedDirectToolCall,
  isParsedIndexedToolCallDelta,
  parseStreamChunk,
} from '@/lib/ai-service/stream-events';
import { getLogger } from '@/lib/logger';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import type { SessionStatus } from './types';
import { isAbortError } from './types';
import type { Message, MessageError, ToolCall } from '@/models/chat';
import type {
  MCPTool,
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp';
import type { Settings } from '@/lib/services/settings-service';
import { buildServiceRuntimeConfig } from './service-runtime-config';
import { isSupersededRequestError } from './types';
import {
  buildStreamingMessage,
  extractThinkingText,
  extractToolCalls,
  hasRenderableAssistantOutput,
} from './streaming-message-utils';
import { detectRepeatedThinkingLoop } from './repeatedThinkingDetector';
import { detectRepeatedTextLoop } from './repeatedTailDetector';

const logger = getLogger('useExecuteCompletion');
const STREAMING_TEXT_THROTTLE_MS = 50;
const STREAMING_TOOL_CALL_THROTTLE_MS = 100;
const REPEATED_TAIL_CHECK_INTERVAL = 5;
type RequestTerminationReason = 'aborted' | 'superseded';

function createExecutionError(
  type: MessageError['type'],
  displayMessage: string,
  originalError: unknown,
  context?: Record<string, unknown>,
): MessageError {
  return {
    type,
    displayMessage,
    recoverable: true,
    details: {
      originalError,
      timestamp: new Date().toISOString(),
      context,
    },
  };
}

interface UseExecuteCompletionProps {
  settingsRef: React.MutableRefObject<Settings>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => void;
}

export function useExecuteCompletion({
  settingsRef,
  setStreamingMessages,
  updateSessionStatus,
}: UseExecuteCompletionProps) {
  // Track which shared service instance is currently bound to each session
  const activeServicesRef = useRef<Map<string, AICompletionExecutionService>>(
    new Map(),
  );
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  // Track last streaming UI update time per session (throttle to ~20fps)
  const lastStreamingUpdateRef = useRef<Map<string, number>>(new Map());
  // Track the latest request identity per session to suppress stale stream updates/results
  const activeRequestIdsRef = useRef<Map<string, string>>(new Map());
  // Track explicit request termination reasons so late chunks cannot "win" races.
  const terminatedRequestsRef = useRef<Map<string, RequestTerminationReason>>(
    new Map(),
  );

  // Context window usage per session for gauge display

  // Clean up on unmount
  useEffect(() => {
    return () => {
      abortControllersRef.current.forEach((controller) => controller.abort());
      abortControllersRef.current.clear();

      timeoutsRef.current.forEach((timeoutId) =>
        window.clearTimeout(timeoutId),
      );
      timeoutsRef.current.clear();

      activeServicesRef.current.clear();
      activeRequestIdsRef.current.clear();
      terminatedRequestsRef.current.clear();
    };
  }, []);

  const getRequestKey = useCallback(
    (sessionId: string, responseMessageId: string) =>
      `${sessionId}:${responseMessageId}`,
    [],
  );

  const getRequestTerminationReason = useCallback(
    (sessionId: string, responseMessageId: string) =>
      terminatedRequestsRef.current.get(
        getRequestKey(sessionId, responseMessageId),
      ),
    [getRequestKey],
  );

  const isCurrentRequest = useCallback(
    (sessionId: string, responseMessageId: string) =>
      activeRequestIdsRef.current.get(sessionId) === responseMessageId,
    [],
  );

  const executeCompletionRequest = useCallback(
    async (
      sessionId: string,
      responseMessageId: string,
      messages: Message[],
      model: string,
      provider: string,
      apiKey?: string,
      systemPrompt?: string,
      temperature?: number,
      maxTokens?: number,
      availableTools?: MCPTool[],
    ): Promise<Message> => {
      logger.info('🚀 Executing completion request', {
        sessionId,
        messageCount: messages.length,
        provider,
        model,
        temperature,
        maxTokens,
        toolCount: availableTools?.length ?? 0,
        firstMessageId: messages[0]?.id ?? 'none',
        lastMessageId: messages[messages.length - 1]?.id ?? 'none',
      });

      // Cancel any previously active request for this session.
      // Shared service instances are factory-owned and must not be disposed here.
      terminatedRequestsRef.current.delete(
        getRequestKey(sessionId, responseMessageId),
      );
      const previousRequestId = activeRequestIdsRef.current.get(sessionId);
      const previousController = abortControllersRef.current.get(sessionId);
      if (previousController && previousRequestId) {
        terminatedRequestsRef.current.set(
          getRequestKey(sessionId, previousRequestId),
          'superseded',
        );
        previousController.abort();
      }
      terminatedRequestsRef.current.delete(
        getRequestKey(sessionId, responseMessageId),
      );
      activeServicesRef.current.delete(sessionId);
      const previousTimeoutId = timeoutsRef.current.get(sessionId);
      if (previousTimeoutId) {
        clearTimeout(previousTimeoutId);
        timeoutsRef.current.delete(sessionId);
      }
      activeRequestIdsRef.current.set(sessionId, responseMessageId);

      // Create abort controller for this request
      const abortController = new AbortController();
      abortControllersRef.current.set(sessionId, abortController);

      // Create service instance via factory using the provider/apiKey from this request
      const providerConfig =
        settingsRef.current.serviceConfigs?.[provider as AIServiceProvider] ||
        {};
      const service = AIServiceFactory.getService(
        provider as AIServiceProvider,
        apiKey ?? '',
        providerConfig,
      );
      const runtimeConfig = buildServiceRuntimeConfig(
        settingsRef.current,
        providerConfig,
      );
      activeServicesRef.current.set(sessionId, service);

      // Get existing streaming message (already set by event listener)
      const streamingMessage: Partial<Message> = {
        id: responseMessageId,
        sessionId,
        threadId: sessionId,
        role: 'assistant',
        content: [],
        createdAt: new Date(),
      };

      try {
        // Build config
        const config: AIServiceConfig = {
          ...runtimeConfig,
          maxTokens:
            maxTokens ||
            settingsRef.current.advanced?.defaultMaxOutputTokens ||
            8192,
          temperature: temperature,
        };

        // ── Prepare context messages based on selected strategy ─────────────
        let enrichedMessages: Message[];

        // Process the pre-sliced messages provided by the Rust backend.
        const safeMessages = MessageNormalizer.sanitizeMessagesForService(
          messages.map(sanitizeMessage),
          service,
        );
        logger.info('✅ Messages sanitized for provider compatibility', {
          sessionId,
          safeCount: safeMessages.length,
        });

        enrichedMessages = await prepareMessagesForLLM(safeMessages);

        // ── Execute Stream ───────────────────────────────────────────────────
        updateSessionStatus(sessionId, 'streaming');

        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, {
            id: responseMessageId,
            sessionId,
            threadId: sessionId,
            role: 'assistant',
            content: [],
            createdAt: new Date(),
            isStreaming: true,
          });
          return next;
        });

        const content: MCPContent[] = [];
        const activeToolCallIndices = new Map<number, number>();
        const indexedToolCalls = new Map<number, ToolCall>();
        const directToolCalls: ToolCall[] = [];

        let thinkingStartTime: number | undefined;
        let currentThinkingTime: number | undefined;
        let currentThinkingText: string | undefined;
        let finalUsage: TokenUsage | undefined;
        let firstChunkTime: number | undefined;
        let thinkingSignature: string | undefined;
        let repeatedThinkingIssueReported = false;
        let repeatedThinkingCheckCounter = 0;
        let currentStreamingText = '';
        let hasToolCallInStream = false;
        let repeatedTextIssueReported = false;
        let repeatedTextCheckCounter = 0;

        const startTime = performance.now();
        const ensureRequestStillActive = (phase: string) => {
          const terminationReason = getRequestTerminationReason(
            sessionId,
            responseMessageId,
          );
          const activeRequestId = activeRequestIdsRef.current.get(sessionId);

          if (terminationReason === 'superseded') {
            logger.info(`Dropping ${phase} for superseded request`, {
              sessionId,
              responseMessageId,
              activeRequestId,
            });
            throw new Error('Request superseded');
          }

          if (terminationReason === 'aborted') {
            logger.warn(`Completion request aborted during ${phase}`, {
              sessionId,
              responseMessageId,
            });
            throw new Error('Request aborted');
          }

          if (!isCurrentRequest(sessionId, responseMessageId)) {
            if (
              activeRequestId !== undefined &&
              activeRequestId !== responseMessageId
            ) {
              logger.info(`Dropping ${phase} for superseded request`, {
                sessionId,
                responseMessageId,
                activeRequestId,
              });
              throw new Error('Request superseded');
            }

            if (abortController.signal.aborted) {
              logger.warn(`Completion request aborted during ${phase}`, {
                sessionId,
                responseMessageId,
              });
              throw new Error('Request aborted');
            }

            logger.info(`Dropping ${phase} for inactive request`, {
              sessionId,
              responseMessageId,
              activeRequestId,
            });
            throw new Error('Request superseded');
          }

          if (abortController.signal.aborted) {
            logger.warn(`Completion request aborted during ${phase}`, {
              sessionId,
              responseMessageId,
            });
            throw new Error('Request aborted');
          }
        };

        const streamGenerator = service.streamChat(enrichedMessages, {
          modelName: model,
          systemPrompt,
          availableTools: availableTools || [],
          config,
          forceToolUse: false,
          signal: abortController.signal,
        });

        for await (const rawChunk of streamGenerator) {
          ensureRequestStillActive('stream processing');

          if (firstChunkTime === undefined) {
            firstChunkTime = performance.now();
          }

          const chunk = parseStreamChunk(rawChunk);

          // 1. Accumulate Text
          if (typeof chunk.content === 'string') {
            const lastItem = content[content.length - 1];
            if (lastItem && lastItem.type === 'text') {
              (lastItem as MCPTextContent).text += chunk.content;
            } else {
              content.push({ type: 'text', text: chunk.content });
            }

            currentStreamingText += chunk.content;

            if (!hasToolCallInStream && !repeatedTextIssueReported) {
              repeatedTextCheckCounter += 1;
              const textDetection =
                repeatedTextCheckCounter % REPEATED_TAIL_CHECK_INTERVAL === 0 &&
                currentStreamingText
                  ? detectRepeatedTextLoop(currentStreamingText)
                  : null;
              if (textDetection) {
                repeatedTextIssueReported = true;
                logger.warn('Detected repeated text pattern during streaming', {
                  sessionId,
                  responseMessageId,
                  ...textDetection,
                });
                void reportLLMStreamingIssue({
                  sessionId,
                  responseMessageId,
                  issueKind: 'REPEATED_TEXT_LOOP',
                  observedTailChars: textDetection.observedTailChars,
                  patternLength: textDetection.patternLength,
                  repetitionCount: textDetection.repetitionCount,
                }).catch((error: unknown) => {
                  logger.warn('Failed to report repeated text pattern', {
                    sessionId,
                    responseMessageId,
                    error,
                  });
                });
              }
            }
          }

          // 2. Accumulate Thinking
          if (typeof chunk.thinking === 'string') {
            if (thinkingStartTime === undefined) {
              thinkingStartTime = performance.now();
            }
            const lastItem = content[content.length - 1];
            const appendedToExistingThinkingBlock =
              !!lastItem && lastItem.type === 'thinking';
            if (lastItem && lastItem.type === 'thinking') {
              (lastItem as MCPThinkingContent).thinking += chunk.thinking;
            } else {
              content.push({ type: 'thinking', thinking: chunk.thinking });
            }

            currentThinkingText = appendedToExistingThinkingBlock
              ? `${currentThinkingText ?? ''}${chunk.thinking}` || undefined
              : currentThinkingText
                ? `${currentThinkingText}\n${chunk.thinking}`
                : chunk.thinking;

            if (!repeatedThinkingIssueReported) {
              repeatedThinkingCheckCounter += 1;
              const detection =
                repeatedThinkingCheckCounter % REPEATED_TAIL_CHECK_INTERVAL ===
                  0 && currentThinkingText
                  ? detectRepeatedThinkingLoop(currentThinkingText)
                  : null;
              if (detection) {
                repeatedThinkingIssueReported = true;
                logger.warn(
                  'Detected repeated thinking pattern during streaming',
                  {
                    sessionId,
                    responseMessageId,
                    ...detection,
                  },
                );
                void reportLLMStreamingIssue({
                  sessionId,
                  responseMessageId,
                  issueKind: 'REPEATED_THINKING_LOOP',
                  observedTailChars: detection.observedTailChars,
                  patternLength: detection.patternLength,
                  repetitionCount: detection.repetitionCount,
                }).catch((error: unknown) => {
                  logger.warn('Failed to report repeated thinking pattern', {
                    sessionId,
                    responseMessageId,
                    error,
                  });
                });
              }
            }
          }

          // 3. Accumulate Thinking Signature
          if (typeof chunk.thinkingSignature === 'string') {
            thinkingSignature = chunk.thinkingSignature;
          }

          // 4. Accumulate Tool Calls
          const toolCallStartChunks = chunk.tool_call_starts ?? [];
          const toolCallDeltaChunks = chunk.tool_calls ?? [];
          const toolCallChunks = [
            ...toolCallStartChunks,
            ...toolCallDeltaChunks,
          ];
          const hasToolCallUpdate = toolCallChunks.length > 0;
          const previousToolCallCount =
            indexedToolCalls.size + directToolCalls.length;

          if (hasToolCallUpdate) {
            hasToolCallInStream = true;
            toolCallChunks.forEach((toolCallChunk) => {
              if (isParsedIndexedToolCallDelta(toolCallChunk)) {
                const { index } = toolCallChunk;

                if (activeToolCallIndices.has(index)) {
                  const contentIndex = activeToolCallIndices.get(index)!;
                  const targetBlock = content[
                    contentIndex
                  ] as MCPToolCallContent;
                  const existingToolCall = indexedToolCalls.get(index) ?? {
                    id: targetBlock.id,
                    type: 'function' as const,
                    function: {
                      name: targetBlock.name,
                      arguments: targetBlock.arguments,
                    },
                  };
                  if (toolCallChunk.id && !targetBlock.id) {
                    targetBlock.id = toolCallChunk.id;
                  }
                  if (toolCallChunk.function?.name) {
                    if (!targetBlock.name) {
                      targetBlock.name = toolCallChunk.function.name;
                    } else if (
                      targetBlock.name !== toolCallChunk.function.name &&
                      !toolCallChunk.function.name.startsWith(targetBlock.name)
                    ) {
                      targetBlock.name = toolCallChunk.function.name;
                    }
                  }
                  if (toolCallChunk.function?.arguments) {
                    targetBlock.arguments += toolCallChunk.function.arguments;
                  }

                  indexedToolCalls.set(index, {
                    id: targetBlock.id || existingToolCall.id,
                    type: 'function',
                    function: {
                      name: targetBlock.name || existingToolCall.function.name,
                      arguments:
                        targetBlock.arguments ||
                        existingToolCall.function.arguments,
                    },
                  });
                } else {
                  const newBlock: MCPToolCallContent = {
                    type: 'tool_call',
                    id: toolCallChunk.id || '',
                    name: toolCallChunk.function?.name || '',
                    arguments: toolCallChunk.function?.arguments || '',
                  };
                  content.push(newBlock);
                  activeToolCallIndices.set(index, content.length - 1);
                  indexedToolCalls.set(index, {
                    id: newBlock.id,
                    type: 'function',
                    function: {
                      name: newBlock.name,
                      arguments: newBlock.arguments,
                    },
                  });
                }
                return;
              }

              if (isParsedDirectToolCall(toolCallChunk)) {
                const directToolCall: ToolCall = {
                  id: toolCallChunk.id,
                  type: 'function',
                  function: {
                    name: toolCallChunk.function.name,
                    arguments: toolCallChunk.function.arguments,
                  },
                };
                content.push({
                  type: 'tool_call',
                  id: directToolCall.id,
                  name: directToolCall.function.name,
                  arguments: directToolCall.function.arguments,
                });
                directToolCalls.push(directToolCall);
              }
            });
          }

          const shouldFlushToolCallImmediately =
            hasToolCallUpdate &&
            indexedToolCalls.size + directToolCalls.length >
              previousToolCallCount;

          if (thinkingStartTime !== undefined) {
            currentThinkingTime =
              (performance.now() - thinkingStartTime) / 1000;
          }

          if (chunk.usage) {
            const incomingUsage = chunk.usage;
            if (finalUsage) {
              finalUsage = {
                // Use || to prevent 0 values in delta chunks from overwriting cumulative totals
                promptTokens:
                  incomingUsage.promptTokens || finalUsage.promptTokens,
                completionTokens:
                  incomingUsage.completionTokens || finalUsage.completionTokens,
                totalTokens:
                  incomingUsage.totalTokens || finalUsage.totalTokens,
                cachedPromptTokens:
                  incomingUsage.cachedPromptTokens ??
                  finalUsage.cachedPromptTokens,
                details: {
                  ...finalUsage.details,
                  ...incomingUsage.details,
                },
              };
            } else {
              // Normalise the first usage chunk — ensure required numeric fields are always numbers
              finalUsage = {
                promptTokens: incomingUsage.promptTokens ?? 0,
                completionTokens: incomingUsage.completionTokens ?? 0,
                totalTokens: incomingUsage.totalTokens ?? 0,
                cachedPromptTokens: incomingUsage.cachedPromptTokens,
                details: incomingUsage.details,
              };
            }
          }

          // Update real-time duration for TPS calculation
          if (finalUsage) {
            if (!finalUsage.details) finalUsage.details = {};
            const currentTime = performance.now();
            // If we have the first chunk time, use it to measure actual generation duration
            // otherwise measure from the start of the call
            finalUsage.details.evalDuration =
              currentTime - (firstChunkTime || startTime);
          }

          // Throttle React state updates to ~20fps (50ms) to avoid WebView GPU overload
          const nowMs = performance.now();
          const lastUpdateMs =
            lastStreamingUpdateRef.current.get(sessionId) ?? 0;
          const throttleMs = hasToolCallUpdate
            ? STREAMING_TOOL_CALL_THROTTLE_MS
            : STREAMING_TEXT_THROTTLE_MS;
          if (
            shouldFlushToolCallImmediately ||
            nowMs - lastUpdateMs >= throttleMs
          ) {
            lastStreamingUpdateRef.current.set(sessionId, nowMs);
            const streamingToolCalls = [
              ...[...indexedToolCalls.entries()]
                .sort(([leftIndex], [rightIndex]) => leftIndex - rightIndex)
                .map(([, toolCall]) => toolCall),
              ...directToolCalls,
            ];
            setStreamingMessages((prev) => {
              if (!isCurrentRequest(sessionId, responseMessageId)) {
                return prev;
              }
              const next = new Map(prev);
              next.set(
                sessionId,
                buildStreamingMessage(
                  streamingMessage,
                  content,
                  thinkingSignature,
                  currentThinkingTime,
                  finalUsage,
                  {
                    toolCalls: streamingToolCalls,
                    thinkingText: currentThinkingText,
                  },
                ),
              );
              return next;
            });
          }
        }

        // Always flush final streaming state after loop ends
        lastStreamingUpdateRef.current.delete(sessionId);
        const streamingToolCalls = [
          ...[...indexedToolCalls.entries()]
            .sort(([leftIndex], [rightIndex]) => leftIndex - rightIndex)
            .map(([, toolCall]) => toolCall),
          ...directToolCalls,
        ];
        setStreamingMessages((prev) => {
          if (!isCurrentRequest(sessionId, responseMessageId)) {
            return prev;
          }
          const next = new Map(prev);
          next.set(
            sessionId,
            buildStreamingMessage(
              streamingMessage,
              content,
              thinkingSignature,
              currentThinkingTime,
              finalUsage,
              {
                toolCalls: streamingToolCalls,
                thinkingText: currentThinkingText,
              },
            ),
          );
          return next;
        });

        ensureRequestStillActive('stream finalization');

        const endTime = performance.now();
        const totalDurationMs = endTime - startTime;

        if (finalUsage && finalUsage.completionTokens > 0) {
          if (!finalUsage.details) {
            finalUsage.details = {};
          }
          if (!finalUsage.details.evalDuration) {
            if (firstChunkTime) {
              finalUsage.details.promptEvalDuration =
                firstChunkTime - startTime;
              finalUsage.details.evalDuration = endTime - firstChunkTime;
            } else {
              finalUsage.details.evalDuration = totalDurationMs;
            }
          }
          if (!finalUsage.details.timeToFirstToken && firstChunkTime) {
            finalUsage.details.timeToFirstToken = firstChunkTime - startTime;
          }
        }

        const finalToolCalls: ToolCall[] =
          streamingToolCalls.length > 0
            ? streamingToolCalls
            : extractToolCalls(content);
        const finalThinking =
          currentThinkingText ?? extractThinkingText(content);

        const finalMessage: Message = {
          id: responseMessageId,
          sessionId,
          threadId: sessionId,
          role: 'assistant',
          content,
          createdAt: new Date(),
          tool_calls: finalToolCalls.length > 0 ? finalToolCalls : undefined,
          thinking: finalThinking,
          thinkingSignature,
          thinkingTime: thinkingStartTime
            ? (performance.now() - thinkingStartTime) / 1000
            : undefined,
          usage: finalUsage,
          isStreaming: false,
        };

        logger.info('Completion request completed', {
          sessionId,
          contentLength: content.length,
          toolCallCount: finalToolCalls.length,
          finalUsage: finalUsage
            ? {
                promptTokens: finalUsage.promptTokens,
                completionTokens: finalUsage.completionTokens,
                totalTokens: finalUsage.totalTokens,
                cachedPromptTokens: finalUsage.cachedPromptTokens,
                details: finalUsage.details,
              }
            : undefined,
        });

        const hasContent = hasRenderableAssistantOutput(finalMessage);

        const hasUsage =
          finalMessage.usage && finalMessage.usage.completionTokens > 0;

        if (!hasContent && !hasUsage) {
          logger.error('❌ Empty response detected', {
            sessionId,
            finalMessage: {
              ...finalMessage,
              content: finalMessage.content?.length,
            },
            hasContent,
            hasUsage,
          });
          throw createExecutionError(
            'AI_SERVICE_ERROR',
            'Received empty response from LLM provider',
            'empty_response_from_provider',
            { sessionId },
          );
        } else if (!hasContent && hasUsage) {
          // Usage tokens were charged but no content was returned (e.g. Gemini context
          // overflow returning [{type:"text",text:""}]).  This is still an invalid
          // response — route through the normal error/retry path instead of forwarding
          // an empty message to Rust (which would cause a WorkflowError directly).
          logger.warn(
            '⚠️ Response has usage but no content - treating as empty response',
            {
              sessionId,
              usage: finalMessage.usage,
            },
          );
          throw createExecutionError(
            'AI_SERVICE_ERROR',
            'Received empty response from LLM provider',
            'empty_response_from_provider',
            { sessionId },
          );
        }

        ensureRequestStillActive('final message commit');

        // 1. Update with isStreaming: false IMMEDIATELY
        setStreamingMessages((prev) => {
          if (!isCurrentRequest(sessionId, responseMessageId)) {
            return prev;
          }
          const next = new Map(prev);
          next.set(sessionId, finalMessage);
          return next;
        });

        // 2. Schedule cleanup with 500ms delay to allow backend event to arrive
        const timeoutId = window.setTimeout(() => {
          setStreamingMessages((prev) => {
            if (!isCurrentRequest(sessionId, responseMessageId)) {
              return prev;
            }
            const next = new Map(prev);
            // Only delete if it's still the same message (same ID)
            const current = next.get(sessionId);
            if (current?.id === finalMessage.id) {
              next.delete(sessionId);
            }
            return next;
          });
          timeoutsRef.current.delete(sessionId);
        }, 500);
        timeoutsRef.current.set(sessionId, timeoutId);

        ensureRequestStillActive('request completion');

        updateSessionStatus(sessionId, 'idle');

        if (activeRequestIdsRef.current.get(sessionId) === responseMessageId) {
          activeRequestIdsRef.current.delete(sessionId);
        }
        if (abortControllersRef.current.get(sessionId) === abortController) {
          abortControllersRef.current.delete(sessionId);
        }
        if (activeServicesRef.current.get(sessionId) === service) {
          activeServicesRef.current.delete(sessionId);
        }
        terminatedRequestsRef.current.delete(
          getRequestKey(sessionId, responseMessageId),
        );

        return finalMessage;
      } catch (error) {
        const isAborted = isAbortError(error);
        const isSuperseded = isSupersededRequestError(error);

        if (isAborted || isSuperseded) {
          logger.info('Completion request ended without surfacing an error', {
            sessionId,
            responseMessageId,
            reason: isSuperseded ? 'superseded' : 'aborted',
          });
        } else {
          logger.error('Completion request failed', error);
        }

        if (isCurrentRequest(sessionId, responseMessageId)) {
          updateSessionStatus(
            sessionId,
            isAborted || isSuperseded ? 'idle' : 'error',
          );

          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.delete(sessionId);
            return next;
          });

          const timeoutId = timeoutsRef.current.get(sessionId);
          if (timeoutId) {
            clearTimeout(timeoutId);
            timeoutsRef.current.delete(sessionId);
          }

          abortControllersRef.current.delete(sessionId);
          activeRequestIdsRef.current.delete(sessionId);
        }
        if (activeServicesRef.current.get(sessionId) === service) {
          activeServicesRef.current.delete(sessionId);
        }
        terminatedRequestsRef.current.delete(
          getRequestKey(sessionId, responseMessageId),
        );

        throw error;
      }
    },
    [
      getRequestKey,
      getRequestTerminationReason,
      isCurrentRequest,
      updateSessionStatus,
      settingsRef,
      setStreamingMessages,
    ],
  );

  const cancelCompletionRequest = useCallback(
    (sessionId: string, responseMessageId?: string) => {
      const activeResponseMessageId =
        activeRequestIdsRef.current.get(sessionId);
      if (
        responseMessageId !== undefined &&
        activeResponseMessageId !== responseMessageId
      ) {
        logger.info('Ignoring stale completion cancel request', {
          sessionId,
          responseMessageId,
          activeResponseMessageId,
        });
        return;
      }

      logger.info('Manually cancelling completion request', { sessionId });
      if (activeResponseMessageId) {
        terminatedRequestsRef.current.set(
          getRequestKey(sessionId, activeResponseMessageId),
          'aborted',
        );
      }
      activeRequestIdsRef.current.delete(sessionId);
      lastStreamingUpdateRef.current.delete(sessionId);
      setStreamingMessages((prev) => {
        const next = new Map(prev);
        next.delete(sessionId);
        return next;
      });
      const timeoutId = timeoutsRef.current.get(sessionId);
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutsRef.current.delete(sessionId);
      }
      const service = activeServicesRef.current.get(sessionId);
      const abortController = abortControllersRef.current.get(sessionId);
      if (abortController) {
        abortControllersRef.current.delete(sessionId);
        abortController.abort();
      }
      if (service) {
        activeServicesRef.current.delete(sessionId);
      }
    },
    [getRequestKey, setStreamingMessages],
  );

  return { executeCompletionRequest, cancelCompletionRequest };
}
