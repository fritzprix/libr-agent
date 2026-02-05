import { useCallback, MutableRefObject } from 'react';
import { Message } from '@/models/chat';
import { MCPTool } from '@/lib/mcp-types';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import {
  AIServiceConfig,
  AIServiceProvider,
  IAIService,
} from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import {
  selectMessagesWithinContext,
  estimateTokensBPE,
} from '@/lib/token-utils';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import { processLLMStream } from './stream-processor';
import { SessionStatus } from './types';
import { Settings } from '@/lib/services/settings-service';
import { ToolCall } from '@/models/chat';

const logger = getLogger('CompletionExecutor');

interface UseCompletionExecutorProps {
  settingsRef: MutableRefObject<Settings>;
  streamingMessages: Map<string, Partial<Message>>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => void;
  sessionAgentModes: Map<string, boolean>;
  activeServicesRef: MutableRefObject<Map<string, IAIService>>;
  abortControllersRef: MutableRefObject<Map<string, AbortController>>;
  timeoutsRef: MutableRefObject<Map<string, number>>;
}

export function useCompletionExecutor({
  settingsRef,
  streamingMessages,
  setStreamingMessages,
  updateSessionStatus,
  sessionAgentModes,
  activeServicesRef,
  abortControllersRef,
  timeoutsRef,
}: UseCompletionExecutorProps) {
  const executeCompletionRequest = useCallback(
    async (
      sessionId: string,
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
        firstMessageId: messages[0]?.id ?? 'none',
        lastMessageId: messages[messages.length - 1]?.id ?? 'none',
      });

      logger.debug('📝 Messages being sent to LLM service', {
        sessionId,
        messages: messages.map((m, idx) => ({
          index: idx,
          id: m.id,
          role: m.role,
          contentPreview:
            m.content?.[0]?.type === 'text'
              ? m.content[0].text.substring(0, 50) + '...'
              : `[${m.content?.length ?? 0} items]`,
          hasToolCalls: !!m.tool_calls,
          toolCallCount: m.tool_calls?.length ?? 0,
          toolCallId: m.tool_call_id,
        })),
      });

      // 🔍 Log available tools in detail
      logger.info('🔧 Available Tools Summary', {
        sessionId,
        totalToolCount: availableTools?.length ?? 0,
        toolsByServer: availableTools?.reduce(
          (acc, tool) => {
            const serverName = tool.name.split('__')[0] || 'unknown';
            acc[serverName] = (acc[serverName] || 0) + 1;
            return acc;
          },
          {} as Record<string, number>,
        ),
      });

      logger.debug('🔧 Available Tools Detail', {
        sessionId,
        tools: availableTools?.map((tool) => ({
          name: tool.name,
          description: tool.description?.substring(0, 100),
          hasInputSchema: !!tool.inputSchema,
        })),
      });

      const finalSystemPrompt = systemPrompt;

      // 🔍 Log system prompt
      logger.info('📋 System Prompt Configuration', {
        sessionId,
        finalPromptLength: finalSystemPrompt?.length ?? 0,
        systemPromptPreview: finalSystemPrompt?.substring(0, 200) + '...',
        includesSkills:
          finalSystemPrompt?.includes('<available_skills>') ?? false,
      });

      if (finalSystemPrompt) {
        logger.debug('📋 Full System Prompt', {
          sessionId,
          systemPrompt: finalSystemPrompt,
        });
      }

      // Update status to streaming
      updateSessionStatus(sessionId, 'streaming');

      // Create abort controller for this request
      const abortController = new AbortController();
      abortControllersRef.current.set(sessionId, abortController);

      try {
        // Get service instance with provider-specific configuration
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

        // Build config
        const config: AIServiceConfig = {
          temperature,
          maxTokens:
            maxTokens ||
            settingsRef.current.advanced?.defaultMaxOutputTokens ||
            8192,
        };

        // Calculate safe input token limit
        let safeInputTokenLimit: number | undefined;
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

        if (modelInfo) {
          // Use resolved max tokens or fallback
          const effectiveMaxTokens =
            maxTokens ||
            settingsRef.current.advanced?.defaultMaxOutputTokens ||
            8192;

          // Reserve maxTokens + 100 safety buffer
          const reserved = effectiveMaxTokens + 100;
          if (reserved < modelInfo.contextWindow) {
            safeInputTokenLimit = modelInfo.contextWindow - reserved;
          }
        }

        // Select messages within context window and message count limit
        const { windowSize } = settingsRef.current;
        logger.info('🎯 Applying windowSize constraint from settings', {
          sessionId,
          inputMessageCount: messages.length,
          windowSize,
          provider,
          model,
          safeInputTokenLimit: safeInputTokenLimit || 'auto(90%)',
        });
        const contextMessages = selectMessagesWithinContext(
          messages,
          provider,
          model,
          safeInputTokenLimit,
          {
            systemPrompt: finalSystemPrompt,
            maxMessages: windowSize,
          },
        );

        // Sanitize messages
        const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
          contextMessages.map(sanitizeMessage),
          provider as AIServiceProvider,
        );
        logger.info('✅ Messages sanitized for provider compatibility', {
          sessionId,
          originalCount: contextMessages.length,
          safeCount: safeMessages.length,
        });

        // Preprocess messages
        const enrichedMessages = await prepareMessagesForLLM(safeMessages);

        // Log attachment enrichment
        const attachmentCount = enrichedMessages.reduce(
          (total, msg) => total + (msg.attachments?.length || 0),
          0,
        );
        if (attachmentCount > 0) {
          logger.info('📎 Messages enriched with attachment metadata', {
            sessionId,
            attachmentCount,
            messagesWithAttachments: enrichedMessages.filter(
              (m) => m.attachments && m.attachments.length > 0,
            ).length,
          });
        }

        // Measure final token count for logging
        const totalEstimatedTokens = enrichedMessages.reduce(
          (sum, msg) => sum + estimateTokensBPE(msg),
          0,
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
              totalEstimatedTokens,
            },
          );
        }

        logger.debug('Final Payload Token Estimate', {
          count: totalEstimatedTokens,
          limit: safeInputTokenLimit || 'auto(90%)',
        });

        // START TIMING
        const startTime = performance.now();

        // Create async generator for streaming
        const streamGenerator = service.streamChat(enrichedMessages, {
          modelName: model,
          systemPrompt: finalSystemPrompt,
          availableTools: availableTools || [],
          config,
          forceToolUse: sessionAgentModes.get(sessionId) ?? false,
        });

        // EXECUTE STREAM
        const { content, finalUsage, thinkingStartTime, firstChunkTime } =
          await processLLMStream(
            sessionId,
            streamGenerator,
            streamingMessage,
            {
              onUpdate: (updatedMessage) => {
                setStreamingMessages((prev) => {
                  const next = new Map(prev);
                  next.set(sessionId, updatedMessage);
                  return next;
                });
              },
              signal: abortController.signal,
            },
          );

        // Calculate final timing
        const endTime = performance.now();
        const totalDurationMs = endTime - startTime;

        const effectiveUsage = finalUsage;

        if (effectiveUsage && effectiveUsage.completionTokens > 0) {
          if (!effectiveUsage.details) {
            effectiveUsage.details = {};
          }
          if (!effectiveUsage.details.evalDuration) {
            if (firstChunkTime) {
              effectiveUsage.details.promptEvalDuration =
                firstChunkTime - startTime;
              effectiveUsage.details.evalDuration = endTime - firstChunkTime;
            } else {
              effectiveUsage.details.evalDuration = totalDurationMs;
            }
          }
          if (!effectiveUsage.details.timeToFirstToken && firstChunkTime) {
            effectiveUsage.details.timeToFirstToken =
              firstChunkTime - startTime;
          }
        }

        // Create final message with isStreaming: false
        // Derive legacy fields for final message
        const finalLegacyToolCalls: ToolCall[] = content
          .filter((c) => c.type === 'tool_call')
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          .map((c: any) => {
            return {
              id: c.id,
              type: 'function',
              function: {
                name: c.name,
                arguments: c.arguments,
              },
            };
          });

        const finalLegacyThinking = content
          .filter((c) => c.type === 'thinking')
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          .map((c: any) => c.thinking)
          .join('\n');

        const finalMessage: Message = {
          id: streamingMessage.id ?? `msg_${Date.now()}`,
          sessionId,
          threadId: sessionId,
          role: 'assistant',
          content,
          createdAt: new Date(),
          tool_calls:
            finalLegacyToolCalls.length > 0 ? finalLegacyToolCalls : undefined,
          thinking: finalLegacyThinking || undefined,
          thinkingTime: thinkingStartTime
            ? (performance.now() - thinkingStartTime) / 1000
            : undefined,
          usage: effectiveUsage,
          isStreaming: false,
        };

        logger.info('Completion request completed', {
          sessionId,
          contentLength: content.length,
          toolCallCount: finalLegacyToolCalls.length,
        });

        // Check for empty message
        const hasContent =
          (finalMessage.content && finalMessage.content.length > 0) ||
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

        // Set finalMessage
        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, finalMessage);
          return next;
        });

        // Clean up timeout
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
        abortControllersRef.current.delete(sessionId);
        activeServicesRef.current.delete(sessionId);

        return finalMessage;
      } catch (error) {
        logger.error('Completion request failed', error);

        updateSessionStatus(sessionId, 'error');

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
        activeServicesRef.current.delete(sessionId);

        throw error;
      }
    },
    [
      updateSessionStatus,
      settingsRef,
      streamingMessages,
      setStreamingMessages,
      sessionAgentModes,
      activeServicesRef,
      abortControllersRef,
      timeoutsRef,
    ],
  );

  return executeCompletionRequest;
}
