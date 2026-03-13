import React, { useCallback, useEffect, useRef, useState } from 'react';

import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import type {
  IAIService,
  AIServiceConfig,
  TokenUsage,
} from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import type { SessionStatus } from './types';
import { isAbortError } from './types';
import type { Message, ToolCall } from '@/models/chat';
import type {
  MCPTool,
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp';
import type { Settings } from '@/lib/services/settings-service';

const logger = getLogger('useLLMExecution');

interface UseLLMExecutionProps {
  settingsRef: React.MutableRefObject<Settings>;
  streamingMessages: Map<string, Partial<Message>>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => void;
}

export function useLLMExecution({
  settingsRef,
  streamingMessages,
  setStreamingMessages,
  updateSessionStatus,
}: UseLLMExecutionProps) {
  // Track active service instances for cleanup
  const activeServicesRef = useRef<Map<string, IAIService>>(new Map());
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  // Track last streaming UI update time per session (throttle to ~20fps)
  const lastStreamingUpdateRef = useRef<Map<string, number>>(new Map());

  // Context window usage per session for gauge display
  const [contextUsageMap, setContextUsageMap] = useState<
    ReadonlyMap<
      string,
      { totalTokens: number; contextWindow: number; modelMaxContext?: number }
    >
  >(new Map());

  // Tracks which sessions have an async compaction in-flight
  const [compactingMap, setCompactingMap] = useState<
    ReadonlyMap<string, boolean>
  >(new Map());

  const setCompacting = useCallback((sessionId: string, value: boolean) => {
    setCompactingMap((prev) => {
      const next = new Map(prev);
      if (value) {
        next.set(sessionId, true);
      } else {
        next.delete(sessionId);
      }
      return next;
    });
  }, []);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      abortControllersRef.current.forEach((controller) => controller.abort());
      abortControllersRef.current.clear();

      timeoutsRef.current.forEach((timeoutId) =>
        window.clearTimeout(timeoutId),
      );
      timeoutsRef.current.clear();

      activeServicesRef.current.forEach((svc) => svc.dispose());
      activeServicesRef.current.clear();
    };
  }, []);

  const executeCompletionRequest = useCallback(
    async (
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
      contextUsage?: {
        totalTokens: number;
        contextWindow: number;
        modelMaxContext?: number;
      },
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

      // Cancel and dispose any previously active request for this session
      const previousController = abortControllersRef.current.get(sessionId);
      if (previousController) {
        previousController.abort();
      }
      const previousService = activeServicesRef.current.get(sessionId);
      if (previousService) {
        previousService.dispose();
      }

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
      activeServicesRef.current.set(sessionId, service);

      // Get existing streaming message (already set by event listener)
      const existingStreamingMessage = streamingMessages.get(sessionId);
      const streamingMessage: Partial<Message> = existingStreamingMessage || {
        id: `msg_${Date.now()}`,
        sessionId,
        threadId: sessionId,
        role: 'assistant',
        content: [],
        createdAt: new Date(),
      };

      try {
        // Build config
        const config: AIServiceConfig = {
          maxTokens:
            maxTokens ||
            settingsRef.current.advanced?.defaultMaxOutputTokens ||
            8192,
          temperature: temperature,
        };

        // Get model metadata
        let modelInfo: ModelInfo | null =
          (await service.listModels()).find((m) => m.name === model) || null;
        if (!modelInfo) {
          modelInfo =
            llmConfigManager.getModel(provider, model) ??
            ({
              contextWindow: 64 * 1024,
              supportReasoning: false,
              supportTools: false,
              cost: { input: 0, output: 0 },
              name: model,
            } as ModelInfo);
        }

        logger.info('Model Info for Token Limit Calculation', {
          sessionId,
          provider,
          model,
          modelInfo,
        });

        // ── Prepare context messages based on selected strategy ─────────────
        let enrichedMessages: Message[];

        // Process the pre-sliced messages provided by the Rust backend.
        const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
          messages.map(sanitizeMessage),
          provider as AIServiceProvider,
        );
        logger.info('✅ Messages sanitized for provider compatibility', {
          sessionId,
          safeCount: safeMessages.length,
        });

        enrichedMessages = await prepareMessagesForLLM(safeMessages);

        // 3. Set telemetries
        if (contextUsage) {
          setContextUsageMap((prev) => {
            const next = new Map(prev);
            next.set(sessionId, {
              totalTokens: contextUsage.totalTokens,
              contextWindow: contextUsage.contextWindow,
              modelMaxContext: contextUsage.modelMaxContext,
            });
            return next;
          });
        }

        // ── Execute Stream ───────────────────────────────────────────────────
        updateSessionStatus(sessionId, 'streaming');

        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, {
            id: `msg_${Date.now()}`,
            role: 'assistant',
            content: [],
            isStreaming: true,
          });
          return next;
        });

        const content: MCPContent[] = [];
        const activeToolCallIndices = new Map<number, number>();

        let thinkingStartTime: number | undefined;
        let currentThinkingTime: number | undefined;
        let finalUsage: TokenUsage | undefined;
        let firstChunkTime: number | undefined;
        let thinkingSignature: string | undefined;

        const startTime = performance.now();

        // Let the provider choose how to deliver sessionContext (stable system
        // prompt concat vs. ephemeral tail message injection for prefix caching).
        const {
          systemPrompt: effectiveSystemPrompt,
          messages: effectiveMessages,
        } = service.prepareContextInjection(
          systemPrompt,
          sessionContext,
          enrichedMessages,
        );

        const streamGenerator = service.streamChat(effectiveMessages, {
          modelName: model,
          systemPrompt: effectiveSystemPrompt,
          availableTools: availableTools || [],
          config,
          forceToolUse: false,
        });

        for await (const rawChunk of streamGenerator) {
          if (firstChunkTime === undefined) {
            firstChunkTime = performance.now();
          }

          if (abortController.signal.aborted) {
            logger.warn('Completion request aborted', { sessionId });
            throw new Error('Request aborted');
          }

          // streamChat yields JSON strings; parse into a typed chunk object.
          let chunk: Record<string, unknown>;
          try {
            chunk = JSON.parse(rawChunk);
          } catch {
            chunk = { content: rawChunk };
          }

          // 1. Accumulate Text
          if (chunk.content && typeof chunk.content === 'string') {
            const lastItem = content[content.length - 1];
            if (lastItem && lastItem.type === 'text') {
              (lastItem as MCPTextContent).text += chunk.content;
            } else {
              content.push({ type: 'text', text: chunk.content });
            }
          }

          // 2. Accumulate Thinking
          if (chunk.thinking && typeof chunk.thinking === 'string') {
            if (thinkingStartTime === undefined) {
              thinkingStartTime = performance.now();
            }
            const lastItem = content[content.length - 1];
            if (lastItem && lastItem.type === 'thinking') {
              (lastItem as MCPThinkingContent).thinking += chunk.thinking;
            } else {
              content.push({ type: 'thinking', thinking: chunk.thinking });
            }
          }

          // 3. Accumulate Thinking Signature
          const chunkMetadata =
            chunk.metadata !== null && typeof chunk.metadata === 'object'
              ? (chunk.metadata as Record<string, unknown>)
              : undefined;
          if (
            chunkMetadata?.thinking_signature &&
            typeof chunkMetadata.thinking_signature === 'string'
          ) {
            thinkingSignature = chunkMetadata.thinking_signature;
          }

          // 4. Accumulate Tool Calls
          if (chunk.tool_calls && Array.isArray(chunk.tool_calls)) {
            (chunk.tool_calls as (ToolCall & { index?: number })[]).forEach(
              (toolCallChunk) => {
                const { index } = toolCallChunk;

                if (index === undefined) {
                  content.push({
                    type: 'tool_call',
                    id: toolCallChunk.id || '',
                    name: toolCallChunk.function?.name || '',
                    arguments: toolCallChunk.function?.arguments || '',
                  });
                  return;
                }

                if (activeToolCallIndices.has(index)) {
                  const contentIndex = activeToolCallIndices.get(index)!;
                  const targetBlock = content[
                    contentIndex
                  ] as MCPToolCallContent;
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
                } else {
                  const newBlock: MCPToolCallContent = {
                    type: 'tool_call',
                    id: toolCallChunk.id || '',
                    name: toolCallChunk.function?.name || '',
                    arguments: toolCallChunk.function?.arguments || '',
                  };
                  content.push(newBlock);
                  activeToolCallIndices.set(index, content.length - 1);
                }
              },
            );
          }

          if (thinkingStartTime !== undefined) {
            currentThinkingTime =
              (performance.now() - thinkingStartTime) / 1000;
          }

          if (chunk.usage) {
            const incomingUsage = chunk.usage as TokenUsage;
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
                  incomingUsage.cachedPromptTokens ||
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
          if (nowMs - lastUpdateMs >= 50) {
            lastStreamingUpdateRef.current.set(sessionId, nowMs);
            setStreamingMessages((prev) => {
              const next = new Map(prev);
              const toolCalls: ToolCall[] = content
                .filter((c) => c.type === 'tool_call')
                .map((c) => {
                  const tc = c as MCPToolCallContent;
                  return {
                    id: tc.id,
                    type: 'function',
                    function: { name: tc.name, arguments: tc.arguments },
                  };
                });
              const thinking = content
                .filter((c) => c.type === 'thinking')
                .map((c) => (c as MCPThinkingContent).thinking)
                .join('\n');
              next.set(sessionId, {
                ...streamingMessage,
                content,
                tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
                thinking: thinking || undefined,
                thinkingSignature,
                thinkingTime: currentThinkingTime,
                usage: finalUsage,
                isStreaming: true,
              });
              return next;
            });
          }
        }

        // Always flush final streaming state after loop ends
        lastStreamingUpdateRef.current.delete(sessionId);
        setStreamingMessages((prev) => {
          const next = new Map(prev);
          const toolCalls: ToolCall[] = content
            .filter((c) => c.type === 'tool_call')
            .map((c) => {
              const tc = c as MCPToolCallContent;
              return {
                id: tc.id,
                type: 'function',
                function: { name: tc.name, arguments: tc.arguments },
              };
            });
          const thinking = content
            .filter((c) => c.type === 'thinking')
            .map((c) => (c as MCPThinkingContent).thinking)
            .join('\n');
          next.set(sessionId, {
            ...streamingMessage,
            content,
            tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
            thinking: thinking || undefined,
            thinkingSignature,
            thinkingTime: currentThinkingTime,
            usage: finalUsage,
            isStreaming: true,
          });
          return next;
        });

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

        const finalToolCalls: ToolCall[] = content
          .filter((c) => c.type === 'tool_call')
          .map((c) => {
            const tc = c as MCPToolCallContent;
            return {
              id: tc.id,
              type: 'function',
              function: { name: tc.name, arguments: tc.arguments },
            };
          });

        const finalThinking = content
          .filter((c) => c.type === 'thinking')
          .map((c) => (c as MCPThinkingContent).thinking)
          .join('\n');

        const finalMessage: Message = {
          id: streamingMessage.id ?? `msg_${Date.now()}`,
          sessionId,
          threadId: sessionId,
          role: 'assistant',
          content,
          createdAt: new Date(),
          tool_calls: finalToolCalls.length > 0 ? finalToolCalls : undefined,
          thinking: finalThinking || undefined,
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
        });

        const hasContent =
          (finalMessage.content &&
            finalMessage.content.some((c) =>
              c.type === 'text' ? !!(c as MCPTextContent).text?.trim() : true,
            )) ||
          (finalMessage.tool_calls && finalMessage.tool_calls.length > 0) ||
          !!finalMessage.thinking;

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
          throw new Error('Received empty response from LLM provider');
        } else if (!hasContent && hasUsage) {
          logger.warn(
            '⚠️ Response has usage but no content - allowing to proceed',
            {
              sessionId,
              usage: finalMessage.usage,
            },
          );
        }

        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, finalMessage);
          return next;
        });

        const timeoutId = window.setTimeout(() => {
          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.delete(sessionId);
            return next;
          });
          timeoutsRef.current.delete(sessionId);
        }, 100);
        timeoutsRef.current.set(sessionId, timeoutId);

        updateSessionStatus(sessionId, 'idle');

        if (abortControllersRef.current.get(sessionId) === abortController) {
          abortControllersRef.current.delete(sessionId);
        }
        if (activeServicesRef.current.get(sessionId) === service) {
          activeServicesRef.current.delete(sessionId);
        }

        return finalMessage;
      } catch (error) {
        const isAborted = isAbortError(error);

        if (isAborted) {
          logger.info('Completion request was aborted by cancellation', {
            sessionId,
          });
        } else {
          logger.error('Completion request failed', error);
        }

        if (abortControllersRef.current.get(sessionId) === abortController) {
          updateSessionStatus(sessionId, isAborted ? 'idle' : 'error');

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
        }
        if (activeServicesRef.current.get(sessionId) === service) {
          activeServicesRef.current.delete(sessionId);
        }

        throw error;
      }
    },
    [updateSessionStatus, settingsRef, streamingMessages, setStreamingMessages],
  );

  const cancelCompletionRequest = useCallback((sessionId: string) => {
    logger.info('Manually cancelling completion request', { sessionId });
    const abortController = abortControllersRef.current.get(sessionId);
    if (abortController) {
      abortController.abort();
    }
  }, []);

  const clearSessionState = useCallback((sessionId: string) => {
    setContextUsageMap((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  }, []);

  /**
   * Clears in-memory context usage state for ALL sessions.
   * Called when the global context strategy changes.
   */
  const clearAllCompactState = useCallback(() => {
    setContextUsageMap(new Map());
  }, []);

  /**
   * Resets the token count for a session to 0 after compaction completes.
   * Keeps contextWindow/modelMaxContext so the gauge still renders correctly
   * (showing 0%) until the next LLM call provides the real post-compact estimate.
   */
  const resetContextUsageForSession = useCallback((sessionId: string) => {
    setContextUsageMap((prev) => {
      const existing = prev.get(sessionId);
      if (!existing) return prev;
      const next = new Map(prev);
      next.set(sessionId, { ...existing, totalTokens: 0 });
      return next;
    });
  }, []);

  return {
    executeCompletionRequest,
    cancelCompletionRequest,
    clearSessionState,
    clearAllCompactState,
    resetContextUsageForSession,
    setCompacting,
    isCompacting: (sessionId: string) => compactingMap.get(sessionId) === true,
    isAwaitingCompact: () => false,
    getContextUsage: (
      sessionId: string,
    ):
      | { totalTokens: number; contextWindow: number; modelMaxContext?: number }
      | undefined => contextUsageMap.get(sessionId),
    getCompactedRange: (): { fromId: string; toId: string } | undefined =>
      undefined,
  };
}
