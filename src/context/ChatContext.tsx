import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useSessionContext } from './SessionContext';
import { useSessionHistory } from './SessionHistoryContext';
import { useAIService } from '../hooks/use-ai-service';
import { useAssistantContext } from './AssistantContext';
import { useBuiltInTools } from './BuiltInToolContext';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message } from '@/models/chat';
import { useSettings } from '../hooks/use-settings';
import { AIServiceConfig } from '@/lib/ai-service';

const logger = getLogger('ChatContext');

interface SystemPromptExtension {
  id: string;
  content: string | (() => Promise<string>);
  priority: number; // Higher number = higher priority
}

interface ChatContextValue {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
  isLoading: boolean;
  messages: Message[];
  registerSystemPrompt: (extension: Omit<SystemPromptExtension, 'id'>) => string;
  unregisterSystemPrompt: (id: string) => void;
}

const ChatContext = createContext<ChatContextValue | undefined>(undefined);

const validateMessage = (message: Message): boolean => {
  return Boolean(message.role && (message.content || message.tool_calls));
};

interface ChatProviderProps {
  children: React.ReactNode;
}

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

export function ChatProvider({ children }: ChatProviderProps) {
  const { messages: history, addMessage } = useSessionHistory();
  const { current: currentSession } = useSessionContext();
  const { value: settingValue } = useSettings();
  const { getCurrent: getCurrentAssistant, availableTools } = useAssistantContext();
  const { availableTools: builtInTools } = useBuiltInTools();

  const [streamingMessage, setStreamingMessage] = useState<Message | null>(
    null,
  );
  const [systemPromptExtensions, setSystemPromptExtensions] = useState<SystemPromptExtension[]>([]);

  // Extract window size with default fallback
  const messageWindowSize = settingValue?.windowSize ?? 20;

  // Register system prompt extension
  const registerSystemPrompt = useCallback((extension: Omit<SystemPromptExtension, 'id'>) => {
    const id = createId();
    const newExtension: SystemPromptExtension = { ...extension, id };
    
    setSystemPromptExtensions((prev) => {
      const updated = [...prev, newExtension];
      // Sort by priority (higher priority first)
      return updated.sort((a, b) => b.priority - a.priority);
    });

    logger.debug('Registered system prompt extension', { id, priority: extension.priority });
    return id;
  }, []);

  // Unregister system prompt extension
  const unregisterSystemPrompt = useCallback((id: string) => {
    setSystemPromptExtensions((prev) => prev.filter((ext) => ext.id !== id));
    logger.debug('Unregistered system prompt extension', { id });
  }, []);

  // Build combined system prompt with extensions
  const buildSystemPrompt = useCallback(async (): Promise<string> => {
    const basePrompt = getCurrentAssistant()?.systemPrompt || DEFAULT_SYSTEM_PROMPT;
    
    if (systemPromptExtensions.length === 0) {
      return basePrompt;
    }

    // Resolve all extensions (handle both string and function types)
    const extensionPrompts = await Promise.all(
      systemPromptExtensions.map(async (ext) => {
        if (typeof ext.content === 'function') {
          return await ext.content();
        }
        return ext.content;
      })
    );

    // Combine base prompt with extensions
    const combinedPrompt = [basePrompt, ...extensionPrompts]
      .filter(Boolean)
      .join('\n\n');

    logger.debug('Built combined system prompt', {
      baseLength: basePrompt.length,
      extensionsCount: systemPromptExtensions.length,
      totalLength: combinedPrompt.length,
    });

    return combinedPrompt;
  }, [getCurrentAssistant, systemPromptExtensions]);

  // AI Service configuration with tools only
  const aiServiceConfig = useMemo((): AIServiceConfig => ({
    tools: [...availableTools, ...builtInTools],
    maxRetries: 3,
    maxTokens: 4096,
  }), [availableTools, builtInTools]);

  const { submit: triggerAIService, isLoading, response } = useAIService(aiServiceConfig);

  // Combine history with streaming message, avoiding duplicates
  const messages = useMemo(() => {
    if (!streamingMessage) {
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
  }, [streamingMessage, history]);

  // Handle AI service streaming responses
  useEffect(() => {
    if (!response) return;

    setStreamingMessage((previous) => {
      if (previous) {
        // Merge response with existing streaming message
        return { ...previous, ...response };
      }

      // Create new streaming message with proper defaults
      return {
        ...response,
        id: response.id ?? createId(),
        content: response.content ?? '',
        role: 'assistant' as const,
        sessionId: response.sessionId ?? currentSession?.id ?? '',
        isStreaming: response.isStreaming !== false,
      };
    });
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
      if (!currentSession) {
        throw new Error('No active session available for message submission');
      }

      try {
        let messagesToSend = messages;

        // Process and validate new messages if provided
        if (messageToAdd?.length) {
          const processedMessages = await Promise.all(
            messageToAdd.map(async (message) => {
              if (!validateMessage(message)) {
                throw new Error(
                  'Invalid message: must have role and either content or tool_calls',
                );
              }

              const messageWithSession = {
                ...message,
                sessionId: currentSession.id,
              };

              await addMessage(messageWithSession);
              return messageWithSession;
            }),
          );

          messagesToSend = [...messages, ...processedMessages];
        }

        // Get windowed messages (excluding system prompts from history)
        const userMessages = messagesToSend
          .filter((msg) => msg.role !== 'system')
          .slice(-messageWindowSize);

        // Combine system prompts with user messages
        const finalMessages = [...userMessages];

        logger.debug('Submitting messages with system prompts', {
          userMessagesCount: userMessages.length,
          extensionsCount: systemPromptExtensions.length,
          agentKey,
        });

        // Send combined messages to AI service with dynamic system prompt
        const aiResponse = await triggerAIService(finalMessages, buildSystemPrompt);

        // Handle AI response persistence
        if (aiResponse) {
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
        }

        return aiResponse;
      } catch (error) {
        logger.error('Message submission failed', { error, agentKey });
        setStreamingMessage(null);
        throw error;
      }
    },
    [
      currentSession,
      messages,
      messageWindowSize,
      triggerAIService,
      addMessage,
      systemPromptExtensions,
    ]
  );

  const value: ChatContextValue = useMemo(
    () => ({
      submit,
      isLoading,
      messages,
      registerSystemPrompt,
      unregisterSystemPrompt,
    }),
    [messages, submit, isLoading, registerSystemPrompt, unregisterSystemPrompt],
  );

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>;
}

export function useChatContext(): ChatContextValue {
  const context = useContext(ChatContext);
  if (context === undefined) {
    throw new Error('useChatContext must be used within a ChatProvider');
  }
  return context;
}
