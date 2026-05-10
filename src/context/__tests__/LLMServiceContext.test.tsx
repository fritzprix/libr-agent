import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  LLMServiceProvider,
  useLLMService,
  useStreamingMessages,
} from '../LLMServiceContext';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import * as agentCommands from '@/lib/backend/agent-commands';
import type { Message } from '@/models/chat';
import { SettingsProvider } from '../SettingsContext';
import React, { type ReactNode } from 'react';
import { __resetLLMListenerStartupLogStateForTests } from '../llm/useLLMListener';

const { loggerInfo, loggerDebug, loggerWarn, loggerError } = vi.hoisted(() => ({
  loggerInfo: vi.fn(),
  loggerDebug: vi.fn(),
  loggerWarn: vi.fn(),
  loggerError: vi.fn(),
}));

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock agent commands
vi.mock('@/lib/backend/agent-commands', () => ({
  handleLLMError: vi.fn(),
  handleLLMResponse: vi.fn(),
  reportLLMStreamingIssue: vi.fn(),
  getAgentCompactContext: vi.fn(),
}));

// Mock AIServiceFactory
vi.mock('@/lib/ai-service/factory', () => ({
  AIServiceFactory: {
    getService: vi.fn(),
  },
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: loggerInfo,
    debug: loggerDebug,
    warn: loggerWarn,
    error: loggerError,
  }),
}));

// Mock retry-utils
vi.mock('@/lib/retry-utils', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/retry-utils')>();
  return { ...actual, sleep: vi.fn().mockResolvedValue(undefined) };
});

// Test wrapper with required providers
function TestWrapper({ children }: { children: ReactNode }) {
  return (
    <SettingsProvider>
      <LLMServiceProvider>{children}</LLMServiceProvider>
    </SettingsProvider>
  );
}

function useLLMServiceHarness() {
  return {
    ...useLLMService(),
    streamingMessages: useStreamingMessages(),
  };
}

