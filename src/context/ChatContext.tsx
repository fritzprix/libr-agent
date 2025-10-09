import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useSessionContext } from './SessionContext';
import { useSessionHistory } from './SessionHistoryContext';
import { useAIService } from '../hooks/use-ai-service';
import { useAssistantContext } from './AssistantContext';
import { useBuiltInTool } from '@/features/tools';
import { useToolProcessor } from '../hooks/use-tool-processor';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message } from '@/models/chat';
import { useSettings } from '../hooks/use-settings';
import { AIServiceConfig } from '@/lib/ai-service';
import { useSystemPrompt } from './SystemPromptContext';
import {
  stringToMCPContentArray,
  extractBuiltInServiceAlias,
} from '@/lib/utils';
import { MCPTool } from '@/lib/mcp-types';

const logger = getLogger('ChatContext');

// Local guard for error-shaped objects returned by useAIService
function isErrorClassification(obj: unknown): obj is {
  displayMessage: string;
  type: string;
  recoverable: boolean;
  details?: Record<string, unknown>;
} {
  if (!obj || typeof obj !== 'object') return false;
  const cast = obj as Record<string, unknown>;
  return (
    typeof cast.displayMessage === 'string' &&
    typeof cast.type === 'string' &&
    typeof cast.recoverable === 'boolean'
  );
}

// --- STATE CONTEXT ---
interface ChatStateContextValue {
  isLoading: boolean;
  isToolExecuting: boolean;
  messages: Message[];
  pendingCancel: boolean;
  // Current top-level assistant error (if any) and the message id it came from
  error: Message['error'] | null;
}

const ChatStateContext = createContext<ChatStateContextValue | undefined>(
  undefined,
);

// --- ACTIONS CONTEXT ---
interface ChatActionsContextValue {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
  cancel: () => void;
  addToMessageQueue: (message: Partial<Message>) => void;
  retryMessage: () => Promise<void>;
}

const ChatActionsContext = createContext<ChatActionsContextValue | undefined>(
  undefined,
);

interface ChatProviderProps {
  children: React.ReactNode;
}

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

