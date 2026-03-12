import React, { useCallback, useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import type {
  IAIService,
  AIServiceConfig,
  TokenUsage,
} from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import {
  calculateGroundedTotalTokens,
  estimateTextTokens,
  estimateTokensBPE,
  selectMessagesWithinContext,
} from '@/lib/token-utils';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import type { CompactionStateEvent, SessionStatus } from './types';
import { isAbortError } from './types';
import { compactContextService } from '@/lib/compact-context-service';
import {
  calculateCompactThreshold,
  calculateEffectiveContextLimit,
  findCompactionSplitIndex,
  stripCompactSummaryPrefix,
} from '@/lib/compact-utils';
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
  const streamingMessagesRef = useRef(streamingMessages);
  // Track active service instances for cleanup
  const activeServicesRef = useRef<Map<string, IAIService>>(new Map());
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  // Track last streaming UI update time per session (throttle to ~20fps)
  const lastStreamingUpdateRef = useRef<Map<string, number>>(new Map());

  // SP17: Compact strategy state
  const compactCacheRef = useRef<
    Map<string, { fromId: string; toId: string; summary: string }>
  >(new Map());
  // Resolvers receive `true` on successful compaction, `false` on failure.
  // Waiters use this to decide whether to rebuild the candidate stack.
  const compactResolversRef = useRef<
    Map<string, ((success: boolean) => void)[]>
  >(new Map());
  // Guard flag: set to true on unmount so background compaction IIFEs avoid
  // calling state setters on the unmounted component.
  const unmountedRef = useRef(false);

  const [compactingSet, setCompactingSet] = useState<Set<string>>(new Set());
  const [awaitingSet, setAwaitingSet] = useState<Set<string>>(new Set());
  const [compactedRangeMap, setCompactedRangeMap] = useState<
    ReadonlyMap<string, { fromId: string; toId: string }>
  >(new Map());
  // Context window usage per session for gauge display
  const [contextUsageMap, setContextUsageMap] = useState<
    ReadonlyMap<
      string,
      { totalTokens: number; contextWindow: number; modelMaxContext?: number }
    >
  >(new Map());

  // Clean up on unmount
  useEffect(() => {
    streamingMessagesRef.current = streamingMessages;
  }, [streamingMessages]);

  useEffect(() => {
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;

      abortControllersRef.current.forEach((controller) => controller.abort());
      abortControllersRef.current.clear();

      timeoutsRef.current.forEach((timeoutId) =>
        window.clearTimeout(timeoutId),
      );
      timeoutsRef.current.clear();

      activeServicesRef.current.forEach((svc) => svc.dispose());
      activeServicesRef.current.clear();

      // Resolve all pending compaction waiters with `false` so they don't
      // block indefinitely after unmount, then clear both refs.
      compactResolversRef.current.forEach((resolvers) =>
        resolvers.forEach((r) => r(false)),
      );
      compactResolversRef.current.clear();
      compactCacheRef.current.clear();
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
      backendOwnedCompaction?: boolean,
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
      const existingStreamingMessage =
        streamingMessagesRef.current.get(sessionId);
      const streamingMessage: Partial<Message> =
        existingStreamingMessage?.isStreaming === true
          ? existingStreamingMessage
          : {
              id: `msg_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
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

        const { effectiveLimit: safeInputTokenLimit, modelMaxLimit } =
          calculateEffectiveContextLimit(
            modelInfo,
            settingsRef.current.advanced?.defaultMaxOutputTokens || 8192,
            settingsRef.current.maxInputContext,
          );

        const { windowSize, contextStrategy, toolCallGroupVisibleCount } =
          settingsRef.current;
        const finalSystemPrompt = systemPrompt;
        // Tokens consumed by volatile session context (sections 4-5 from Rust).
        // Counted separately from finalSystemPrompt so budget calculations are
        // accurate regardless of which injection channel the provider uses.
        const sessionContextTokens = sessionContext
          ? estimateTextTokens(sessionContext)
          : 0;

        logger.info('🎯 Applying context management strategy', {
          sessionId,
          inputMessageCount: messages.length,
          contextStrategy: contextStrategy ?? 'window',
          windowSize,
          provider,
          model,
          safeInputTokenLimit,
          modelMaxLimit,
        });

        // ── Prepare context messages based on selected strategy ─────────────
        let enrichedMessages: Message[];

        if (
          (contextStrategy ?? 'window') === 'compact' &&
          !backendOwnedCompaction
        ) {
          // ── Compact strategy (SP17) ─────────────────────────────────────────

          // 1. Initial Load from DB if not in cache (Session Resume)
          if (!compactCacheRef.current.has(sessionId)) {
            const persisted =
              await compactContextService.getCompactContext(sessionId);
            if (persisted) {
              compactCacheRef.current.set(sessionId, {
                fromId: persisted.fromId,
                toId: persisted.toId,
                summary: persisted.summary,
              });
              setCompactedRangeMap((prev) => {
                if (prev.has(sessionId)) return prev;
                const next = new Map(prev);
                next.set(sessionId, {
                  fromId: persisted.fromId,
                  toId: persisted.toId,
                });
                return next;
              });
            }
          }

          // Helper to build candidate stack from current cache
          const buildCandidateStack = (
            msgs: Message[],
          ): { messages: Message[]; summary?: string } => {
            const cached = compactCacheRef.current.get(sessionId);
            if (!cached) return { messages: msgs };

            const toIdIndex = msgs.findIndex((m) => m.id === cached.toId);
            if (toIdIndex >= 0) {
              const remainingMessages = msgs.slice(toIdIndex + 1);
              return {
                messages: remainingMessages,
                summary: `### Previous Conversation Summary\n${cached.summary}`,
              };
            } else {
              logger.warn(
                'Stale compact cache: toId not found. Invalidating.',
                {
                  sessionId,
                  toId: cached.toId,
                },
              );
              compactCacheRef.current.delete(sessionId);
              return { messages: msgs };
            }
          };

          let { messages: candidateMessages, summary: conversationSummary } =
            buildCandidateStack(messages);

          // 2. Token threshold check
          const baseSystemPrompt = systemPrompt || '';
          let finalSystemPrompt = conversationSummary
            ? `${baseSystemPrompt}\n\n${conversationSummary}`.trim()
            : baseSystemPrompt;

          const systemPromptTokens = finalSystemPrompt
            ? estimateTextTokens(finalSystemPrompt)
            : 0;
          const toolsJson = availableTools?.length
            ? JSON.stringify(availableTools)
            : undefined;
          const toolsTokens = toolsJson ? estimateTextTokens(toolsJson) : 0;

          // Guard: if reserved tokens alone already exceed the limit there is
          // nothing compaction can do — the system prompt / tool set is simply
          // too large for this model. Log a clear warning so the user / dev can
          // diagnose it and proceed (selectMessagesWithinContext will keep at
          // least the most-recent turn via its own safety floor).
          const reservedTokens =
            systemPromptTokens + toolsTokens + sessionContextTokens;
          if (reservedTokens >= safeInputTokenLimit) {
            logger.warn(
              '⚠️ Reserved tokens (system prompt + tools) already exceed the context limit. ' +
                'No messages can be safely included. Consider reducing the system prompt, ' +
                'using fewer tools, or switching to a model with a larger context window.',
              {
                sessionId,
                reservedTokens,
                safeInputTokenLimit,
                systemPromptTokens,
                toolsTokens,
              },
            );
            // Notify the user — use a stable ID so repeated sends update the
            // same toast rather than stacking new ones.
            toast.warning('Context window too small', {
              id: `ctx-overflow-${sessionId}`,
              description:
                `System prompt + tools use ~${Math.round(reservedTokens / 1000)}k tokens, ` +
                `which exceeds this model's limit (~${Math.round(safeInputTokenLimit / 1000)}k). ` +
                'Try reducing active tools or switching to a model with a larger context window.',
              duration: 8000,
            });
          }

          let totalTokens = calculateGroundedTotalTokens(
            candidateMessages,
            systemPromptTokens + sessionContextTokens,
            toolsTokens,
          );
          const threshold = calculateCompactThreshold(safeInputTokenLimit);
          let overflow = totalTokens >= safeInputTokenLimit;

          // 3. Wait for pending compaction if overflow (100%+)
          if (overflow && compactResolversRef.current.has(sessionId)) {
            logger.info(
              '⏳ Context overflow: waiting for pending compaction...',
              {
                sessionId,
              },
            );
            setAwaitingSet((prev) => new Set([...prev, sessionId]));
            try {
              const compactionSucceeded = await new Promise<boolean>(
                (resolve) => {
                  const list = compactResolversRef.current.get(sessionId) ?? [];
                  list.push(resolve);
                  compactResolversRef.current.set(sessionId, list);
                },
              );
              if (compactionSucceeded) {
                // Compaction wrote a new summary — rebuild the stack from fresh cache.
                const { messages: newMessages, summary: newSummary } =
                  buildCandidateStack(messages);
                candidateMessages = newMessages;

                const currentBaseSystemPrompt = systemPrompt || '';
                const updatedSystemPrompt = newSummary
                  ? `${currentBaseSystemPrompt}\n\n${newSummary}`.trim()
                  : currentBaseSystemPrompt;

                // Update finalSystemPrompt so selectMessagesWithinContext
                // uses the correct token budget after compaction.
                finalSystemPrompt = updatedSystemPrompt;

                const updatedSystemPromptTokens = updatedSystemPrompt
                  ? estimateTextTokens(updatedSystemPrompt)
                  : 0;

                totalTokens = calculateGroundedTotalTokens(
                  candidateMessages,
                  updatedSystemPromptTokens + sessionContextTokens,
                  toolsTokens,
                );
                overflow = totalTokens >= safeInputTokenLimit;
                logger.info('⏳ Resuming after successful compaction', {
                  sessionId,
                  newTotalTokens: totalTokens,
                });
              } else {
                // Compaction failed — cache is unchanged. Proceed with the current
                // (oversized) stack; selectMessagesWithinContext will hard-trim it.
                logger.warn(
                  '⏳ Resuming after failed compaction — context still large, will trim',
                  { sessionId, totalTokens },
                );
              }
            } finally {
              setAwaitingSet((prev) => {
                const next = new Set(prev);
                next.delete(sessionId);
                return next;
              });
            }
          }

          // Update gauge with the latest (possibly post-wait) status
          setContextUsageMap((prev) => {
            const next = new Map(prev);
            next.set(sessionId, {
              totalTokens,
              contextWindow: safeInputTokenLimit,
              modelMaxContext: modelMaxLimit,
            });
            return next;
          });

          // 4. Trigger async compaction if threshold (90%+) exceeded
          if (
            totalTokens >= threshold &&
            !compactResolversRef.current.has(sessionId)
          ) {
            const splitIdx = findCompactionSplitIndex(
              candidateMessages,
              estimateTokensBPE,
              threshold,
              systemPromptTokens,
              toolsTokens,
            );
            const oldMessages = candidateMessages.slice(0, splitIdx);

            if (
              oldMessages.length >= 5 ||
              (compactCacheRef.current.has(sessionId) && oldMessages.length > 1)
            ) {
              compactResolversRef.current.set(sessionId, []);
              setCompactingSet((prev) => new Set([...prev, sessionId]));

              // Create a dedicated service instance for compaction so it is NOT
              // tracked in activeServicesRef and cannot be disposed by a subsequent
              // request for the same session while compaction is still in-flight.
              const compactionService = AIServiceFactory.getService(
                provider as AIServiceProvider,
                apiKey ?? '',
                settingsRef.current.serviceConfigs?.[
                  provider as AIServiceProvider
                ] || {},
              );

              (async () => {
                let compactionSucceeded = false;
                try {
                  logger.info('🚀 Triggering async compaction', {
                    sessionId,
                    oldCount: oldMessages.length,
                  });
                  const summary = await compactionService.compact(oldMessages, {
                    modelName: model,
                  });

                  // Abort post-compaction state updates if the component unmounted.
                  if (unmountedRef.current) return;

                  // Use stable IDs; strip compact-summary- prefix to avoid nesting.
                  const firstMsg = oldMessages[0];
                  const fromId = stripCompactSummaryPrefix(firstMsg.id);
                  const toId = oldMessages[oldMessages.length - 1].id;

                  compactCacheRef.current.set(sessionId, {
                    fromId,
                    toId,
                    summary,
                  });
                  await compactContextService.saveCompactContext(sessionId, {
                    id: `cc_${Date.now()}`,
                    sessionId,
                    fromId,
                    toId,
                    summary,
                    createdAt: Date.now(),
                  });

                  if (unmountedRef.current) return;

                  setCompactedRangeMap((prev) => {
                    const next = new Map(prev);
                    next.set(sessionId, { fromId, toId });
                    return next;
                  });
                  compactionSucceeded = true;
                  logger.info('✅ Async compaction completed', {
                    sessionId,
                    fromId,
                    toId,
                  });
                } catch (err) {
                  logger.error('❌ Async compaction failed', {
                    sessionId,
                    error: err,
                  });
                } finally {
                  compactionService.dispose();
                  if (!unmountedRef.current) {
                    const resolvers =
                      compactResolversRef.current.get(sessionId) ?? [];
                    resolvers.forEach((r) => r(compactionSucceeded));
                    compactResolversRef.current.delete(sessionId);
                    setCompactingSet((prev) => {
                      const next = new Set(prev);
                      next.delete(sessionId);
                      return next;
                    });
                  }
                }
              })();
            }
          }

          // 5. Final Selection
          const contextMessages = selectMessagesWithinContext(
            candidateMessages,
            provider,
            model,
            safeInputTokenLimit,
            {
              systemPrompt: finalSystemPrompt,
              toolsJson,
              maxToolCallsPerMessage:
                provider === AIServiceProvider.Gemini
                  ? 100
                  : toolCallGroupVisibleCount || 4,
            },
          );

          const safeCompactMessages =
            MessageNormalizer.sanitizeMessagesForProvider(
              contextMessages.map(sanitizeMessage),
              provider as AIServiceProvider,
            );
          enrichedMessages = await prepareMessagesForLLM(safeCompactMessages);
        } else {
          // ── Window strategy (default) ──────────────────────────────────────
          const toolsJson = availableTools?.length
            ? JSON.stringify(availableTools)
            : undefined;

          const contextMessages = selectMessagesWithinContext(
            messages,
            provider,
            model,
            safeInputTokenLimit,
            {
              systemPrompt: finalSystemPrompt,
              toolsJson,
              maxMessages:
                (contextStrategy ?? 'window') === 'compact' &&
                backendOwnedCompaction
                  ? undefined
                  : windowSize,
              maxToolCallsPerMessage:
                provider === AIServiceProvider.Gemini
                  ? 100
                  : toolCallGroupVisibleCount || 4,
            },
          );

          const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
            contextMessages.map(sanitizeMessage),
            provider as AIServiceProvider,
          );
          logger.info('✅ Messages sanitized for provider compatibility', {
            sessionId,
            originalCount: contextMessages.length,
            safeCount: safeMessages.length,
          });

          const windowEnrichedMessages =
            await prepareMessagesForLLM(safeMessages);

          const attachmentCount = windowEnrichedMessages.reduce(
            (total, msg) => total + (msg.attachments?.length || 0),
            0,
          );
          if (attachmentCount > 0) {
            logger.info('📎 Messages enriched with attachment metadata', {
              sessionId,
              attachmentCount,
              messagesWithAttachments: windowEnrichedMessages.filter(
                (m) => m.attachments && m.attachments.length > 0,
              ).length,
            });
          }

          const systemPromptTokens = finalSystemPrompt
            ? estimateTextTokens(finalSystemPrompt)
            : 0;
          const toolsTokens = toolsJson ? estimateTextTokens(toolsJson) : 0;
          const totalEstimatedTokens = calculateGroundedTotalTokens(
            windowEnrichedMessages,
            systemPromptTokens + sessionContextTokens,
            toolsTokens,
          );

          if (safeMessages.length < messages.length) {
            logger.info(
              'Messages truncated/sanitized to fit context/window size',
              {
                originalCount: messages.length,
                newCount: safeMessages.length,
                windowSize,
                provider,
                model,
                safeInputTokenLimit,
                estimatedPromptTokens: totalEstimatedTokens,
              },
            );
          }

          // Update context usage for gauge display
          setContextUsageMap((prev) => {
            const next = new Map(prev);
            next.set(sessionId, {
              totalTokens: totalEstimatedTokens,
              contextWindow: safeInputTokenLimit,
              modelMaxContext: modelMaxLimit,
            });
            return next;
          });

          enrichedMessages = windowEnrichedMessages;
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
          finalSystemPrompt,
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
    [updateSessionStatus, settingsRef, setStreamingMessages],
  );

  const cancelCompletionRequest = useCallback((sessionId: string) => {
    logger.info('Manually cancelling completion request', { sessionId });
    const abortController = abortControllersRef.current.get(sessionId);
    if (abortController) {
      abortController.abort();
    }
  }, []);

  const clearSessionState = useCallback((sessionId: string) => {
    compactCacheRef.current.delete(sessionId);
    compactResolversRef.current.delete(sessionId);
    setCompactingSet((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Set(prev);
      next.delete(sessionId);
      return next;
    });
    setAwaitingSet((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Set(prev);
      next.delete(sessionId);
      return next;
    });
    setContextUsageMap((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
    setCompactedRangeMap((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  }, []);

  /**
   * Clears in-memory compact state for ALL sessions.
   * Called when the global context strategy changes so stale caches,
   * pending resolvers, and UI state don't leak across modes.
   */
  const clearAllCompactState = useCallback(() => {
    compactCacheRef.current.clear();
    compactResolversRef.current.clear();
    setCompactingSet(new Set());
    setAwaitingSet(new Set());
    setContextUsageMap(new Map());
    setCompactedRangeMap(new Map());
  }, []);

  const applyCompactionState = useCallback(
    (event: CompactionStateEvent) => {
      setCompactingSet((prev) => {
        const next = new Set(prev);
        if (event.status === 'compacting') {
          next.add(event.sessionId);
        } else {
          next.delete(event.sessionId);
        }
        return next;
      });

      setAwaitingSet((prev) => {
        const next = new Set(prev);
        if (event.status === 'awaiting') {
          next.add(event.sessionId);
        } else {
          next.delete(event.sessionId);
        }
        return next;
      });

      setContextUsageMap((prev) => {
        const next = new Map(prev);
        if (event.contextUsage) {
          next.set(event.sessionId, event.contextUsage);
        } else {
          next.delete(event.sessionId);
        }
        return next;
      });

      setCompactedRangeMap((prev) => {
        const next = new Map(prev);
        if (event.compactedRange) {
          next.set(event.sessionId, event.compactedRange);
        } else {
          next.delete(event.sessionId);
        }
        return next;
      });
    },
    [],
  );

  return {
    executeCompletionRequest,
    applyCompactionState,
    cancelCompletionRequest,
    clearSessionState,
    clearAllCompactState,
    isCompacting: (sessionId: string) => compactingSet.has(sessionId),
    isAwaitingCompact: (sessionId: string) => awaitingSet.has(sessionId),
    getContextUsage: (
      sessionId: string,
    ):
      | { totalTokens: number; contextWindow: number; modelMaxContext?: number }
      | undefined => contextUsageMap.get(sessionId),
    getCompactedRange: (
      sessionId: string,
    ): { fromId: string; toId: string } | undefined =>
      compactedRangeMap.get(sessionId),
  };
}
