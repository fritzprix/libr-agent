import {
  mockMessages,
  mockStreamChat,
  setupAgentChatMocks,
  TestWrapper,
} from './agent-chat-test-utils';
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  useAgentChatMessages,
  useAgentChatMessageTail,
  useAgentChatState,
  useAgentChatActions,
  useAgentChat,
  isAssistantStreamingMessageSuperseded,
} from '../AgentChatContext';
import { useLLMService } from '../LLMServiceContext';
import { useAgentSessionState } from '../AgentSessionContext';
import type { Message } from '@/models/chat';

describe('AgentChatContext', () => {
  beforeEach(() => {
    setupAgentChatMocks();
  });

  describe('Provider Setup', () => {
    it('should provide state context', async () => {
      const { result } = renderHook(() => useAgentChatState(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(result.current.isSessionLoading).toBe(false);
      expect(result.current.error).toBeNull();
      expect(result.current.workflowStatus).toBe('idle');
      expect(result.current).not.toHaveProperty('messages');
    });

    it('should provide messages context separately from state', async () => {
      const { result } = renderHook(() => useAgentChatMessages(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(result.current.messages).toEqual(mockMessages);
      });
    });

    it('should expose message tail keyed by last message id', async () => {
      const { result } = renderHook(() => useAgentChatMessageTail(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(result.current.lastMessage?.id).toBe(
          mockMessages[mockMessages.length - 1]?.id,
        );
      });
    });

    it('should provide actions context', () => {
      const { result } = renderHook(() => useAgentChatActions(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(typeof result.current.submit).toBe('function');
      expect(typeof result.current.cancel).toBe('function');
      expect(typeof result.current.cancelPendingPrompt).toBe('function');
      expect(typeof result.current.retryMessage).toBe('function');
      expect(typeof result.current.appendToolMessages).toBe('function');
    });

    it('should provide combined hook', () => {
      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(result.current.isSessionLoading).toBe(false);
      expect(typeof result.current.submit).toBe('function');
    });

    it('should throw error when state hook used outside provider', () => {
      const originalError = console.error;
      console.error = vi.fn();

      expect(() => {
        renderHook(() => useAgentChatState());
      }).toThrow('useAgentChatState must be used within AgentChatProvider');

      console.error = originalError;
    });

    it('should throw error when actions hook used outside provider', () => {
      const originalError = console.error;
      console.error = vi.fn();

      expect(() => {
        renderHook(() => useAgentChatActions());
      }).toThrow('useAgentChatActions must be used within AgentChatProvider');

      console.error = originalError;
    });
  });

  describe('Streaming message merge', () => {
    it('does not mark a streaming assistant as superseded until persisted tool calls catch up', () => {
      const streamingMessage: Message = {
        id: 'stream-1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'assistant',
        content: [{ type: 'text', text: 'Building artifact...' }],
        thinking: 'Need a tool...',
        tool_calls: [
          {
            id: 'call-streaming',
            type: 'function',
            function: {
              name: 'workspace__writeFile',
              arguments: '{"path":"index.html"',
            },
          },
        ],
        createdAt: new Date('2026-04-04T05:00:00.000Z'),
        updatedAt: new Date('2026-04-04T05:00:01.000Z'),
      };

      const persistedWithoutToolCalls: Message = {
        ...streamingMessage,
        id: 'persisted-1',
        tool_calls: [],
        updatedAt: new Date('2026-04-04T05:00:02.000Z'),
      };

      const persistedWithToolCalls: Message = {
        ...streamingMessage,
        id: 'persisted-2',
        tool_calls: [
          {
            id: 'call-streaming',
            type: 'function',
            function: {
              name: 'workspace__writeFile',
              arguments: '{"path":"index.html","content":"ok"}',
            },
          },
        ],
        updatedAt: new Date('2026-04-04T05:00:03.000Z'),
      };

      expect(
        isAssistantStreamingMessageSuperseded(
          streamingMessage,
          persistedWithoutToolCalls,
        ),
      ).toBe(false);
      expect(
        isAssistantStreamingMessageSuperseded(
          streamingMessage,
          persistedWithToolCalls,
        ),
      ).toBe(true);
    });

    it('exposes a streaming assistant message with thinking and tool calls before completion', async () => {
      let releaseStream!: () => void;
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ thinking: 'Need a tool...' });
        yield JSON.stringify({
          tool_calls: [
            {
              index: 0,
              id: 'call-streaming',
              type: 'function',
              function: {
                name: 'workspace__readFile',
                arguments: '{"path":"foo.txt"}',
              },
            },
          ],
        });

        await new Promise<void>((resolve) => {
          releaseStream = resolve;
        });
      });

      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        session: { id: 'test-session', name: 'Test Session' },
        messages: mockMessages,
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'busy',
      });

      const { result } = renderHook(
        () => ({
          llm: useLLMService(),
          agent: useAgentChat(),
        }),
        { wrapper: TestWrapper },
      );

      let promise!: Promise<Message>;
      await act(async () => {
        promise = result.current.llm.executeCompletionRequest(
          'test-session',
          'response-msg-agent-1',
          mockMessages,
          'test-model',
          'openai',
          'test-key',
        );
      });

      await waitFor(() => {
        const streamingAssistant = result.current.agent.messages.find(
          (message) =>
            message.role === 'assistant' &&
            message.isStreaming &&
            message.tool_calls?.length,
        );

        expect(streamingAssistant).toMatchObject({
          role: 'assistant',
          isStreaming: true,
          thinking: 'Need a tool...',
          tool_calls: [
            {
              id: 'call-streaming',
              type: 'function',
              function: {
                name: 'workspace__readFile',
                arguments: '{"path":"foo.txt"}',
              },
            },
          ],
        });
      });

      await act(async () => {
        releaseStream();
        await promise.catch(() => undefined);
      });
    });
  });
});
