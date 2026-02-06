import { useCallback, MutableRefObject } from 'react';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { AIServiceConfig, AIServiceProvider } from '@/lib/ai-service/types';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import {
  selectMessagesWithinContext,
  estimateTokensBPE,
} from '@/lib/token-utils';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import { StreamAccumulator } from './stream-processor';
import type { useLLMState } from './useLLMState';
import type { SettingsContextType } from '../SettingsContext';

const logger = getLogger('LLMServiceExecutor');

type LLMState = ReturnType<typeof useLLMState>;

export function useCompletionExecutor(
  state: LLMState,
  settingsRef: MutableRefObject<SettingsContextType['value']>,
) {
  const {
    setStreamingMessages,
    updateSessionStatus,
    abortControllersRef,
    activeServicesRef,
    sessionAgentModes,
    timeoutsRef,
  } = state;

  /**
   * Execute a completion request
   * Streams the response and returns the final message
   */
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

      // System prompt is now built entirely in Rust via ContextProvider framework
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

        // Initialize streaming message
        const streamingMessage: Partial<Message> = {
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

        // Preprocess messages to include attachment information
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

        // Create async generator for streaming
        const streamGenerator = service.streamChat(enrichedMessages, {
          modelName: model,
          systemPrompt: finalSystemPrompt,
          availableTools: availableTools || [],
          config,
          forceToolUse: sessionAgentModes.get(sessionId) ?? false,
        });

        // Use StreamAccumulator to handle chunk processing
        const accumulator = new StreamAccumulator();

        for await (const chunk of streamGenerator) {
          // Check if aborted
          if (abortController.signal.aborted) {
            logger.warn('Completion request aborted', { sessionId });
            throw new Error('Request aborted');
          }

          // Process the chunk
          accumulator.processChunk(chunk);

          // Update streaming message state
          setStreamingMessages((prev) => {
            const next = new Map(prev);

            const legacyToolCalls = accumulator.getLegacyToolCalls();
            const legacyThinking = accumulator.getLegacyThinking();

            next.set(sessionId, {
              ...streamingMessage,
              content: accumulator.content,
              tool_calls:
                legacyToolCalls.length > 0 ? legacyToolCalls : undefined,
              thinking: legacyThinking || undefined,
              thinkingTime: accumulator.getCurrentThinkingTime(),
              usage: accumulator.finalUsage,
              isStreaming: true,
            });
            return next;
          });
        }

        // Finalize usage
        const endTime = performance.now();
        const finalUsage = accumulator.finalizeUsage(endTime);

        // Create final message with isStreaming: false
        const finalLegacyToolCalls = accumulator.getLegacyToolCalls();
        const finalLegacyThinking = accumulator.getLegacyThinking();

        const finalMessage: Message = {
          id: streamingMessage.id ?? `msg_${Date.now()}`,
          sessionId,
          threadId: sessionId,
          role: 'assistant',
          content: accumulator.content,
          createdAt: new Date(),
          tool_calls:
            finalLegacyToolCalls.length > 0 ? finalLegacyToolCalls : undefined,
          thinking: finalLegacyThinking || undefined,
          thinkingTime: accumulator.getCurrentThinkingTime(),
          usage: finalUsage,
          isStreaming: false,
        };

        logger.info('Completion request completed', {
          sessionId,
          contentLength: accumulator.content.length,
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

        // Clear after a brief delay
        const timeoutId = window.setTimeout(() => {
          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.delete(sessionId);
            return next;
          });
          timeoutsRef.current.delete(sessionId);
        }, 100);
        timeoutsRef.current.set(sessionId, timeoutId);

        // Update status to idle
        updateSessionStatus(sessionId, 'idle');

        // Cleanup
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
    [updateSessionStatus, settingsRef, setStreamingMessages, activeServicesRef, abortControllersRef, timeoutsRef, sessionAgentModes],
  );

  return { executeCompletionRequest };
}