describe('LLMServiceContext – Core', () => {
  const mockUnlisten = vi.fn();
  const mockStreamChat = vi.fn();
  const mockListModels = vi.fn();
  const mockCancel = vi.fn();
  const mockDispose = vi.fn();
  const mockSetDefaultConfig = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    __resetLLMListenerStartupLogStateForTests();

    // Setup listen mock
    (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);

    // Setup AIServiceFactory mock
    (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
      streamChat: mockStreamChat,
      listModels: mockListModels,
      cancel: mockCancel,
      dispose: mockDispose,
      setDefaultConfig: mockSetDefaultConfig,
      sanitizeMessages: vi.fn((messages: Message[]) => messages),
      // Default implementation: pass-through (mirrors BaseAIService default)
      prepareContextInjection: vi.fn((systemPrompt, _sessionContext, messages) => ({
        systemPrompt,
        messages,
      })),
    });

    // Setup mockListModels to return test models
    mockListModels.mockResolvedValue([
      {
        name: 'test-model',
        contextWindow: 4096,
        supportReasoning: false,
        supportTools: true,
        supportStreaming: true,
        cost: { input: 0.001, output: 0.002 },
        description: 'Test model for unit tests',
      },
    ]);
  });

    afterEach(() => {
      vi.clearAllMocks();
    });

  describe('Provider Setup', () => {
    it('should provide context value', () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(result.current.streamingMessages).toBeInstanceOf(Map);
      expect(typeof result.current.getSessionStatus).toBe('function');
      expect(typeof result.current.executeCompletionRequest).toBe('function');
    });

    it('hydrates persisted compacted range into frontend state', async () => {
      (agentCommands.getAgentCompactContext as ReturnType<typeof vi.fn>).mockResolvedValue({
        id: 'compact-1',
        sessionId: 'session-1',
        fromId: 'msg-1',
        toId: 'msg-9',
        summary: 'Persisted summary',
        createdAt: Date.now(),
      });

      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.refreshCompactedRange('session-1');
      });

      expect(result.current.getCompactedRange('session-1')).toEqual({
        fromId: 'msg-1',
        toId: 'msg-9',
        summary: 'Persisted summary',
      });
    });

    it('should throw error when used outside provider', () => {
      // Suppress console.error for this test
      const originalError = console.error;
      console.error = vi.fn();

      expect(() => {
        renderHook(() => useLLMService());
      }).toThrow('useLLMService must be used within LLMServiceProvider');

      console.error = originalError;
    });

    it('should register event listener on mount', async () => {
      renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(listen).toHaveBeenCalledWith(
          'llm:completion-request',
          expect.any(Function),
        );
      });
    });

    it('dedupes startup listener lifecycle logs across StrictMode remounts', async () => {
      const wrapper = ({ children }: { children: ReactNode }) => (
        <React.StrictMode>
          <TestWrapper>{children}</TestWrapper>
        </React.StrictMode>
      );

      renderHook(() => useLLMService(), {
        wrapper,
      });

      await waitFor(() => {
        expect(loggerInfo).toHaveBeenCalledWith(
          'LLM completion request listener registered',
        );
      });

      expect(
        loggerInfo.mock.calls.filter(
          ([message]) =>
            message === '🎧 Initializing LLM completion request listener',
        ),
      ).toHaveLength(1);
      expect(
        loggerInfo.mock.calls.filter(
          ([message]) =>
            message === 'Setting up LLM completion request listener',
        ),
      ).toHaveLength(1);
      expect(
        loggerInfo.mock.calls.filter(
          ([message]) =>
            message === 'LLM completion request listener registered',
        ),
      ).toHaveLength(1);
    });

    it('should cleanup on unmount', async () => {
      const { unmount } = renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(listen).toHaveBeenCalled();
      });

      unmount();

      await waitFor(() => {
        expect(mockUnlisten).toHaveBeenCalled();
      });

      expect(mockDispose).not.toHaveBeenCalled();
    });
  });

  describe('Session Status', () => {
    it('should return idle status by default', () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      expect(result.current.getSessionStatus('test-session')).toBe('idle');
    });

    it('should track streaming status', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      // Mock streaming response with delay to make status observable
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ content: 'Hello' });
        await new Promise((resolve) => setTimeout(resolve, 50));
        yield JSON.stringify({ content: ' world' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      // Execute completion request in act
      let promise: Promise<Message>;
      await act(async () => {
        promise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-1',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );

        // Wait a bit for streaming state to be set
        await new Promise((resolve) => setTimeout(resolve, 10));
      });

      // Should be streaming during execution
      await waitFor(() => {
        expect(result.current.getSessionStatus('test-session')).toBe(
          'streaming',
        );
      });

      await act(async () => {
        await promise;
      });

      // Should be idle after completion
      await waitFor(() => {
        expect(result.current.getSessionStatus('test-session')).toBe('idle');
      });
    });
  });

  describe('Execute Completion Request', () => {
    it('should execute completion and return message', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      // Mock streaming response
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ content: 'Hello' });
        yield JSON.stringify({ content: ' world' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let resultMessage;
      await act(async () => {
        resultMessage = await result.current.executeCompletionRequest(
          'test-session',
          'response-msg-2',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      expect(resultMessage).toMatchObject({
        sessionId: 'test-session',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hello world' }],
      });

      expect(AIServiceFactory.getService).toHaveBeenCalledWith(
        'openai',
        'test-key',
        expect.any(Object), // Settings config object
      );
      expect(mockSetDefaultConfig).not.toHaveBeenCalled();
    });

    it('should handle tool calls in response', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      // Mock streaming response with tool calls
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({
          content: 'Let me help',
          tool_calls: [
            {
              id: 'call1',
              type: 'function',
              function: { name: 'test_tool', arguments: '{"arg": "value"}' },
            },
          ],
        });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let resultMessage!: Message;
      await act(async () => {
        resultMessage = (await result.current.executeCompletionRequest(
          'test-session',
          'response-msg-3',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        )) as Message;
      });

      expect(resultMessage.tool_calls).toHaveLength(1);
      expect(resultMessage.tool_calls?.[0]).toMatchObject({
        id: 'call1',
        type: 'function',
        function: { name: 'test_tool', arguments: '{"arg": "value"}' },
      });
    });

    it('surfaces streaming tool calls before the stream completes', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      let releaseStream!: () => void;
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({
          tool_calls: [
            {
              index: 0,
              id: 'call-streaming',
              type: 'function',
              function: {
                name: 'test_tool',
                arguments: '{"arg":"partial"',
              },
            },
          ],
        });

        await new Promise<void>((resolve) => {
          releaseStream = resolve;
        });

        yield JSON.stringify({
          tool_calls: [
            {
              index: 0,
              function: {
                arguments: ',"rest":"done"}',
              },
            },
          ],
        });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let promise!: Promise<Message>;
      await act(async () => {
        promise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-4',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      await waitFor(() => {
        expect(
          result.current.streamingMessages.get('test-session'),
        ).toMatchObject({
          role: 'assistant',
          isStreaming: true,
          tool_calls: [
            {
              id: 'call-streaming',
              type: 'function',
              function: {
                name: 'test_tool',
                arguments: '{"arg":"partial"',
              },
            },
          ],
        });
      });

      await act(async () => {
        releaseStream();
        await promise;
      });
    });

    it('surfaces tool_call_starts before argument deltas complete', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      let releaseStream!: () => void;
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({
          tool_call_starts: [
            {
              index: 0,
              id: 'call-start-only',
              type: 'function',
              function: {
                name: 'test_tool',
                arguments: '',
              },
            },
          ],
        });

        await new Promise<void>((resolve) => {
          releaseStream = resolve;
        });

        yield JSON.stringify({
          tool_calls: [
            {
              index: 0,
              function: {
                arguments: '{"arg":"done"}',
              },
            },
          ],
        });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let promise!: Promise<Message>;
      await act(async () => {
        promise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-5',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      await waitFor(() => {
        expect(
          result.current.streamingMessages.get('test-session'),
        ).toMatchObject({
          role: 'assistant',
          isStreaming: true,
          tool_calls: [
            {
              id: 'call-start-only',
              type: 'function',
              function: {
                name: 'test_tool',
                arguments: '',
              },
            },
          ],
        });
      });

      await act(async () => {
        releaseStream();
        await promise;
      });
    });

    it('creates a renderable streaming assistant placeholder before chunks complete', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      let releaseStream!: () => void;
      mockStreamChat.mockImplementation(async function* () {
        await new Promise<void>((resolve) => {
          releaseStream = resolve;
        });
        yield JSON.stringify({ content: 'done' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let promise!: Promise<Message>;
      await act(async () => {
        promise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-6',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      const streamingMessage = result.current.streamingMessages.get('test-session');
      expect(streamingMessage).toMatchObject({
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'assistant',
        isStreaming: true,
        content: [],
      });

      await act(async () => {
        releaseStream();
        await promise;
      });
    });

    it('should handle thinking content', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      // Mock streaming response with thinking
      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ thinking: 'Let me think...' });
        yield JSON.stringify({ content: 'Answer' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let resultMessage!: Message;
      await act(async () => {
        resultMessage = (await result.current.executeCompletionRequest(
          'test-session',
          'response-msg-7',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        )) as Message;
      });

      expect(resultMessage.thinking).toBe('Let me think...');
      expect(resultMessage.content).toEqual([
        { type: 'thinking', thinking: 'Let me think...' },
        { type: 'text', text: 'Answer' },
      ]);
    });

    it('should handle errors and update status', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      // Mock streaming error
      mockStreamChat.mockImplementation(async function* () {
        yield; // Satisfy require-yield
        throw new Error('API Error');
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      await expect(
        result.current.executeCompletionRequest(
          'test-session',
          'response-msg-8',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        ),
      ).rejects.toThrow('API Error');

      await waitFor(() => {
        expect(result.current.getSessionStatus('test-session')).toBe('error');
      });
    });

    it('should cleanup resources after completion', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ content: 'Done' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      await result.current.executeCompletionRequest(
        'test-session',
        'response-msg-9',
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

      // Streaming message should be cleared
      expect(result.current.streamingMessages.has('test-session')).toBe(false);
    });

    it('clears stale streaming UI immediately on cancellation', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementation(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        yield JSON.stringify({ thinking: 'looping...' });

        while (!options?.signal?.aborted) {
          await new Promise((resolve) => window.setTimeout(resolve, 0));
        }
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let requestPromise!: Promise<Message>;
      let requestSettled!: Promise<unknown>;
      await act(async () => {
        requestPromise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-cancel',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
        requestSettled = requestPromise.catch((error: unknown) => error);
      });

      await waitFor(() => {
        expect(result.current.streamingMessages.has('test-session')).toBe(true);
      });

      act(() => {
        result.current.cancelCompletionRequest(
          'test-session',
          'response-msg-cancel',
        );
      });

      await waitFor(() => {
        expect(result.current.streamingMessages.has('test-session')).toBe(false);
      });

      await expect(requestPromise).rejects.toThrow('Request aborted');
      await act(async () => {
        await requestSettled;
      });

      expect(mockDispose).not.toHaveBeenCalled();
    });

    it('treats a silent pre-first-chunk abort as cancellation instead of empty response', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementation(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        await new Promise((resolve) => window.setTimeout(resolve, 0));
        if (options?.signal?.aborted) {
          return;
        }
        yield JSON.stringify({ content: 'late reply' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let requestPromise!: Promise<Message>;
      await act(async () => {
        requestPromise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-silent-cancel',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      act(() => {
        result.current.cancelCompletionRequest(
          'test-session',
          'response-msg-silent-cancel',
        );
      });

      await expect(requestPromise).rejects.toThrow('Request aborted');
      expect(mockDispose).not.toHaveBeenCalled();
    });

    it('treats a silent pre-first-chunk supersede as superseded instead of empty response', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementation(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        await new Promise((resolve) => window.setTimeout(resolve, 0));
        if (options?.signal?.aborted) {
          return;
        }
        yield JSON.stringify({ content: 'fresh reply' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      const firstPromise = result.current.executeCompletionRequest(
        'test-session',
        'response-msg-old-silent',
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

      const secondPromise = result.current.executeCompletionRequest(
        'test-session',
        'response-msg-new-silent',
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

      await expect(firstPromise).rejects.toThrow('Request superseded');
      await expect(secondPromise).resolves.toMatchObject({
        content: [{ type: 'text', text: 'fresh reply' }],
      });
      expect(mockDispose).not.toHaveBeenCalled();
    });

    it('does not dispose a shared cached service when a new request supersedes the old one', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      let streamCallCount = 0;
      mockStreamChat.mockImplementation(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        streamCallCount += 1;
        if (streamCallCount === 1) {
          while (!options?.signal?.aborted) {
            await new Promise((resolve) => window.setTimeout(resolve, 0));
          }
          return;
        }

        yield JSON.stringify({ content: 'fresh reply' });
      });

      const messages: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
      ];

      let firstPromise!: Promise<Message>;
      let secondPromise!: Promise<Message>;
      let firstSettled!: Promise<unknown>;
      let secondSettled!: Promise<unknown>;
      await act(async () => {
        firstPromise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-old',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
        firstSettled = firstPromise.catch((error: unknown) => error);
      });

      await act(async () => {
        secondPromise = result.current.executeCompletionRequest(
          'test-session',
          'response-msg-new',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
        secondSettled = secondPromise.catch((error: unknown) => error);
      });

      await act(async () => {
        await Promise.all([firstSettled, secondSettled]);
      });

      expect(mockDispose).not.toHaveBeenCalled();
    });
  });
});
