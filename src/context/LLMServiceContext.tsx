import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { AIServiceConfig } from '@/lib/ai-service/types';
import { AIServiceProvider } from '@/lib/ai-service/types';
import type { Message, ToolCall } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import {
  selectMessagesWithinContext,
  estimateTokensBPE,
} from '@/lib/token-utils';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import type { IAIService, TokenUsage } from '@/lib/ai-service/types';
import { useSettings } from './SettingsContext';
import { useSystemPrompt } from './SystemPromptContext';

import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import { sanitizeMessage } from '@/lib/ai-service/sanitizer';
import { normalizeRustMessage } from '@/lib/ai-service/utils';
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';

const logger = getLogger('LLMServiceContext');

/**
 * Request from Rust backend to execute an LLM completion
 */
interface CompletionRequest {
  sessionId: string;
  messages: Message[];
  model: string;
  provider: string;
  apiKey?: string;
  systemPrompt?: string;
  temperature?: number;
  maxTokens?: number;
  availableTools?: MCPTool[];
}

/**
 * Status of LLM execution for a specific session
 */
type SessionStatus = 'idle' | 'streaming' | 'error';

/**
 * Context value for LLM Service Provider
 */
interface LLMServiceContextValue {
  /**
   * Map of session IDs to their current streaming messages
   */
  streamingMessages: Map<string, Partial<Message>>;

  /**
   * Get the current status of a session
   */
  getSessionStatus: (sessionId: string) => SessionStatus;

  /**
   * Clear streaming message for a session
   * Called by AgentChatContext after persisting to message stack
   */
  clearStreamingMessage: (sessionId: string) => void;

  /**
   * Execute a completion request for a session
   * This is invoked by Rust via IPC events
   */
  executeCompletionRequest: (
    sessionId: string,
    messages: Message[],
    model: string,
    provider: string,
    apiKey?: string,
    systemPrompt?: string,
    temperature?: number,
    maxTokens?: number,
  ) => Promise<Message>;
}

const LLMServiceContext = createContext<LLMServiceContextValue | undefined>(
  undefined,
);

/**
 * Hook to access LLM Service Context
 */
export function useLLMService(): LLMServiceContextValue {
  const context = useContext(LLMServiceContext);
  if (!context) {
    throw new Error('useLLMService must be used within LLMServiceProvider');
  }
  return context;
}

interface LLMServiceProviderProps {
  children: ReactNode;
}

/**
 * Global LLM Service Provider
 * Lives at the App level and never unmounts
 * Provides centralized LLM execution for both UI and Agent workflows
 */
