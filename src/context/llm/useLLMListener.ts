import {
  handleCompactionError,
  handleCompactionResponse,
  handleLLMError,
  handleLLMResponse,
} from '@/lib/backend/agent-commands';
import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';

import { AIServiceFactory } from '@/lib/ai-service';
import { messageToRustMessage, type Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { Settings } from '@/context/SettingsContext';
import { AIServiceProvider } from '@/lib/ai-service/types';
import { normalizeRustMessage } from '@/lib/ai-service/utils';
import { getLogger } from '@/lib/logger';
import { sleep } from '@/lib/retry-utils';
import type {
  CompactionRequest,
  CompactionStateEvent,
  CompletionRequest,
} from './types';
import { isAbortError } from './types';

const logger = getLogger('useLLMListener');

interface UseLLMListenerProps {
  settingsRef: React.MutableRefObject<Settings>;
  executeCompletionRequest: (
    sessionId: string,
    messages: Message[],
    model: string,
    provider: string,
    apiKey?: string,
    systemPrompt?: string,
    sessionContext?: string,
    temperature?: number,
    maxTokens?: number,
    availableTools?: MCPTool[],
    backendOwnedCompaction?: boolean,
  ) => Promise<Message>;
  applyCompactionState: (event: CompactionStateEvent) => void;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
}

export function useLLMListener({
  settingsRef,
  executeCompletionRequest,
  applyCompactionState,
  setStreamingMessages,
}: UseLLMListenerProps) {
  const executeCompletionRequestRef = useRef(executeCompletionRequest);
  const applyCompactionStateRef = useRef(applyCompactionState);

  useEffect(() => {
    executeCompletionRequestRef.current = executeCompletionRequest;
  }, [executeCompletionRequest]);

  useEffect(() => {
    applyCompactionStateRef.current = applyCompactionState;
  }, [applyCompactionState]);

  useEffect(() => {
    logger.info('🎧 Initializing LLM bridge listeners');

    let isMounted = true;
    const unlisteners: Array<() => void> = [];

    const setupListeners = async () => {
      const unlistenCompletion = await listen<CompletionRequest>(
        'llm:completion-request',
        async (event) => {
          const {
            sessionId,
            messages: rawMessages,
            model,
            provider,
            systemPrompt,
            sessionContext,
            temperature,
            maxTokens,
            availableTools,
            backendOwnedCompaction,
          } = event.payload;

          const messages = rawMessages.map(normalizeRustMessage);
          const finalApiKey =
            settingsRef.current.serviceConfigs?.[provider as AIServiceProvider]
              ?.apiKey || '';

          logger.info('📥 Received LLM completion request from Rust', {
            sessionId,
            messageCount: messages.length,
            toolCount: availableTools?.length ?? 0,
            provider,
            hasApiKey: !!finalApiKey,
            eventId: event.id,
            firstMessageId: messages[0]?.id ?? 'none',
            lastMessageId: messages[messages.length - 1]?.id ?? 'none',
          });

          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.set(sessionId, {
              id: `msg_${Date.now()}`,
              sessionId,
              threadId: sessionId,
              role: 'assistant',
              content: [],
              isStreaming: true,
              createdAt: new Date(),
            });
            return next;
          });

          try {
            const SP4_MAX_RETRIES = 3;
            const SP4_BASE_DELAY_MS = 500;

            const attemptCompletion = async (
              targetModel: string,
              targetProvider: string,
              targetApiKey: string,
            ): Promise<Message> => {
              for (let attempt = 0; attempt <= SP4_MAX_RETRIES; attempt++) {
                if (attempt > 0) {
                  const rawDelay = Math.min(
                    SP4_BASE_DELAY_MS * Math.pow(2, attempt - 1),
                    30000,
                  );
                  const jitteredDelay = rawDelay * (0.5 + Math.random());
                  logger.warn(
                    `SP4: Retry ${attempt}/${SP4_MAX_RETRIES} after ${Math.round(jitteredDelay)}ms`,
                    { sessionId, model: targetModel, provider: targetProvider },
                  );
                  await sleep(jitteredDelay);

                  setStreamingMessages((prev) => {
                    const next = new Map(prev);
                    next.set(sessionId, {
                      id: `msg_${Date.now()}`,
                      sessionId,
                      threadId: sessionId,
                      role: 'assistant',
                      content: [],
                      isStreaming: true,
                      createdAt: new Date(),
                    });
                    return next;
                  });
                }

                try {
                  return await executeCompletionRequestRef.current(
                    sessionId,
                    messages,
                    targetModel,
                    targetProvider,
                    targetApiKey,
                    systemPrompt,
                    sessionContext,
                    temperature,
                    maxTokens,
                    availableTools,
                    backendOwnedCompaction,
                  );
                } catch (attemptError) {
                  if (isAbortError(attemptError)) {
                    throw attemptError;
                  }
                  if (attempt === SP4_MAX_RETRIES) {
                    throw attemptError;
                  }
                  logger.warn(
                    `SP4: Attempt ${attempt + 1} failed, will retry`,
                    { sessionId, error: attemptError },
                  );
                }
              }

              throw new Error('SP4: retry loop exhausted');
            };

            let result: Message;
            try {
              result = await attemptCompletion(model, provider, finalApiKey);
            } catch (primaryError) {
              if (isAbortError(primaryError)) {
                throw primaryError;
              }

              const fallbackModel = settingsRef.current.fallbackModel;
              if (!fallbackModel) {
                throw primaryError;
              }

              const fallbackApiKey =
                settingsRef.current.serviceConfigs?.[
                  fallbackModel.provider as AIServiceProvider
                ]?.apiKey ?? '';

              logger.warn(
                `SP4: Primary model failed all retries, switching to fallback ${fallbackModel.provider}/${fallbackModel.model}`,
                { sessionId },
              );

              setStreamingMessages((prev) => {
                const next = new Map(prev);
                next.set(sessionId, {
                  id: `msg_${Date.now()}`,
                  sessionId,
                  threadId: sessionId,
                  role: 'assistant',
                  content: [],
                  isStreaming: true,
                  createdAt: new Date(),
                });
                return next;
              });

              result = await executeCompletionRequestRef.current(
                sessionId,
                messages,
                fallbackModel.model,
                fallbackModel.provider,
                fallbackApiKey,
                systemPrompt,
                sessionContext,
                temperature,
                maxTokens,
                availableTools,
                backendOwnedCompaction,
              );
            }

            const messageForRust = messageToRustMessage(result);
            await handleLLMResponse(sessionId, messageForRust);
          } catch (error) {
            if (isAbortError(error)) {
              logger.info(
                'LLM request aborted due to cancellation, skipping error report to Rust',
                { sessionId },
              );
              return;
            }

            logger.error('Failed to execute LLM completion', error);
            await handleLLMError(
              sessionId,
              error instanceof Error ? error.message : String(error),
            );
          }
        },
      );

      const unlistenCompaction = await listen<CompactionRequest>(
        'llm:compaction-request',
        async (event) => {
          const {
            requestId,
            sessionId,
            messages: rawMessages,
            model,
            provider,
          } = event.payload;
          const messages = rawMessages.map(normalizeRustMessage);
          const apiKey =
            settingsRef.current.serviceConfigs?.[provider as AIServiceProvider]
              ?.apiKey || '';

          const service = AIServiceFactory.getService(
            provider as AIServiceProvider,
            apiKey,
            settingsRef.current.serviceConfigs?.[
              provider as AIServiceProvider
            ] || {},
          );

          try {
            const summary = await service.compact(messages, {
              modelName: model,
            });
            try {
              await handleCompactionResponse(sessionId, requestId, summary);
            } catch (backendError) {
              // Backend rejected the response (e.g. no pending compaction after cancel,
              // or request-id mismatch). Treat as a no-op — the workflow has already
              // moved on and there is nothing to complete.
              logger.info('handleCompactionResponse rejected by backend (stale/cancelled); ignoring', {
                sessionId,
                requestId,
                error: backendError instanceof Error ? backendError.message : String(backendError),
              });
            }
          } catch (error) {
            if (isAbortError(error)) {
              logger.info('Compaction request aborted by cancellation', {
                sessionId,
                requestId,
              });
              return;
            }

            logger.error('Failed to execute compaction request', error);
            try {
              await handleCompactionError(
                sessionId,
                requestId,
                error instanceof Error ? error.message : String(error),
              );
            } catch (backendError) {
              // Backend rejected the error report (stale/cancelled). No-op.
              logger.info('handleCompactionError rejected by backend (stale/cancelled); ignoring', {
                sessionId,
                requestId,
                error: backendError instanceof Error ? backendError.message : String(backendError),
              });
            }
          } finally {
            service.dispose();
          }
        },
      );

      const unlistenCompactionState = await listen<CompactionStateEvent>(
        'llm:compaction-state',
        (event) => {
          applyCompactionStateRef.current(event.payload);
        },
      );

      if (!isMounted) {
        unlistenCompletion();
        unlistenCompaction();
        unlistenCompactionState();
      } else {
        unlisteners.push(
          unlistenCompletion,
          unlistenCompaction,
          unlistenCompactionState,
        );
        logger.info('LLM bridge listeners registered');
      }
    };

    setupListeners();

    return () => {
      isMounted = false;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [settingsRef, setStreamingMessages]);
}
