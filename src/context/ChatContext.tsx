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

const logger = getLogger('ChatContext');

interface ChatContextValue {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
  isLoading: boolean;
  messages: Message[];
  cancel: () => void;
}

const ChatContext = createContext<ChatContextValue | undefined>(undefined);

interface ChatProviderProps {
  children: React.ReactNode;
}

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

export function ChatProvider({ children }: ChatProviderProps) {
  const { messages: history, addMessage, addMessages } = useSessionHistory();
  const { current: currentSession } = useSessionContext();
  const { value: settingValue } = useSettings();
  const { getCurrent: getCurrentAssistant, availableTools } =
    useAssistantContext();
  const { getSystemPrompt } = useSystemPrompt();
  const { availableTools: builtInTools } = useBuiltInTool();
  const cancelRequestRef = useRef(false);

  const [streamingMessage, setStreamingMessage] = useState<Message | null>(
    null,
  );
  const [isToolExecuting, setIsToolExecuting] = useState(false);

  // Extract window size with default fallback
  const messageWindowSize = settingValue?.windowSize ?? 20;

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      setStreamingMessage(null);
      setIsToolExecuting(false);
    };
  }, []);

  // 세션 변경 시 streamingMessage 초기화 루틴 (타이밍 문제 해결)
  useEffect(() => {
    if (currentSession?.id) {
      logger.debug('Session changed, ensuring streamingMessage is cleared', {
        newSessionId: currentSession.id,
      });
      setStreamingMessage(null); // 세션 변경 시 무조건 초기화
      setIsToolExecuting(false); // 툴 실행 상태도 초기화
    }
  }, [currentSession?.id]); // currentSession?.id 변경 시 실행

  const buildSystemPrompt = useCallback(async (): Promise<string> => {
    const basePrompt =
      getCurrentAssistant()?.systemPrompt || DEFAULT_SYSTEM_PROMPT;
    const extensionPrompt = await getSystemPrompt();
    const combined = [basePrompt, extensionPrompt].filter(Boolean).join('\n\n');
    logger.info('Built combined system prompt', {
      baseLength: basePrompt.length,
      extensionLength: extensionPrompt.length,
      totalLength: combined.length,
    });
    return combined;
  }, [getCurrentAssistant, getSystemPrompt]);

  // AI Service configuration with tools only
  const aiServiceConfig = useMemo(
    (): AIServiceConfig => ({
      tools: [...availableTools, ...builtInTools],
      maxRetries: 3,
      maxTokens: 4096,
    }),
    [availableTools, builtInTools],
  );

  const {
    submit: triggerAIService,
    isLoading: aiServiceLoading,
    response,
  } = useAIService(aiServiceConfig);

  // Combined loading state: AI service loading OR tool execution
  const isLoading = aiServiceLoading || isToolExecuting;

  // Combine history with streaming message, avoiding duplicates
  const messages = useMemo(() => {
    if (!streamingMessage) {
      return history;
    }

    // 세션 불일치 시 streamingMessage 무시 (Race Condition 방지)
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

  // Handle AI service streaming responses
  useEffect(() => {
    if (!response) return;

    // 세션 불일치 시 response 무시 (Session 검증 강화)
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

      // Create new streaming message with proper defaults
      return {
        ...response,
        id: response.id ?? createId(),
        content: response.content ?? '',
        role: 'assistant' as const,
        sessionId: currentSession?.id ?? '',
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
      logger.info('submit ', { messageToAdd });
      if (!currentSession) {
        throw new Error('No active session available for message submission');
      }

      try {
        // Clear any previous streaming state before starting new request
        setStreamingMessage(null);

        let messagesToSend = messages;

        // Process and validate new messages if provided (tool 결과 유실 방지)
        if (messageToAdd?.length) {
          const messagesWithSession = messageToAdd.map((m) => ({
            ...m,
            sessionId: currentSession.id,
          }));
          if (typeof addMessages === 'function') {
            await addMessages(messagesWithSession);
            messagesToSend = [...messages, ...messagesWithSession];
          } else {
            // 안전한 폴백: 순차 저장
            const persisted: Message[] = [];
            for (const msg of messagesWithSession) {
              const added = await addMessage(msg);
              persisted.push(added);
            }
            messagesToSend = [...messages, ...persisted];
          }
        }

        // Cancel 체크를 메시지 추가 후로 이동 (tool 결과는 보존)
        if (cancelRequestRef.current) {
          cancelRequestRef.current = false;
          logger.info('Request cancelled after message persistence');
          return {
            id: createId(),
            content: 'Request cancelled',
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
          return finalizedMessage;
        }

        // Handle case where no response was received
        throw new Error('No response received from AI service');
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
      addMessages,
      buildSystemPrompt,
    ],
  );

  const handleCancel = useCallback(() => {
    cancelRequestRef.current = true;
  }, []);

  // Initialize tool processor hook after submit function is defined
  const { processToolCalls } = useToolProcessor({
    onToolExecutionChange: setIsToolExecuting,
    submit,
  });

  // Process tool calls when messages change
  useEffect(() => {
    const lastMessage = messages[messages.length - 1];
    if (lastMessage) {
      processToolCalls(lastMessage);
    }
  }, [messages, processToolCalls]);

  const value: ChatContextValue = useMemo(
    () => ({
      submit,
      isLoading,
      messages,
      cancel: handleCancel,
    }),
    [messages, submit, isLoading, handleCancel],
  );

  return (
    <ChatContext.Provider value={value}>
      {children}
    </ChatContext.Provider>
  );
}

export function useChatContext(): ChatContextValue {
  const context = useContext(ChatContext);
  if (context === undefined) {
    throw new Error('useChatContext must be used within a ChatProvider');
  }
  return context;
}
