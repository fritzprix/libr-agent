import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useAsyncFn } from 'react-use';
import { useSessionContext } from './SessionContext';
import { useSessionHistory } from './SessionHistoryContext';
import { useAIService } from '../hooks/use-ai-service';
import { useAssistantContext } from './AssistantContext';
import { useBuiltInTools } from './BuiltInToolContext';
import { useUnifiedMCP } from '../hooks/use-unified-mcp';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message, UIResource } from '@/models/chat';
import { useSettings } from '../hooks/use-settings';
import { AIServiceConfig } from '@/lib/ai-service';
import {
  isMCPError,
  MCPResponse,
  mcpResponseToString,
  MCPResourceContent,
} from '@/lib/mcp-types';

const logger = getLogger('ChatContext');

interface SystemPromptExtension {
  id: string;
  content: string | (() => Promise<string>);
  priority: number; // Higher number = higher priority
}

interface SerializedToolResult {
  success: boolean;
  text?: string;
  uiResource?: UIResource | UIResource[];
  error?: string;
  metadata: Record<string, unknown>;
  toolName: string;
  executionTime: number;
  timestamp: string;
}

// Pure function - serializeToolResult
const serializeToolResult = (
  mcpResponse: MCPResponse,
  toolName: string,
  executionStartTime: number,
): SerializedToolResult => {
  const executionTime = Date.now() - executionStartTime;
  const timestamp = new Date().toISOString();

  // 표준 MCP 에러 처리
  if (isMCPError(mcpResponse)) {
    return {
      success: false,
      error: `${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`,
      metadata: {
        toolName,
        mcpResponseId: mcpResponse.id,
        jsonrpc: mcpResponse.jsonrpc,
        errorCode: mcpResponse.error.code,
        errorData: mcpResponse.error.data,
      },
      toolName,
      executionTime,
      timestamp,
    };
  }

  // Tool 실행 에러 처리 (isError: true)
  if (mcpResponse.result?.isError) {
    const errorText =
      mcpResponse.result.content?.[0]?.type === 'text'
        ? mcpResponse.result.content[0].text
        : 'Tool execution failed';

    return {
      success: false,
      error: errorText,
      metadata: {
        toolName,
        mcpResponseId: mcpResponse.id,
        jsonrpc: mcpResponse.jsonrpc,
        isToolExecutionError: true,
      },
      toolName,
      executionTime,
      timestamp,
    };
  }

  // 성공 케이스 - UIResource 감지 및 보존
  let uiResources: UIResource[] = [];
  let textContent = '';

  if (mcpResponse.result?.content) {
    // UIResource 항목들을 찾아서 수집
    const resourceItems = mcpResponse.result.content.filter(
      (item): item is MCPResourceContent => item.type === 'resource',
    );

    if (resourceItems.length > 0) {
      uiResources = resourceItems.map((item) => item.resource);
      logger.info(`Found ${uiResources.length} UI resources in tool response`, {
        toolName,
        resourceTypes: uiResources.map((r) => r.mimeType),
      });
    }

    // 텍스트 내용 수집 - UI Resource가 있으면 요약, 없으면 전체 텍스트
    if (uiResources.length > 0) {
      // UIResource가 있을 때는 간단한 요약만 제공 (HTML 등 대용량 컨텐츠 제외)
      // undefined 방지를 위한 안전한 요약 생성
      const resourceSummary = uiResources
        .map((r) => {
          const mimeType = r.mimeType || 'unknown';
          const uri = r.uri || 'no-uri';
          return `${mimeType} (${uri})`;
        })
        .join(', ');
      textContent = `UI Resources: ${resourceSummary}`;
    } else {
      // UIResource가 없을 때는 기존 동작 유지
      textContent = mcpResponseToString(mcpResponse);
    }
  }

  return {
    success: true,
    text: textContent,
    uiResource:
      uiResources.length === 1
        ? uiResources[0]
        : uiResources.length > 1
          ? uiResources
          : undefined,
    metadata: {
      toolName,
      mcpResponseId: mcpResponse.id,
      jsonrpc: mcpResponse.jsonrpc,
      hasUIResource: uiResources.length > 0,
    },
    toolName,
    executionTime,
    timestamp,
  };
};

interface ChatContextValue {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
  isLoading: boolean;
  messages: Message[];
  registerSystemPrompt: (
    extension: Omit<SystemPromptExtension, 'id'>,
  ) => string;
  unregisterSystemPrompt: (id: string) => void;
  cancel: () => void;
}

