import { useCallback, useEffect } from 'react';

import {
  AIServiceFactory,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service';
import type { AIServiceConfig } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import type { SessionStatus } from './types';
import { isAbortError, isSupersededRequestError } from './types';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { Settings } from '@/lib/services/settings-service';
import { buildServiceRuntimeConfig } from './service-runtime-config';
import { buildStreamingMessage } from './streaming-message-utils';
import {
  useSessionRequestTracker,
  StreamAccumulator,
  validateAndFinalizeMessage,
} from './execute-completion';

const logger = getLogger('useExecuteCompletion');

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
  const tracker = useSessionRequestTracker();

  // Clean up on unmount
  useEffect(() => {
    const abortControllers = tracker.abortControllersRef;
    const timeouts = tracker.timeoutsRef;
    const activeServices = tracker.activeServicesRef;
    const activeRequestIds = tracker.activeRequestIdsRef;
    const terminatedRequests = tracker.terminatedRequestsRef;

    return () => {
      abortControllers.current?.forEach((controller) => controller.abort());
      abortControllers.current?.clear();

      timeouts.current?.forEach((timeoutId) => window.clearTimeout(timeoutId));
      timeouts.current?.clear();

      activeServices.current?.clear();
      activeRequestIds.current?.clear();
      terminatedRequests.current?.clear();
    };
    // Refs are stable across re-renders; run cleanup only on component unmount
  }, []);

  const executeCompletionRequest = useCallback(
    async (
      sessionId: string,
      responseMessageId: string,
      messages: Message[],
      model: string,
      provider: string,
      apiKey?: string,
      systemPrompt?: string,
      maxTokens?: number,
      availableTools?: MCPTool[],
    ): Promise<Message> => {
      logger.info('🚀 Executing completion request', {
        sessionId,
        messageCount: messages.length,
        provider,
        model,
        maxTokens,
        toolCount: availableTools?.length ?? 0,
        firstMessageId: messages[0]?.id ?? 'none',
        lastMessageId: messages[messages.length - 1]?.id ?? 'none',
      });

      const abortController = tracker.prepareForNewRequest(
        sessionId,
        responseMessageId,
      );

      const resolved = resolveProviderRuntimeConfig(
        provider,
        settingsRef.current,
      );
      const runtimeConfig = buildServiceRuntimeConfig(
        settingsRef.current,
        resolved.serviceConfig,
      );
      const service = AIServiceFactory.getService(
        provider,
        apiKey || resolved.apiKey || '',
        runtimeConfig,
      );
      tracker.activeServicesRef.current.set(sessionId, service);

      const streamingMessage: Partial<Message> = {
        id: responseMessageId,
        sessionId,
        threadId: sessionId,
        role: 'assistant',
        content: [],
        createdAt: new Date(),
      };

      try {
        const config: AIServiceConfig = {
          ...runtimeConfig,
          maxTokens:
            maxTokens ||
            settingsRef.current.advanced?.defaultMaxOutputTokens ||
            8192,
        };

        const safeMessages = MessageNormalizer.sanitizeMessagesForService(
          messages.map(sanitizeMessage),
          service,
        );
        logger.info('✅ Messages sanitized for provider compatibility', {
          sessionId,
          safeCount: safeMessages.length,
        });

        const enrichedMessages = await prepareMessagesForLLM(safeMessages);

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

        const startTime = performance.now();
        const accumulator = new StreamAccumulator(
          sessionId,
          responseMessageId,
          settingsRef,
          startTime,
        );

        const streamGenerator = service.streamChat(enrichedMessages, {
          modelName: model,
          systemPrompt,
          availableTools: availableTools || [],
          config,
          forceToolUse: false,
          signal: abortController.signal,
        });

        for await (const rawChunk of streamGenerator) {
          tracker.ensureRequestStillActive(
            sessionId,
            responseMessageId,
            'stream processing',
            abortController.signal,
          );

          const { hasToolCallUpdate, shouldFlushToolCallImmediately } =
            accumulator.processChunk(rawChunk);

          const nowMs = performance.now();
          const lastUpdateMs =
            tracker.lastStreamingUpdateRef.current.get(sessionId) ?? 0;

          if (
            accumulator.shouldThrottleUpdate(
              lastUpdateMs,
              nowMs,
              hasToolCallUpdate,
              shouldFlushToolCallImmediately,
            )
          ) {
            tracker.lastStreamingUpdateRef.current.set(sessionId, nowMs);
            const streamingToolCalls = accumulator.getStreamingToolCalls();
            setStreamingMessages((prev) => {
              if (!tracker.isCurrentRequest(sessionId, responseMessageId)) {
                return prev;
              }
              const next = new Map(prev);
              next.set(
                sessionId,
                buildStreamingMessage(
                  streamingMessage,
                  accumulator.content,
                  accumulator.thinkingSignature,
                  accumulator.currentThinkingTime,
                  accumulator.finalUsage,
                  {
                    toolCalls: streamingToolCalls,
                    thinkingText: accumulator.currentThinkingText,
                  },
                ),
              );
              return next;
            });
          }
        }

        // Always flush final streaming state after loop ends
        tracker.lastStreamingUpdateRef.current.delete(sessionId);
        const streamingToolCalls = accumulator.getStreamingToolCalls();
        setStreamingMessages((prev) => {
          if (!tracker.isCurrentRequest(sessionId, responseMessageId)) {
            return prev;
          }
          const next = new Map(prev);
          next.set(
            sessionId,
            buildStreamingMessage(
              streamingMessage,
              accumulator.content,
              accumulator.thinkingSignature,
              accumulator.currentThinkingTime,
              accumulator.finalUsage,
              {
                toolCalls: streamingToolCalls,
                thinkingText: accumulator.currentThinkingText,
              },
            ),
          );
          return next;
        });

        tracker.ensureRequestStillActive(
          sessionId,
          responseMessageId,
          'stream finalization',
          abortController.signal,
        );

        const endTime = performance.now();
        const finalMessage = validateAndFinalizeMessage({
          sessionId,
          responseMessageId,
          content: accumulator.content,
          streamingToolCalls,
          currentThinkingText: accumulator.currentThinkingText,
          thinkingSignature: accumulator.thinkingSignature,
          thinkingStartTime: accumulator.thinkingStartTime,
          finalUsage: accumulator.finalUsage,
          startTime,
          endTime,
          firstChunkTime: accumulator.firstChunkTime,
        });

        tracker.ensureRequestStillActive(
          sessionId,
          responseMessageId,
          'final message commit',
          abortController.signal,
        );

        // 1. Update with isStreaming: false IMMEDIATELY
        setStreamingMessages((prev) => {
          if (!tracker.isCurrentRequest(sessionId, responseMessageId)) {
            return prev;
          }
          const next = new Map(prev);
          next.set(sessionId, finalMessage);
          return next;
        });

        // 2. Schedule cleanup with 500ms delay to allow backend event to arrive
        const timeoutId = window.setTimeout(() => {
          setStreamingMessages((prev) => {
            if (!tracker.isCurrentRequest(sessionId, responseMessageId)) {
              return prev;
            }
            const next = new Map(prev);
            const current = next.get(sessionId);
            if (current?.id === finalMessage.id) {
              next.delete(sessionId);
            }
            return next;
          });
          tracker.timeoutsRef.current.delete(sessionId);
        }, 500);
        tracker.timeoutsRef.current.set(sessionId, timeoutId);

        tracker.ensureRequestStillActive(
          sessionId,
          responseMessageId,
          'request completion',
          abortController.signal,
        );

        updateSessionStatus(sessionId, 'idle');
        tracker.cleanupRequestState(
          sessionId,
          responseMessageId,
          abortController,
          service,
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

        if (tracker.isCurrentRequest(sessionId, responseMessageId)) {
          updateSessionStatus(
            sessionId,
            isAborted || isSuperseded ? 'idle' : 'error',
          );

          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.delete(sessionId);
            return next;
          });

          const timeoutId = tracker.timeoutsRef.current.get(sessionId);
          if (timeoutId) {
            clearTimeout(timeoutId);
            tracker.timeoutsRef.current.delete(sessionId);
          }

          tracker.abortControllersRef.current.delete(sessionId);
          tracker.activeRequestIdsRef.current.delete(sessionId);
        }
        if (tracker.activeServicesRef.current.get(sessionId) === service) {
          tracker.activeServicesRef.current.delete(sessionId);
        }
        tracker.terminatedRequestsRef.current.delete(
          tracker.getRequestKey(sessionId, responseMessageId),
        );

        throw error;
      }
    },
    [tracker, settingsRef, updateSessionStatus, setStreamingMessages],
  );

  const cancelCompletionRequest = useCallback(
    (sessionId: string, responseMessageId?: string) => {
      tracker.cancelCompletionRequest(sessionId, responseMessageId, (sid) => {
        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.delete(sid);
          return next;
        });
      });
    },
    [tracker.cancelCompletionRequest, setStreamingMessages],
  );

  return { executeCompletionRequest, cancelCompletionRequest };
}
