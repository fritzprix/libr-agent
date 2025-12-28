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
import type { IAIService } from '@/lib/ai-service/types';
import { useSettings } from './SettingsContext';

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
      logger.info('Executing completion request', {
        sessionId,
        messageCount: messages.length,
        provider,
        model,
      });

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

        // Initialize streaming message
        const streamingMessage: Partial<Message> = {
          id: `msg_${Date.now()}`,
          sessionId,
          threadId: sessionId, // For top-level thread: threadId === sessionId
          role: 'assistant',
          content: [],
          createdAt: new Date(),
        };

        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, streamingMessage);
          return next;
        });

        // Build config
        const config: AIServiceConfig = {
          temperature,
          maxTokens,
        };

        // Create async generator for streaming
        const streamGenerator = service.streamChat(messages, {
          modelName: model,
          systemPrompt,
          availableTools: availableTools || [],
          config,
        });

        // Accumulate chunks
        let fullContent = '';
        let toolCalls: ToolCall[] = [];
        let thinkingContent = '';

        for await (const chunk of streamGenerator) {
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
                if (toolCallChunk.function?.arguments) {
                  toolCalls[index].function.arguments +=
                    toolCallChunk.function.arguments;
                }
              } else {
                toolCalls[index] = toolCallChunk;
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

          // Update streaming message state (throttled to reduce renders)
          setStreamingMessages((prev) => {
            const next = new Map(prev);
            next.set(sessionId, {
              ...streamingMessage,
              content: fullContent
                ? [{ type: 'text' as const, text: fullContent }]
                : [],
              tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
              thinking: thinkingContent || undefined,
            });
            return next;
          });
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
          isStreaming: false, // ✅ Explicit completion flag to trigger AgentChatContext effect
        };

        logger.info('Completion request completed', {
          sessionId,
          contentLength: fullContent.length,
          toolCallCount: toolCalls.length,
        });

        // ✅ Set finalMessage to trigger AgentChatContext effect (idea.md architecture)
        setStreamingMessages((prev) => {
          const next = new Map(prev);
          next.set(sessionId, finalMessage);
          return next;
        });

        // AgentChatContext will add to messages array and handle cleanup
        // No setTimeout needed - effect-based persistence is more reliable

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
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      logger.info('Setting up LLM completion request listener');

      unlisten = await listen<CompletionRequest>(
        'llm:completion-request',
        async (event) => {
          const {
            sessionId,
            messages,
            model,
            provider,
            systemPrompt,
            temperature,
            maxTokens,
            availableTools,
          } = event.payload;

          // Always get API key from Settings, ignore any apiKey from Rust backend
          const finalApiKey =
            settingsRef.current.serviceConfigs?.[provider as AIServiceProvider]
              ?.apiKey || '';

          logger.debug('Received LLM completion request', {
            sessionId,
            messageCount: messages.length,
            toolCount: availableTools?.length ?? 0,
            provider,
            hasApiKey: !!finalApiKey,
            eventId: event.id, // Track event identity
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
            logger.debug('Sending LLM response to Rust', {
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
              toolCalls: result.tool_calls || undefined,
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

            logger.debug('Message prepared for Rust', {
              sessionId,
              hasToolCalls: !!messageForRust.toolCalls,
              toolCallCount: messageForRust.toolCalls?.length ?? 0,
              createdAtType: typeof messageForRust.createdAt,
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

      logger.info('LLM completion request listener registered');
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        logger.info('LLM completion request listener cleaned up');
      }

      // Cancel all active requests
      abortControllersRef.current.forEach((controller) => controller.abort());
      abortControllersRef.current.clear();

      // Dispose all active services
      activeServicesRef.current.forEach((service) => service.dispose());
      activeServicesRef.current.clear();
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