const ChatContext = createContext<ChatContextValue | undefined>(undefined);

const validateMessage = (message: Message): boolean => {
  return Boolean(message.role && (message.content || message.tool_calls));
};

interface ChatProviderProps {
  children: React.ReactNode;
}

const DEFAULT_SYSTEM_PROMPT = 'You are a helpful assistant.';

// ToolCaller 컴포넌트 - callback prop으로 tool 실행 상태 관리
interface ToolCallerProps {
  onToolExecutionChange: (isExecuting: boolean) => void;
}

const ToolCaller: React.FC<ToolCallerProps> = ({ onToolExecutionChange }) => {
  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { messages, submit } = useChatContext();
  const { executeToolCall } = useUnifiedMCP();

  const lastProcessedMessageId = useRef<string | null>(null);

  const [{ loading }, execute] = useAsyncFn(
    async (tcMessage: Message) => {
      if (!tcMessage.tool_calls || tcMessage.tool_calls.length === 0) {
        logger.warn('No tool calls found in message');
        return;
      }

      // Tool 실행 시작을 부모에게 알림
      onToolExecutionChange(true);

      try {
        logger.info('Starting tool execution batch', {
          toolCallCount: tcMessage.tool_calls.length,
          messageId: tcMessage.id,
        });

        // Tool Call ID 검증 강화
        const validateToolCallId = (toolCall: { id: string }): boolean => {
          // Tool call ID가 유효한지 검증
          return Boolean(toolCall.id && toolCall.id.trim().length > 0);
        };

        // Tool 실행 전 검증 로직 추가
        for (const toolCall of tcMessage.tool_calls) {
          if (!validateToolCallId(toolCall)) {
            logger.error('Invalid tool call ID detected', { toolCall });
            // 유효하지 않은 ID의 경우 새로운 deterministic ID 생성
            toolCall.id = `fallback_${Math.abs(JSON.stringify(toolCall.function).split('').reduce((a, b) => {
              a = ((a << 5) - a) + b.charCodeAt(0);
              return a & a;
            }, 0)).toString(36)}`;
          }
        }

        const toolResults: Message[] = [];

        for (const toolCall of tcMessage.tool_calls) {
          const toolName = toolCall.function.name;
          const executionStartTime = Date.now();

          try {
            logger.debug('Executing tool', {
              toolName,
              toolCallId: toolCall.id,
            });

            const mcpResponse = await executeToolCall(toolCall);
            const serialized = serializeToolResult(
              mcpResponse,
              toolName,
              executionStartTime,
            );

            const toolResultMessage: Message = {
              id: createId(),
              assistantId: currentAssistant?.id,
              role: 'tool',
              content:
                serialized.text ||
                (serialized.uiResource
                  ? `[UIResource: ${
                      Array.isArray(serialized.uiResource)
                        ? serialized.uiResource.length + ' resources'
                        : serialized.uiResource.uri || 'resource'
                    }]`
                  : ''),
              uiResource: serialized.uiResource,
              tool_call_id: toolCall.id,
              sessionId: currentSession?.id || '',
            };

            if (!serialized.success && serialized.error) {
              toolResultMessage.content = `Error: ${serialized.error}`;
            }

            toolResults.push(toolResultMessage);

            logger.info('Tool execution completed', {
              toolName,
              success: serialized.success,
              executionTime: serialized.executionTime,
            });
          } catch (error) {
            logger.error('Tool execution failed', { toolName, error });

            const errorMessage: Message = {
              id: createId(),
              assistantId: currentAssistant?.id,
              role: 'tool',
              content: `Error executing ${toolName}: ${error instanceof Error ? error.message : 'Unknown error'}`,
              sessionId: currentSession?.id || '',
              tool_call_id: toolCall.id,
            };

            toolResults.push(errorMessage);
          }
        }

        if (toolResults.length > 0) {
          logger.info('Submitting tool results', {
            resultCount: toolResults.length,
            messageId: tcMessage.id,
          });

          await submit(toolResults, currentAssistant?.id);
        }
      } finally {
        // Tool 실행 완료를 부모에게 알림
        onToolExecutionChange(false);
      }
    },
    [
      submit,
      executeToolCall,
      currentAssistant,
      currentSession,
      onToolExecutionChange,
    ],
  );

  useEffect(() => {
    const lastMessage = messages[messages.length - 1];
    if (
      lastMessage &&
      lastMessage.role === 'assistant' &&
      lastMessage.tool_calls &&
      lastMessage.tool_calls.length > 0 &&
      !lastMessage.isStreaming &&
      !loading &&
      lastMessage.id &&
      lastProcessedMessageId.current !== lastMessage.id
    ) {
      lastProcessedMessageId.current = lastMessage.id;
      execute(lastMessage);
    }
  }, [messages, execute, loading]);

  return null;
};

