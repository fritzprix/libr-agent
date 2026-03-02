import { useCallback, useRef, useEffect } from 'react';
import type {
  IAIService,
  TokenUsage,
  AIServiceConfig,
} from '@/lib/ai-service/types';
import { AIServiceProvider } from '@/lib/ai-service/types';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { Message, ToolCall } from '@/models/chat';
import type {
  MCPTool,
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp';
import type { Settings } from '@/context/SettingsContext';
import { getLogger } from '@/lib/logger';
import {
  selectMessagesWithinContext,
  estimateTokensBPE,
} from '@/lib/token-utils';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
import type { SessionStatus } from './types';
import { isAbortError } from './types';

const logger = getLogger('useLLMExecution');

interface UseLLMExecutionProps {
  settingsRef: React.MutableRefObject<Settings>;
  streamingMessages: Map<string, Partial<Message>>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => void;
  sessionAgentModes: Map<string, boolean>;
}

export function useLLMExecution({
  settingsRef,
  streamingMessages,
  setStreamingMessages,
  updateSessionStatus,
  sessionAgentModes,
}: UseLLMExecutionProps) {
  // Track active service instances for cleanup
  const activeServicesRef = useRef<Map<string, IAIService>>(new Map());
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  // Track last streaming UI update time per session (throttle to ~20fps)
  const lastStreamingUpdateRef = useRef<Map<string, number>>(new Map());

  // Clean up on unmount
  useEffect(() => {
    return () => {
      // Cancel all active requests
      abortControllersRef.current.forEach((controller) => controller.abort());
      abortControllersRef.current.clear();

      // Clear all pending timeouts
      timeoutsRef.current.forEach((timeoutId) =>
        window.clearTimeout(timeoutId),
      );
      timeoutsRef.current.clear();

      // Dispose all active services
      activeServicesRef.current.forEach((service) => service.dispose());
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

      // System prompt
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

      // Cancel and dispose any previously active request for this session
      // This can happen if a new LLM turn starts before a prior cancelled request
      // fully resolved its async cleanup.
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

      // Get service instance before the try block so it's accessible in catch
      const providerConfig =
        settingsRef.current.serviceConfigs?.[provider as AIServiceProvider] ||
        {};
      const service = AIServiceFactory.getService(
        provider as AIServiceProvider,
        apiKey ?? '',
        providerConfig,
      );
      activeServicesRef.current.set(sessionId, service);

      try {
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
          maxTokens:
            maxTokens ||
            settingsRef.current.advanced?.defaultMaxOutputTokens ||
            8192,
          temperature: temperature,
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
            settingsRef.current.advanced?.defaultMaxOutputTokens || 8192;

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
            // Gemini requires all tool calls in a single turn to maintain thought signature validity
            // Splitting messages (batching) breaks this as subsequent batches lack the signature
            maxToolCallsPerMessage:
              provider === AIServiceProvider.Gemini ? 100 : 4,
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

        const enrichedMessages = await prepareMessagesForLLM(safeMessages);

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

        const startTime = performance.now();

        const streamGenerator = service.streamChat(enrichedMessages, {
          modelName: model,
          systemPrompt: finalSystemPrompt,
          availableTools: availableTools || [],
          config,
          forceToolUse: sessionAgentModes.get(sessionId) ?? false,
        });

        const content: MCPContent[] = [];
        const activeToolCallIndices = new Map<number, number>();

        let thinkingStartTime: number | undefined;
        let currentThinkingTime: number | undefined;
        let finalUsage: TokenUsage | undefined;
        let firstChunkTime: number | undefined;
        let thinkingSignature: string | undefined;

        for await (const chunk of streamGenerator) {
          if (firstChunkTime === undefined) {
            firstChunkTime = performance.now();
          }

          if (abortController.signal.aborted) {
            logger.warn('Completion request aborted', { sessionId });
            throw new Error('Request aborted');
          }

          let parsedChunk: Record<string, unknown>;
          try {
            parsedChunk = JSON.parse(chunk);
          } catch {
            parsedChunk = { content: chunk };
          }

          // 1. Accumulate Content (Text)
          if (parsedChunk.content && typeof parsedChunk.content === 'string') {
            const lastItem = content[content.length - 1];
            if (lastItem && lastItem.type === 'text') {
              (lastItem as MCPTextContent).text += parsedChunk.content;
            } else {
              content.push({ type: 'text', text: parsedChunk.content });
            }
          }

          // 2. Accumulate Thinking
          if (
            parsedChunk.thinking &&
            typeof parsedChunk.thinking === 'string'
          ) {
            if (thinkingStartTime === undefined) {
              thinkingStartTime = performance.now();
            }

            const lastItem = content[content.length - 1];
            if (lastItem && lastItem.type === 'thinking') {
              (lastItem as MCPThinkingContent).thinking += parsedChunk.thinking;
            } else {
              content.push({
                type: 'thinking',
                thinking: parsedChunk.thinking,
              });
            }
          }

          // 3. Accumulate Thinking Signature
          if (
            parsedChunk.thinkingSignature &&
            typeof parsedChunk.thinkingSignature === 'string'
          ) {
            thinkingSignature = parsedChunk.thinkingSignature;
            logger.debug('🧠 Captured thinking signature', {
              sessionId,
              signatureLength: thinkingSignature.length,
            });
          }

          // 4. Accumulate Tool Calls
          if (parsedChunk.tool_calls && Array.isArray(parsedChunk.tool_calls)) {
            (
              parsedChunk.tool_calls as (ToolCall & { index?: number })[]
            ).forEach((toolCallChunk) => {
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
                const targetBlock = content[contentIndex] as MCPToolCallContent;

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
            });
          }

          if (thinkingStartTime !== undefined) {
            currentThinkingTime =
              (performance.now() - thinkingStartTime) / 1000;
          }

          if (parsedChunk.usage) {
            const incomingUsage = parsedChunk.usage as TokenUsage;
            if (finalUsage) {
              finalUsage = {
                promptTokens:
                  incomingUsage.promptTokens || finalUsage.promptTokens,
                completionTokens:
                  incomingUsage.completionTokens || finalUsage.completionTokens,
                totalTokens:
                  incomingUsage.totalTokens || finalUsage.totalTokens,
                details: {
                  ...finalUsage.details,
                  ...incomingUsage.details,
                },
              };
            } else {
              finalUsage = incomingUsage;
            }
          }

          // Throttle React state updates to ~20fps (50ms) to avoid WebView GPU overload
          // Content accumulation above still happens on every chunk (cheap)
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
                    function: {
                      name: tc.name,
                      arguments: tc.arguments,
                    },
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

        // Always flush final streaming state after loop ends (ensures last content is visible)
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
                function: {
                  name: tc.name,
                  arguments: tc.arguments,
                },
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
              function: {
                name: tc.name,
                arguments: tc.arguments,
              },
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

        // Only clean up this execution's controller/service if they're still registered
        // (a new execution may have already replaced them)
        if (abortControllersRef.current.get(sessionId) === abortController) {
          abortControllersRef.current.delete(sessionId);
        }
        if (activeServicesRef.current.get(sessionId) === service) {
          activeServicesRef.current.delete(sessionId);
        }

        return finalMessage;
      } catch (error) {
        // Distinguish intentional abort (user cancel) from real errors.
        // AbortError means cancelCompletionRequest() fired — this is NOT an error.
        const isAborted = isAbortError(error);

        if (isAborted) {
          logger.info('Completion request was aborted by cancellation', {
            sessionId,
          });
        } else {
          logger.error('Completion request failed', error);
        }

        // Only update session status and clean up streaming state if this execution's
        // controller is still the active one — if a new execution started, do not stomp
        // on its state.
        if (abortControllersRef.current.get(sessionId) === abortController) {
          // Set 'idle' for intentional aborts, 'error' for real failures.
          // Without this distinction, cancellation leaves the local LLM status as
          // 'error' while the Rust workflow correctly transitions to 'idle'.
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
    [
      updateSessionStatus,
      settingsRef,
      streamingMessages,
      setStreamingMessages,
      sessionAgentModes,
    ],
  );

  const cancelCompletionRequest = useCallback((sessionId: string) => {
    logger.info('Manually cancelling completion request', { sessionId });
    const abortController = abortControllersRef.current.get(sessionId);
    if (abortController) {
      abortController.abort();
    }
  }, []);

  return { executeCompletionRequest, cancelCompletionRequest };
}
