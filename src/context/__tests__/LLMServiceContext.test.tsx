import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LLMServiceProvider, useLLMService } from '../LLMServiceContext';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import * as agentCommands from '@/lib/backend/agent-commands';
import type { Message } from '@/models/chat';
import { SettingsProvider, SettingsContext, DEFAULT_SETTING } from '../SettingsContext';
import type { ReactNode } from 'react';
import type { AIServiceProvider } from '@/lib/ai-service';

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
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

// SP4: make sleep a no-op so retry tests don't take real time
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

function WindowStrategyWrapper({ children }: { children: ReactNode }) {
  return (
    <SettingsContext.Provider
      value={{
        value: { ...DEFAULT_SETTING, contextStrategy: 'window' },
        update: vi.fn().mockResolvedValue(undefined),
        isLoading: false,
        error: null,
      }}
    >
      <LLMServiceProvider>{children}</LLMServiceProvider>
    </SettingsContext.Provider>
  );
}

describe('LLMServiceContext', () => {
  const mockUnlisten = vi.fn();
  const mockStreamChat = vi.fn();
  const mockListModels = vi.fn();
  const mockDispose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    // Setup listen mock
    (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);

    // Setup AIServiceFactory mock
    (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
      streamChat: mockStreamChat,
      listModels: mockListModels,
      dispose: mockDispose,
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
      const { result } = renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(result.current.streamingMessages).toBeInstanceOf(Map);
      expect(typeof result.current.getSessionStatus).toBe('function');
      expect(typeof result.current.executeCompletionRequest).toBe('function');
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
    });
  });

  describe('Session Status', () => {
    it('should return idle status by default', () => {
      const { result } = renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      expect(result.current.getSessionStatus('test-session')).toBe('idle');
    });

    it('should track streaming status', async () => {
      const { result } = renderHook(() => useLLMService(), {
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
      const { result } = renderHook(() => useLLMService(), {
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
    });

    it('should handle tool calls in response', async () => {
      const { result } = renderHook(() => useLLMService(), {
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

    it('should handle thinking content', async () => {
      const { result } = renderHook(() => useLLMService(), {
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
      const { result } = renderHook(() => useLLMService(), {
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
      const { result } = renderHook(() => useLLMService(), {
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
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

      // Streaming message should be cleared
      expect(result.current.streamingMessages.has('test-session')).toBe(false);
    });
  });

  describe('Event Handling', () => {
    it('should handle llm:completion-request event', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ content: 'Response' });
      });

      renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      // Trigger event
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          messages: [
            {
              id: 'msg1',
              sessionId: 'test-session',
              threadId: 'test-session',
              role: 'user',
              content: [{ type: 'text', text: 'Hello' }],
              createdAt: new Date(),
            },
          ],
          model: 'gpt-4',
          provider: 'openai',
          apiKey: 'test-key',
        },
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMResponse).toHaveBeenCalled();
      });
    });

    it('should report errors via agent_handle_llm_error', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockStreamChat.mockImplementation(async function* () {
        yield; // Satisfy require-yield
        throw new Error('Test error');
      });

      renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      // Trigger event
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          messages: [],
          model: 'gpt-4',
          provider: 'openai',
          apiKey: 'test-key',
        },
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMError).toHaveBeenCalledWith(
          'test-session',
          expect.objectContaining({
            type: 'AI_SERVICE_ERROR',
            displayMessage: 'Test error',
          }),
        );
      });
    });

    it('should reject payloads that overflow after context injection', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
        streamChat: mockStreamChat,
        listModels: mockListModels,
        dispose: mockDispose,
        sanitizeMessages: vi.fn((messages: Message[]) => messages),
        prepareContextInjection: vi.fn((_systemPrompt, _sessionContext, messages) => ({
          systemPrompt: 'x'.repeat(30000),
          messages,
        })),
      });

      renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          messages: [
            {
              id: 'msg1',
              sessionId: 'test-session',
              threadId: 'test-session',
              role: 'user',
              content: [{ type: 'text', text: 'Hello' }],
              createdAt: new Date(),
            },
          ],
          model: 'gpt-4',
          provider: 'openai',
          contextUsage: {
            totalTokens: 100,
            contextWindow: 4096,
            modelMaxContext: 128000,
          },
        },
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMError).toHaveBeenCalledWith(
          'test-session',
          expect.objectContaining({
            type: 'CONTEXT_LIMIT_ERROR',
            displayMessage: expect.stringContaining(
              'Prepared payload exceeds the effective context limit',
            ),
          }),
        );
      });
      expect(agentCommands.handleLLMError).toHaveBeenCalledTimes(1);
      expect(AIServiceFactory.getService).toHaveBeenCalledTimes(1);
      expect(mockStreamChat).not.toHaveBeenCalled();
    });

    it('should skip payload overflow preflight in sliding-window mode', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ content: 'window-mode ok' });
      });

      (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
        streamChat: mockStreamChat,
        listModels: mockListModels,
        dispose: mockDispose,
        sanitizeMessages: vi.fn((messages: Message[]) => messages),
        prepareContextInjection: vi.fn((_systemPrompt, _sessionContext, messages) => ({
          systemPrompt: 'x'.repeat(30000),
          messages,
        })),
      });

      renderHook(() => useLLMService(), {
        wrapper: WindowStrategyWrapper,
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      await eventHandler?.({
        payload: {
          sessionId: 'window-session',
          messages: [
            {
              id: 'msg1',
              sessionId: 'window-session',
              threadId: 'window-session',
              role: 'user',
              content: [{ type: 'text', text: 'Hello' }],
              createdAt: new Date(),
            },
          ],
          model: 'gpt-4',
          provider: 'openai',
          contextUsage: {
            totalTokens: 100,
            contextWindow: 4096,
            modelMaxContext: 128000,
          },
        },
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMResponse).toHaveBeenCalledWith(
          'window-session',
          expect.objectContaining({
            role: 'assistant',
          }),
        );
      });

      expect(agentCommands.handleLLMError).not.toHaveBeenCalledWith(
        'window-session',
        expect.objectContaining({
          type: 'CONTEXT_LIMIT_ERROR',
        }),
      );
      expect(mockStreamChat).toHaveBeenCalledTimes(1);
    });
  });

  // ─── SP4 regression: Retry & Fallback Recovery ───────────────────────────
  describe('SP4 – Retry & Fallback Recovery', () => {
    // Capture event handler registered via listen()
    let eventHandler: ((event: unknown) => Promise<void>) | undefined;

    beforeEach(() => {
      eventHandler = undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-request') {
            eventHandler = handler as (event: unknown) => Promise<void>;
          }
          return mockUnlisten;
        },
      );
    });

    /** Trigger llm:completion-request and wait for it to settle */
    const triggerEvent = async (model = 'gpt-4', provider = 'openai') => {
      await eventHandler?.({
        payload: {
          sessionId: 'sp4-session',
          messages: [
            {
              id: 'u1',
              sessionId: 'sp4-session',
              role: 'user',
              content: [{ type: 'text', text: 'hello' }],
              createdAt: new Date(),
            },
          ],
          model,
          provider,
          apiKey: 'test-key',
        },
      });
    };

    it('SP4: succeeds on retry after transient API failure', async () => {
      // First attempt throws; second succeeds
      mockStreamChat
        .mockImplementationOnce(async function* () {
          yield; // satisfy require-yield
          throw new Error('transient');
        })
        .mockImplementationOnce(async function* () {
          yield JSON.stringify({ content: 'recovered' });
        });

      renderHook(() => useLLMService(), { wrapper: TestWrapper });
      await waitFor(() => expect(eventHandler).toBeDefined());

      await act(async () => {
        await triggerEvent();
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMResponse).toHaveBeenCalled();
      });

      // Primary was called twice (1 initial + 1 retry), no error reported
      expect(mockStreamChat).toHaveBeenCalledTimes(2);
      expect(agentCommands.handleLLMError).not.toHaveBeenCalled();
    });

    it('SP4: exhausts all retries and reports error when no fallback configured', async () => {
      // All 4 attempts fail (attempt 0 + retries 1-3)
      mockStreamChat.mockImplementation(async function* () {
        yield;
        throw new Error('persistent failure');
      });

      renderHook(() => useLLMService(), { wrapper: TestWrapper });
      await waitFor(() => expect(eventHandler).toBeDefined());

      await act(async () => {
        await triggerEvent();
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMError).toHaveBeenCalledWith(
          'sp4-session',
          expect.objectContaining({
            type: 'AI_SERVICE_ERROR',
            displayMessage: 'persistent failure',
          }),
        );
      });

      // 4 attempts total (1 initial + 3 retries), no fallback
      expect(mockStreamChat).toHaveBeenCalledTimes(4);
    });

    it('SP4: switches to fallback model after primary exhausts all retries', async () => {
      // Primary fails 4 times; fallback succeeds on 5th call
      mockStreamChat
        .mockImplementationOnce(async function* () { yield; throw new Error('api err'); })
        .mockImplementationOnce(async function* () { yield; throw new Error('api err'); })
        .mockImplementationOnce(async function* () { yield; throw new Error('api err'); })
        .mockImplementationOnce(async function* () { yield; throw new Error('api err'); })
        .mockImplementationOnce(async function* () {
          yield JSON.stringify({ content: 'fallback ok' });
        });

      // Custom wrapper that provides settings with a fallback model configured
      const fallbackSettings = {
        ...DEFAULT_SETTING,
        fallbackModel: {
          provider: 'anthropic' as AIServiceProvider,
          model: 'claude-3-5-sonnet-20241022',
        },
      };

      function WrapperWithFallback({ children }: { children: ReactNode }) {
        return (
          <SettingsContext.Provider
            value={{ value: fallbackSettings, update: vi.fn(), isLoading: false, error: null }}
          >
            <LLMServiceProvider>{children}</LLMServiceProvider>
          </SettingsContext.Provider>
        );
      }

      renderHook(() => useLLMService(), { wrapper: WrapperWithFallback });
      await waitFor(() => expect(eventHandler).toBeDefined());

      await act(async () => {
        await eventHandler?.({
          payload: {
            sessionId: 'sp4-session',
            messages: [
              {
                id: 'u1',
                sessionId: 'sp4-session',
                role: 'user',
                content: [{ type: 'text', text: 'hello' }],
                createdAt: new Date(),
              },
            ],
            model: 'test-model',
            provider: 'openai',
            apiKey: 'test-key',
            contextUsage: {
              totalTokens: 100,
              contextWindow: 4096,
              modelMaxContext: 128000,
            },
          },
        });
      });

      await waitFor(() => {
        expect(agentCommands.handleLLMResponse).toHaveBeenCalled();
      });

      // 5 total calls: 4 primary + 1 fallback
      expect(mockStreamChat).toHaveBeenCalledTimes(5);
      // Last call to getService used the fallback provider
      const getServiceCalls = (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mock.calls;
      expect(getServiceCalls[getServiceCalls.length - 1][0]).toBe('anthropic');
      expect(agentCommands.handleLLMError).not.toHaveBeenCalled();
    });

    it('SP4: abort error is not retried and is silently swallowed', async () => {
      // Throw an AbortError on the first (and only) attempt
      mockStreamChat.mockImplementation(async function* () {
        yield;
        const err = new DOMException('User aborted', 'AbortError');
        throw err;
      });

      renderHook(() => useLLMService(), { wrapper: TestWrapper });
      await waitFor(() => expect(eventHandler).toBeDefined());

      await act(async () => {
        await triggerEvent();
      });
      // Neither error nor success should have been reported to Rust
      expect(agentCommands.handleLLMResponse).not.toHaveBeenCalled();
      expect(agentCommands.handleLLMError).not.toHaveBeenCalled();

      // Only 1 attempt — no retries on abort
      expect(mockStreamChat).toHaveBeenCalledTimes(1);
    });

  });
});