export function ChatProvider({ children }: ChatProviderProps) {
  const { messages: history, addMessage } = useSessionHistory();
  const { current: currentSession } = useSessionContext();
  const { value: settingValue } = useSettings();
  const { getCurrent: getCurrentAssistant, availableTools } =
    useAssistantContext();
  const { availableTools: builtInTools } = useBuiltInTools();
  const cancelRequestRef = useRef(false);

  const [streamingMessage, setStreamingMessage] = useState<Message | null>(
    null,
  );
  const [systemPromptExtensions, setSystemPromptExtensions] = useState<
    SystemPromptExtension[]
  >([]);
  const [isToolExecuting, setIsToolExecuting] = useState(false);

  // Extract window size with default fallback
  const messageWindowSize = settingValue?.windowSize ?? 20;

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      setStreamingMessage(null);
      setSystemPromptExtensions([]);
      setIsToolExecuting(false);
    };
  }, []);

  // Register system prompt extension
  const registerSystemPrompt = useCallback(
    (extension: Omit<SystemPromptExtension, 'id'>) => {
      const id = createId();
      const newExtension: SystemPromptExtension = { ...extension, id };

      setSystemPromptExtensions((prev) => {
        const updated = [...prev, newExtension];
        // Sort by priority (higher priority first)
        return updated.sort((a, b) => b.priority - a.priority);
      });

      logger.debug('Registered system prompt extension', {
        id,
        priority: extension.priority,
      });
      return id;
    },
    [],
  );

  // Unregister system prompt extension
  const unregisterSystemPrompt = useCallback((id: string) => {
    setSystemPromptExtensions((prev) => prev.filter((ext) => ext.id !== id));
    logger.debug('Unregistered system prompt extension', { id });
  }, []);

  // Build combined system prompt with extensions
  const buildSystemPrompt = useCallback(async (): Promise<string> => {
    const basePrompt =
      getCurrentAssistant()?.systemPrompt || DEFAULT_SYSTEM_PROMPT;

    if (systemPromptExtensions.length === 0) {
      return basePrompt;
    }

    try {
      // Resolve all extensions with error handling for individual extensions
      const extensionPrompts = await Promise.allSettled(
        systemPromptExtensions.map(async (ext) => {
          try {
            if (typeof ext.content === 'function') {
              return await ext.content();
            }
            return ext.content;
          } catch (error) {
            logger.warn('Failed to resolve system prompt extension', {
              extensionId: ext.id,
              error,
            });
            return ''; // Return empty string for failed extensions
          }
        }),
      );

      // Extract successful results
      const resolvedPrompts = extensionPrompts
        .map((result) => (result.status === 'fulfilled' ? result.value : ''))
        .filter(Boolean);

      // Combine base prompt with successful extensions
      const combinedPrompt = [basePrompt, ...resolvedPrompts]
        .filter(Boolean)
        .join('\n\n');

      logger.debug('Built combined system prompt', {
        baseLength: basePrompt.length,
        extensionsCount: systemPromptExtensions.length,
        successfulExtensions: resolvedPrompts.length,
        totalLength: combinedPrompt.length,
      });

      return combinedPrompt;
    } catch (error) {
      logger.error('Error building system prompt, using base prompt', {
        error,
      });
      return basePrompt;
    }
  }, [getCurrentAssistant, systemPromptExtensions]);

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
          extensionsCount: systemPromptExtensions.length,
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
      systemPromptExtensions,
      buildSystemPrompt,
    ],
  );

  const handleCancel = useCallback(() => {
    cancelRequestRef.current = true;
  }, []);

  const value: ChatContextValue = useMemo(
    () => ({
      submit,
      isLoading,
      messages,
      registerSystemPrompt,
      unregisterSystemPrompt,
      cancel: handleCancel,
    }),
    [
      messages,
      submit,
      isLoading,
      registerSystemPrompt,
      unregisterSystemPrompt,
      handleCancel,
    ],
  );

  return (
    <ChatContext.Provider value={value}>
      {children}
      {/* ToolCaller를 자동으로 포함 - 사용자가 수동으로 추가할 필요 없음 */}
      <ToolCaller onToolExecutionChange={setIsToolExecuting} />
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