export function ChatProvider({ children }: ChatProviderProps) {
  const { messages: history, addMessage, addMessages } = useSessionHistory();
  const { current: currentSession } = useSessionContext();
  const { value: settingValue } = useSettings();
  const { currentAssistant, availableTools } = useAssistantContext();
  const { getSystemPrompt } = useSystemPrompt();
  const { availableTools: builtInTools } = useBuiltInTool();
  const cancelRequestRef = useRef(false);

  const [streamingMessage, setStreamingMessage] = useState<Message | null>(
    null,
  );
  const [error, setError] = useState<Message['error'] | null>(null);
  const [pendingCancel, setPendingCancel] = useState(false);
  const [messageQueue, setMessageQueue] = useState<Message[]>([]);

  // Extract window size with default fallback
  const messageWindowSize = settingValue?.windowSize ?? 20;

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      setStreamingMessage(null);
      setPendingCancel(false);
      setMessageQueue([]);
    };
  }, []);

  const buildSystemPrompt = useCallback(async (): Promise<string> => {
    const basePrompt = currentAssistant?.systemPrompt || DEFAULT_SYSTEM_PROMPT;
    const extensionPrompt = await getSystemPrompt();
    const combined = [basePrompt, extensionPrompt].filter(Boolean).join('\n\n');
    logger.info('Built combined system prompt', {
      baseLength: basePrompt.length,
      extensionLength: extensionPrompt.length,
      totalLength: combined.length,
    });
    logger.info('Built combined system prompt', {
      basePrompt: basePrompt,
      extensionPrompt: extensionPrompt,
    });
    return combined;
  }, [currentAssistant, getSystemPrompt]);

  // Message queue management function
  const addToMessageQueue = useCallback(
    (message: Partial<Message>) => {
      if (!currentSession?.id) {
        logger.warn('No current session available for queuing message');
        return;
      }

      const queuedMessage: Message = {
        id: createId(),
        role: 'user',
        sessionId: currentSession.id,
        content: stringToMCPContentArray(''),
        ...message,
      };

      setMessageQueue((prev) => [...prev, queuedMessage]);
      logger.info('Message added to queue', {
        messageId: queuedMessage.id,
        queueLength: messageQueue.length + 1,
      });
    },
    [currentSession, messageQueue.length],
  );

  // Pre-compute tool-to-alias mapping for performance
  const toolAliasMap = useMemo(() => {
    const map = new Map<string, string>();
    builtInTools.forEach((tool) => {
      const alias = extractBuiltInServiceAlias(tool.name);
      if (alias) {
        map.set(tool.name, alias);
      }
    });
    return map;
  }, [builtInTools]);

  // Filter built-in tools based on assistant's allowed aliases
  const filterBuiltInTools = useCallback(
    (tools: MCPTool[]): MCPTool[] => {
      try {
        const allowedAliases = currentAssistant?.allowedBuiltInServiceAliases;

        if (allowedAliases === undefined) {
          logger.debug('No built-in tool restrictions for assistant', {
            assistant: currentAssistant?.name,
          });
          return tools;
        }

        if (allowedAliases.length === 0) {
          logger.debug('All built-in tools disabled for assistant', {
            assistant: currentAssistant?.name,
          });
          return [];
        }

        // Filter tools based on allowed aliases
        const filteredTools = tools.filter((tool) => {
          const toolAlias = toolAliasMap.get(tool.name);
          if (!toolAlias) {
            return false;
          }
          return allowedAliases.includes(toolAlias);
        });

        logger.debug('Filtered built-in tools', {
          assistant: currentAssistant?.name,
          allowedAliases,
          totalTools: tools.length,
          filteredTools: filteredTools.length,
        });

        return filteredTools;
      } catch (error) {
        logger.error('Built-in tool filtering failed, allowing all tools', {
          error,
        });
        return tools; // Fallback: allow all tools on error
      }
    },
    [currentAssistant, toolAliasMap],
  );

  // AI Service configuration with tools only
  const aiServiceConfig = useMemo(
    (): AIServiceConfig => ({
      tools: [...availableTools, ...filterBuiltInTools(builtInTools)],
      maxRetries: 3,
      maxTokens: 4096,
    }),
    [availableTools, builtInTools, filterBuiltInTools],
  );

  const {
    submit: triggerAIService,
    isLoading: aiServiceLoading,
    response,
    cancel: cancelAIService,
  } = useAIService(aiServiceConfig);

  // Routine to initialize streamingMessage on session change (to resolve timing issues)
  useEffect(() => {
    // Always cancel any in-flight AI stream when session changes (including null)
    logger.info('Session changed, cancelling active AI streams', {
      newSessionId: currentSession?.id ?? null,
    });

    try {
      cancelAIService?.();
    } catch (err) {
      logger.warn('AI service cancel() threw error during session switch', {
        err,
      });
    }

    // reset internal cancel flag used to block submits
    cancelRequestRef.current = false;

    // Clear streaming and queue state to avoid stray UI
    setStreamingMessage(null);
    setPendingCancel(false);
    setMessageQueue([]);
    // Clear current error when switching sessions
    setError(null);
  }, [currentSession?.id, cancelAIService]); // Run when currentSession?.id changes

  // Combine history with streaming message, avoiding duplicates
  const messages = useMemo(() => {
    if (!streamingMessage) {
      return history;
    }

    // If the streaming message is an in-progress assistant message with no
    // content, no tool calls and no thinking text yet, don't expose it to the
    // UI as it results in an empty assistant bubble. Wait until some content
    // or tool output is available or the message finalizes.
    const isEmptyStreamingAssistant =
      streamingMessage.role === 'assistant' &&
      streamingMessage.isStreaming &&
      (!streamingMessage.content ||
        (Array.isArray(streamingMessage.content) &&
          streamingMessage.content.length === 0)) &&
      (!streamingMessage.tool_calls ||
        streamingMessage.tool_calls.length === 0) &&
      !streamingMessage.thinking;

    if (isEmptyStreamingAssistant) {
      return history;
    }

    // Ignore streamingMessage on session mismatch (to prevent race conditions)
    if (streamingMessage.sessionId !== currentSession?.id) {
      logger.warn(
        'Streaming message session mismatch in messages calculation',
        {
          streamingSessionId: streamingMessage.sessionId,
          currentSessionId: currentSession?.id,
        },
      );
      return history;
    }

    // Check if streaming message already exists in history as finalized
    const existingMessage = history.find(
      (message) => message.id === streamingMessage.id && !message.isStreaming,
    );

    return existingMessage
      ? history.map((msg) =>
          msg.id === streamingMessage.id
            ? { ...msg, ...streamingMessage }
            : msg,
        )
      : [...history, streamingMessage];
  }, [streamingMessage, history, currentSession?.id]);

  useEffect(() => {
    if (error) {
      setStreamingMessage(null);
    }
  }, [error]);

  // Handle AI service streaming responses
  useEffect(() => {
    if (!response) return;

    // Ignore response for different session (strengthen session validation)
    if (response.sessionId && response.sessionId !== currentSession?.id) {
      logger.warn('Ignoring response for different session', {
        responseSessionId: response.sessionId,
        currentSessionId: currentSession?.id,
      });
      return;
    }

    setStreamingMessage((previous) => {
      if (previous) {
        // Merge response with existing streaming message
        return { ...previous, ...response };
      }

      // Only create streaming message if we have a valid session
      if (!currentSession?.id) {
        logger.warn('Cannot create streaming message: no active session');
        return null;
      }

      // Create new streaming message with proper defaults
      return {
        ...response,
        id: response.id ?? createId(),
        content: response.content ?? '',
        role: 'assistant' as const,
        sessionId: currentSession.id,
        isStreaming: response.isStreaming !== false,
      };
    });

    // If the response contains an error object, expose it as currentError
    // and clear any transient streaming message so we don't show an empty
    // assistant bubble in the UI.
    const maybeError = response as unknown as { error?: unknown };
    if (
      maybeError &&
      maybeError.error &&
      isErrorClassification(maybeError.error)
    ) {
      setError(maybeError.error as Message['error']);
      // Clear any transient streaming message created from this response
      // (error responses should not create a visible assistant bubble).
      setStreamingMessage(null);
    } else {
      // If we received a non-error response that corresponds to the current
      // error message id, clear the current error.
      setError(null);
    }
  }, [response, currentSession?.id]);

  // Clear streaming state when message is finalized in history
  useEffect(() => {
    if (!streamingMessage || streamingMessage.isStreaming) return;

    const isMessageInHistory = history.some(
      (message) => message.id === streamingMessage.id && !message.isStreaming,
    );

    if (isMessageInHistory) {
      logger.info('Message finalized in history, clearing streaming state', {
        messageId: streamingMessage.id,
      });
      setStreamingMessage(null);
    }
  }, [history, streamingMessage]);

  const submit = useCallback(
    async (messageToAdd?: Message[], agentKey?: string): Promise<Message> => {
      logger.info('submit ', { messageToAdd });
      if (!currentSession) {
        throw new Error('No active session available for message submission');
      }

      try {
        // Clear any previous streaming state before starting new request
        setStreamingMessage(null);

        let messagesToSend = messages;

        // Process and validate new messages if provided (to prevent loss of tool results)
        if (messageToAdd?.length) {
          const messagesWithSession = messageToAdd.map((m) => {
            if (!m.sessionId) {
              throw new Error('Cannot add message: missing sessionId');
            }
            return { ...m, sessionId: m.sessionId };
          });
          if (typeof addMessages === 'function') {
            await addMessages(messagesWithSession);
            messagesToSend = [...messages, ...messagesWithSession];
          } else {
            // Safe fallback: sequential saving
            const persisted: Message[] = [];
            for (const msg of messagesWithSession) {
              const added = await addMessage(msg);
              persisted.push(added);
            }
            messagesToSend = [...messages, ...persisted];
          }
        }

        // Move cancel check to after message addition (to preserve tool results)
        if (cancelRequestRef.current) {
          cancelRequestRef.current = false;
          logger.info('Request cancelled after message persistence');
          return {
            id: createId(),
            content: stringToMCPContentArray('Request cancelled'),
            role: 'system',
            sessionId: currentSession.id,
            isStreaming: false,
          };
        }

        // Get windowed messages (excluding system prompts from history)
        const userMessages = messagesToSend
          .filter((msg) => msg.role !== 'system')
          .slice(-messageWindowSize);

        // Combine system prompts with user messages
        const finalMessages = [...userMessages];

        logger.debug('Submitting messages with system prompts', {
          userMessagesCount: userMessages.length,
          agentKey,
        });

        // Send combined messages to AI service with dynamic system prompt
        const aiResponse = await triggerAIService(
          finalMessages,
          buildSystemPrompt,
        );

        // Handle AI response persistence
        if (aiResponse) {
          // If the AI service returned an error object, we should NOT persist
          // that error as a new assistant message in the session history.
          // Instead expose it via currentError and let the UI render a
          // non-persistent ErrorBubble and allow retry against the last
          // user message.
          if (aiResponse.error) {
            logger.info(
              'AI response contained an error; exposing as currentError',
              {
                error: aiResponse.error,
              },
            );

            setError(aiResponse.error);
            return {
              ...aiResponse,
              isStreaming: false,
              sessionId: currentSession.id,
            } as Message;
          }

          // Normal successful response: persist as assistant message
          const finalizedMessage: Message = {
            ...aiResponse,
            isStreaming: false,
            sessionId: currentSession.id,
          };

          logger.info('Finalizing AI response', {
            messageId: finalizedMessage.id,
            agentKey,
          });

          // Update streaming state and persist to history
          setStreamingMessage(finalizedMessage);
          await addMessage(finalizedMessage);

          // Clear any transient error state on successful response. Errors are
          // treated as temporary UI-only state and should be removed when a
          // real assistant response arrives.
          setError(null);

          return finalizedMessage;
        }

        // Handle case where no response was received
        throw new Error('No response received from AI service');
      } catch (error) {
        logger.error('Message submission failed', { error, agentKey });
        setError({
          displayMessage: 'Message submission failed. Please try again.',
          type: 'SUBMIT_FAILED',
          recoverable: true,
          details: {
            originalError: error,
            errorCode: 'SUBMIT_FAILED',
            timestamp: new Date().toISOString(),
          },
        });
        throw error;
      }
    },
    [
      currentSession,
      messages,
      messageWindowSize,
      triggerAIService,
      addMessage,
      addMessages,
      buildSystemPrompt,
    ],
  );

  const retryMessage = useCallback(async (): Promise<void> => {
    await submit([]);
  }, [submit]);

  const handleCancel = useCallback(() => {
    setPendingCancel(true);
    cancelRequestRef.current = true;

    // Call the AI service cancel if available to abort in-flight streams
    try {
      cancelAIService?.();
    } catch (err) {
      logger.warn('AI service cancel threw an error', { err });
    }

    // Reset pendingCancel after a delay to show visual feedback
    setTimeout(() => {
      setPendingCancel(false);
    }, 1000);
  }, [cancelAIService]);

  // Tool processor will be initialized after submit is defined
  const { processToolCalls, isProcessing } = useToolProcessor({
    submit,
  });
  // Process queued messages when tool execution completes
  useEffect(() => {
    if (!isProcessing && messageQueue.length > 0) {
      const nextMessage = messageQueue[0];
      logger.info('Processing queued message', {
        messageId: nextMessage.id,
        remainingInQueue: messageQueue.length - 1,
      });

      setMessageQueue((prev) => prev.slice(1));
      submit([nextMessage]);
    }
  }, [isProcessing, messageQueue, submit]);

  // Process tool calls when messages change
  useEffect(() => {
    const lastMessage = messages[messages.length - 1];
    if (lastMessage) {
      processToolCalls(lastMessage);
    }
  }, [messages, processToolCalls]);

  // Combined loading state: AI service loading OR tool execution
  const isLoading = aiServiceLoading || isProcessing;

  const stateValue: ChatStateContextValue = useMemo(
    () => ({
      isLoading,
      isToolExecuting: isProcessing,
      messages,
      pendingCancel,
      error,
    }),
    [isLoading, isProcessing, messages, pendingCancel, error],
  );

  const actionsValue: ChatActionsContextValue = useMemo(
    () => ({
      submit,
      cancel: handleCancel,
      addToMessageQueue,
      retryMessage,
    }),
    [submit, handleCancel, addToMessageQueue, retryMessage],
  );

  return (
    <ChatStateContext.Provider value={stateValue}>
      <ChatActionsContext.Provider value={actionsValue}>
        {children}
      </ChatActionsContext.Provider>
    </ChatStateContext.Provider>
  );
}

export function useChatState(): ChatStateContextValue {
  const context = useContext(ChatStateContext);
  if (context === undefined) {
    throw new Error('useChatState must be used within a ChatProvider');
  }
  return context;
}

export function useChatActions(): ChatActionsContextValue {
  const context = useContext(ChatActionsContext);
  if (context === undefined) {
    throw new Error('useChatActions must be used within a ChatProvider');
  }
  return context;
}
