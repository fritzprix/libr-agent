import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { Settings } from '@/context/SettingsContext';
import { AIServiceProvider } from '@/lib/ai-service/types';
import { normalizeRustMessage } from '@/lib/ai-service/utils';
import { getLogger } from '@/lib/logger';
import type { CompletionRequest } from './types';

const logger = getLogger('useLLMListener');

interface UseLLMListenerProps {
  settingsRef: React.MutableRefObject<Settings>;
  executeCompletionRequest: (
    sessionId: string,
    messages: Message[],
    model: string,
    provider: string,
    apiKey?: string,
    systemPrompt?: string,
    temperature?: number,
    maxTokens?: number,
    availableTools?: MCPTool[],
  ) => Promise<Message>;
  setStreamingMessages: React.Dispatch<
    React.SetStateAction<Map<string, Partial<Message>>>
  >;
}

export function useLLMListener({
  settingsRef,
  executeCompletionRequest,
  setStreamingMessages,
}: UseLLMListenerProps) {
  // Track listener setup to prevent duplicate registration in React Strict Mode
  const listenerSetupRef = useRef(false);

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
              isStreaming: true,
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
      // Reset listener setup ref on unmount
      listenerSetupRef.current = false;
    };
  }, []); // ⚠️ CRITICAL: Empty dependency array to prevent re-registering listener
}
