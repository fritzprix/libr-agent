import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { Settings } from '@/context/SettingsContext';
import { AIServiceProvider } from '@/lib/ai-service/types';
import { normalizeRustMessage } from '@/lib/ai-service/utils';
import { getLogger } from '@/lib/logger';
import { sleep } from '@/lib/retry-utils';
import type { CompletionRequest } from './types';
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
    temperature?: number,
    maxTokens?: number,
    availableTools?: MCPTool[],
  ) => Promise<Message>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
}

export function useLLMListener({
  settingsRef,
  executeCompletionRequest,
  setStreamingMessages,
}: UseLLMListenerProps) {
  // Track listener setup to prevent duplicate registration in React Strict Mode
  const listenerSetupRef = useRef(false);

  useEffect(() => {
    // Prevent duplicate listener registration in React Strict Mode
    if (listenerSetupRef.current) {
      logger.info(
        '⚠️ LLM listener already set up, skipping duplicate registration',
      );
      return;
    }

    listenerSetupRef.current = true;
    logger.info('🎧 Initializing LLM completion request listener');

    let isMounted = true;
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      logger.info('Setting up LLM completion request listener');

      const unlistenFn = await listen<CompletionRequest>(
        'llm:completion-request',
        async (event) => {
          const {
            sessionId,
            messages: rawMessages,
            model,
            provider,
            systemPrompt,
            temperature,
            maxTokens,
            availableTools,
          } = event.payload;

          // Normalize messages from Rust (camelCase -> snake_case)
          const messages = rawMessages.map(normalizeRustMessage);

          // Always get API key from Settings, ignore any apiKey from Rust backend
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
            messageRoles: messages.map((m) => m.role).join(','),
          });

          logger.debug('📋 Full message list received from Rust', {
            sessionId,
            messages: messages.map((m, idx) => ({
              index: idx,
              id: m.id,
              role: m.role,
              hasContent: !!m.content && m.content.length > 0,
              hasToolCalls: !!m.tool_calls,
              toolCallId: m.tool_call_id,
            })),
          });

          // ✅ Set streaming message IMMEDIATELY when request is received
          // This provides instant visual feedback (~50-200ms earlier than setting it inside executeCompletionRequest)
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
            // SP4: Execute with retry (exponential backoff + jitter) and optional fallback model.
            // Malformed/empty responses come as errors from executeCompletionRequest,
            // despite the LLM API returning HTTP 200 — this recovers from those silently.
            const SP4_MAX_RETRIES = 3;
            const SP4_BASE_DELAY_MS = 500;

            const attemptCompletion = async (
              targetModel: string,
              targetProvider: string,
              targetApiKey: string,
            ): Promise<Message> => {
              for (let attempt = 0; attempt <= SP4_MAX_RETRIES; attempt++) {
                if (attempt > 0) {
                  // Exponential backoff with ±50% jitter to spread retries
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

                  // Reset streaming indicator so UI shows a fresh spinner on retry
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
                  return await executeCompletionRequest(
                    sessionId,
                    messages,
                    targetModel,
                    targetProvider,
                    targetApiKey,
                    systemPrompt,
                    temperature,
                    maxTokens,
                    availableTools,
                  );
                } catch (attemptError) {
                  // Abort errors must never be retried — propagate immediately
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
              // Unreachable, but satisfies TS
              throw new Error('SP4: retry loop exhausted');
            };

            // First try primary model with retries
            let result: Message;
            try {
              result = await attemptCompletion(model, provider, finalApiKey);
            } catch (primaryError) {
              if (isAbortError(primaryError)) {
                throw primaryError; // Let the outer catch handle abort
              }

              // Primary model exhausted — try configured fallback model
              const fallbackModel = settingsRef.current.fallbackModel;
              if (fallbackModel) {
                const fallbackApiKey =
                  settingsRef.current.serviceConfigs?.[
                    fallbackModel.provider as AIServiceProvider
                  ]?.apiKey ?? '';

                logger.warn(
                  `SP4: Primary model failed all retries, switching to fallback ${fallbackModel.provider}/${fallbackModel.model}`,
                  { sessionId },
                );

                // Reset streaming indicator for fallback attempt
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

                // One shot with fallback — no further retries
                result = await executeCompletionRequest(
                  sessionId,
                  messages,
                  fallbackModel.model,
                  fallbackModel.provider,
                  fallbackApiKey,
                  systemPrompt,
                  temperature,
                  maxTokens,
                  availableTools,
                );
              } else {
                throw primaryError; // No fallback, propagate to outer catch
              }
            }

            // Send result back to Rust
            logger.info('Sending LLM response to Rust', {
              sessionId,
              hasToolCalls: !!result.tool_calls,
              toolCallCount: result.tool_calls?.length ?? 0,
              toolCalls: result.tool_calls,
            });

            // Convert to Rust Message format with explicit field mapping
            // Fixes deserialization issue: threadId doesn't exist in Rust schema
            // Convert timestamps and map camelCase to snake_case
            const now = Date.now();
            const messageForRust = {
              id: result.id,
              sessionId: result.sessionId,
              // threadId removed - not in Rust Message schema
              role: result.role,
              content: result.content || [],
              // Ensure all tool calls have the required 'type' field
              toolCalls: result.tool_calls
                ? result.tool_calls.map((tc) => ({
                    id: tc.id,
                    type: tc.type || 'function',
                    function: tc.function,
                  }))
                : undefined,
              toolCallId: result.tool_call_id || undefined,
              isStreaming: result.isStreaming || undefined,
              thinking: result.thinking || undefined,
              thinkingSignature: result.thinkingSignature || undefined,
              assistantId: result.assistantId || undefined,
              attachments: result.attachments || undefined,
              toolUse: result.tool_use || undefined,
              createdAt:
                result.createdAt instanceof Date
                  ? result.createdAt.getTime()
                  : result.createdAt || now,
              updatedAt:
                result.updatedAt instanceof Date
                  ? result.updatedAt.getTime()
                  : result.updatedAt ||
                    (result.createdAt instanceof Date
                      ? result.createdAt.getTime()
                      : result.createdAt) ||
                    now,
              source: result.source || undefined,
              error: result.error || undefined,
            };

            logger.info('Message prepared for Rust', {
              sessionId,
              hasToolCalls: !!messageForRust.toolCalls,
              toolCallCount: messageForRust.toolCalls?.length ?? 0,
              createdAtType: typeof messageForRust.createdAt,
              fullMessage: messageForRust,
            });

            await invoke('agent_handle_llm_response', {
              sessionId,
              assistantMessage: messageForRust,
            });

            logger.info('LLM response sent back to Rust', { sessionId });
          } catch (error) {
            // If the request was intentionally aborted (user cancelled), do NOT report
            // this as an error to Rust - the cancel_workflow command already handles
            // state transition to Idle. Reporting it would cause a race where Rust
            // transitions: Idle (cancel) → Error (stale abort) in wrong order.
            const isAborted = isAbortError(error);

            if (isAborted) {
              logger.info(
                'LLM request aborted due to cancellation, skipping error report to Rust',
                { sessionId },
              );
              return;
            }

            logger.error('Failed to execute LLM completion', error);

            // Report error to Rust
            await invoke('agent_handle_llm_error', {
              sessionId,
              error: error instanceof Error ? error.message : String(error),
            });
          }
        },
      );

      if (!isMounted) {
        logger.info(
          'LLM listener setup completed after unmount, cleaning up immediately',
        );
        unlistenFn();
      } else {
        unlisten = unlistenFn;
        logger.info('LLM completion request listener registered');
      }
    };

    setupListener();

    return () => {
      isMounted = false;
      if (unlisten) {
        unlisten();
        logger.info('LLM completion request listener cleaned up');
      }
      // Reset listener setup ref on unmount
      listenerSetupRef.current = false;
    };
  }, []); // ⚠️ CRITICAL: Empty dependency array to prevent re-registering listener
}