export function LLMServiceProvider({ children }: LLMServiceProviderProps) {
  const { value: settings } = useSettings();
  const { getSystemPrompt } = useSystemPrompt();

  // Use ref to always access latest settings in event listeners
  const settingsRef = useRef(settings);
  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  const [streamingMessages, setStreamingMessages] = useState<
    Map<string, Partial<Message>>
  >(new Map());
  const [sessionStatuses, setSessionStatuses] = useState<
    Map<string, SessionStatus>
  >(new Map());

  // Track active service instances for cleanup
  const activeServicesRef = useRef<Map<string, IAIService>>(new Map());
  // Track abort controllers for cancellation
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  // Track timeout IDs for cleanup - using number for browser compatibility
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  // Track listener setup to prevent duplicate registration in React Strict Mode
  const listenerSetupRef = useRef(false);

  /**
   * Get session status
   */
  const getSessionStatus = useCallback(
    (sessionId: string): SessionStatus => {
      return sessionStatuses.get(sessionId) ?? 'idle';
    },
    [sessionStatuses],
  );

  /**
   * Update session status
   */
  const updateSessionStatus = useCallback(
    (sessionId: string, status: SessionStatus) => {
      setSessionStatuses((prev) => {
        const next = new Map(prev);
        next.set(sessionId, status);
        return next;
      });
    },
    [],
  );

  /**
   * Clear streaming message for a specific session
   * This is called by AgentChatContext after persisting the message
   */
  const clearStreamingMessage = useCallback((sessionId: string) => {
    logger.debug('Clearing streaming message', { sessionId });

    setStreamingMessages((prev) => {
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  }, []);

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

      // Fetch dynamic system prompt extensions (e.g. Time & Location)
      const dynamicSystemPrompt = await getSystemPrompt();

      // Combine with the provided system prompt (from Rust/Agent Config)
      const finalSystemPrompt = systemPrompt
        ? `${systemPrompt}\n\n${dynamicSystemPrompt}`
        : dynamicSystemPrompt;

      // 🔍 Log system prompt
      logger.info('📋 System Prompt Configuration', {
        sessionId,
        hasSystemPrompt: !!systemPrompt,
        hasDynamicPrompt: !!dynamicSystemPrompt,
        finalPromptLength: finalSystemPrompt?.length ?? 0,
        systemPromptPreview: finalSystemPrompt?.substring(0, 200) + '...',
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
        // Use settingsRef.current to access latest settings (not stale closure)
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
          maxTokens,
        };

        // Calculate safe input token limit
        // If maxTokens (max output) is specified, reserve strictly for it + safety buffer
        // Otherwise, fallback to selectMessagesWithinContext's default (90% of context window)
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

        if (modelInfo && maxTokens) {
          // Reserve maxTokens + 100 safety buffer
          const reserved = maxTokens + 100;
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

        // Sanitize messages to prevent malformed JSON and ensure provider compatibility
        // This includes:
        // 1. JSON escaping for tool arguments and thinking fields
        // 2. Tool call pairing validation (removing orphans/incomplete pairs)
        // 3. Provider-specific sanitization (e.g. removing thinking for OpenAI)
        const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
          contextMessages.map(sanitizeMessage),
          provider as unknown as AIServiceProvider,
        );
        logger.info('✅ Messages sanitized for provider compatibility', {
          sessionId,
          originalCount: contextMessages.length,
          safeCount: safeMessages.length,
        });

        // Preprocess messages to include attachment information
        // This enriches messages with attachment metadata and tool usage instructions
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

        // Measure final token count for logging (including attachment enrichment)
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
          forceToolUse: false,
        });

        // Accumulate chunks
        let fullContent = '';
        let toolCalls: ToolCall[] = [];
        let thinkingContent = '';
        let finalUsage: TokenUsage | undefined;
        let firstChunkTime: number | undefined;

        for await (const chunk of streamGenerator) {
          // Capture Time to First Token (TTFT) for detailed metrics
          if (firstChunkTime === undefined) {
            firstChunkTime = performance.now();
          }

          // Check if aborted
          if (abortController.signal.aborted) {
            logger.warn('Completion request aborted', { sessionId });
            throw new Error('Request aborted');
          }

          // Parse chunk (it's a JSON string)
          let parsedChunk: Record<string, unknown>;
          try {
            parsedChunk = JSON.parse(chunk);
          } catch {
            // If parsing fails, treat it as plain text content
            parsedChunk = { content: chunk };
          }
          // Accumulate content
          if (parsedChunk.content && typeof parsedChunk.content === 'string') {
            fullContent += parsedChunk.content;
          }

          // Accumulate tool calls
          if (parsedChunk.tool_calls && Array.isArray(parsedChunk.tool_calls)) {
            (
              parsedChunk.tool_calls as (ToolCall & { index?: number })[]
            ).forEach((toolCallChunk) => {
              const { index } = toolCallChunk;

              // If no index provided, treat as a complete tool call
              if (index === undefined) {
                toolCalls.push(toolCallChunk);
                return;
              }

              // Index-based merging for incremental chunks
              if (toolCalls[index]) {
                // ✅ Merge all fields, not just arguments
                if (toolCallChunk.id && !toolCalls[index].id) {
                  toolCalls[index].id = toolCallChunk.id;
                }
                if (toolCallChunk.type && !toolCalls[index].type) {
                  toolCalls[index].type = toolCallChunk.type;
                }
                if (toolCallChunk.function) {
                  if (!toolCalls[index].function) {
                    toolCalls[index].function = { name: '', arguments: '' };
                  }
                  if (
                    toolCallChunk.function.name &&
                    !toolCalls[index].function.name
                  ) {
                    toolCalls[index].function.name =
                      toolCallChunk.function.name;
                  }
                  if (toolCallChunk.function.arguments) {
                    toolCalls[index].function.arguments +=
                      toolCallChunk.function.arguments;
                  }
                }
              } else {
                // Initialize tool call at index
                toolCalls[index] = {
                  id: toolCallChunk.id || '',
                  type: toolCallChunk.type || 'function',
                  function: {
                    name: toolCallChunk.function?.name || '',
                    arguments: toolCallChunk.function?.arguments || '',
                  },
                };
              }
            });
          }

          // Accumulate thinking content
          if (
            parsedChunk.thinking &&
            typeof parsedChunk.thinking === 'string'
          ) {
            thinkingContent += parsedChunk.thinking;
          }

          // Accumulate usage metrics (merge instead of replace to preserve TTFT data)
          if (parsedChunk.usage) {
            const incomingUsage = parsedChunk.usage as TokenUsage;
            if (finalUsage) {
              // Merge usage data, preserving existing fields
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
              // First usage chunk
              finalUsage = incomingUsage;
            }
          }

          // Update streaming message state (no throttling - update on every chunk for responsiveness)
          // Note: React batching already reduces render overhead
          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.set(sessionId, {
              ...streamingMessage,
              content: fullContent
                ? [{ type: 'text' as const, text: fullContent }]
                : [],
              tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
              thinking: thinkingContent || undefined,
              usage: finalUsage,
            });
            return next;
          });
        }

        // Calculate final timing if usage data exists but lacks duration (e.g. OpenAI/Anthropic)
        const endTime = performance.now();
        const totalDurationMs = endTime - startTime;

        if (finalUsage && finalUsage.completionTokens > 0) {
          if (!finalUsage.details) {
            finalUsage.details = {};
          }
          // If provider didn't give duration, use calculated timings
          if (!finalUsage.details.evalDuration) {
            if (firstChunkTime) {
              // Approx: time to first chunk = proper prompt eval time
              // remaining time = generation time
              finalUsage.details.promptEvalDuration =
                firstChunkTime - startTime;
              finalUsage.details.evalDuration = endTime - firstChunkTime;
            } else {
              // Fallback if no chunks (shouldn't happen) or instant
              finalUsage.details.evalDuration = totalDurationMs;
            }
          }
          // If timeToFirstToken wasn't provided by the service, calculate it
          if (!finalUsage.details.timeToFirstToken && firstChunkTime) {
            finalUsage.details.timeToFirstToken = firstChunkTime - startTime;
          }
        }

        // Create final message with isStreaming: false
        const finalMessage: Message = {
          id: streamingMessage.id ?? `msg_${Date.now()}`,
          sessionId,
          threadId: sessionId, // For top-level thread: threadId === sessionId
          role: 'assistant',
          content: fullContent
            ? [{ type: 'text' as const, text: fullContent }]
            : [],
          createdAt: new Date(),
          tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
          thinking: thinkingContent || undefined,
          usage: finalUsage,
          isStreaming: false, // ✅ Explicit completion flag to trigger AgentChatContext effect
        };

        logger.info('Completion request completed', {
          sessionId,
          contentLength: fullContent.length,
          toolCallCount: toolCalls.length,
        });

        // Check for empty message (no content and no tool calls)
        // This prevents saving invalid messages to the history which would later be rejected by MessageNormalizer
        if (
          (!finalMessage.content || finalMessage.content.length === 0) &&
          (!finalMessage.tool_calls || finalMessage.tool_calls.length === 0) &&
          !finalMessage.thinking
        ) {
          throw new Error('Received empty response from LLM provider');
        }

        // ✅ Set finalMessage to trigger AgentChatContext effect (idea.md architecture)
        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, finalMessage);
          return next;
        });

        // ⏰ Clear after a brief delay to allow UI to process the final message
        // This fixes the bug where "Thinking..." indicator persists in idle state
        const timeoutId = window.setTimeout(() => {
          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.delete(sessionId);
            return next;
          });
          // Clean up timeout ID after execution
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

        // Update status to error
        updateSessionStatus(sessionId, 'error');

        // Clear streaming state
        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.delete(sessionId);
          return next;
        });

        // Clear any pending timeouts
        const timeoutId = timeoutsRef.current.get(sessionId);
        if (timeoutId) {
          clearTimeout(timeoutId);
          timeoutsRef.current.delete(sessionId);
        }

        // Cleanup
        abortControllersRef.current.delete(sessionId);
        activeServicesRef.current.delete(sessionId);

        throw error;
      }
    },
    [updateSessionStatus, settings],
  );

  /**
   * Listen for completion requests from Rust backend
   */
  /**
   * Listen for completion requests from Rust backend
   */
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
              createdAt: new Date(),
            });
            return next;
          });

          try {
            // Execute the completion with API key from Settings
            const result = await executeCompletionRequest(
              sessionId,
              messages,
              model,
              provider,
              finalApiKey,
              systemPrompt,
              temperature,
              maxTokens,
              availableTools,
            );

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

      // Reset listener setup ref on unmount
      listenerSetupRef.current = false;
    };
  }, []); // ⚠️ CRITICAL: Empty dependency array to prevent re-registering listener

  const value: LLMServiceContextValue = {
    streamingMessages,
    getSessionStatus,
    clearStreamingMessage,
    executeCompletionRequest,
  };

  return (
    <LLMServiceContext.Provider value={value}>
      {children}
    </LLMServiceContext.Provider>
  );
}
