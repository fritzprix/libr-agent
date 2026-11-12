import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LLMServiceProvider, useLLMService } from '../LLMServiceContext';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import * as agentCommands from '@/lib/backend/agent-commands';
import type { Message } from '@/models/chat';
import { SettingsContext } from '../SettingsContext';
import { TestSettingsProvider } from '@/test/fixtures/TestSettingsProvider';
import type { ReactNode } from 'react';
import type { AIServiceProvider } from '@/lib/ai-service';
import { TEST_SETTINGS } from '@/test/fixtures/settings';
import { sleep } from '@/lib/retry-utils';

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
}));

// Mock AIServiceFactory
vi.mock('@/lib/ai-service/factory', () => ({
  AIServiceFactory: {
    getService: vi.fn(),
  },
}));

// Mock logger
const toastMessage = vi.hoisted(() => vi.fn());
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), {
    message: toastMessage,
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

// Mock retry-utils
vi.mock('@/lib/retry-utils', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/retry-utils')>();
  return { ...actual, sleep: vi.fn().mockResolvedValue(undefined) };
});

// Test wrappers
function TestWrapper({ children }: { children: ReactNode }) {
  return (
    <TestSettingsProvider>
      <LLMServiceProvider>{children}</LLMServiceProvider>
    </TestSettingsProvider>
  );
}

describe('LLMServiceContext – SP4 Retry & Fallback Recovery', () => {
  const mockUnlisten = vi.fn();
  const mockStreamChat = vi.fn();
  const mockListModels = vi.fn();
  const mockDispose = vi.fn();

  // Capture event handler registered via listen()
  let eventHandler: ((event: unknown) => Promise<void>) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();

    // Setup listen mock
    (listen as ReturnType<typeof vi.fn>).mockImplementation(
      async (eventName, handler) => {
        if (eventName === 'llm:completion-request') {
          eventHandler = handler as (event: unknown) => Promise<void>;
        }
        return mockUnlisten;
      },
    );

    // Setup AIServiceFactory mock
    (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
      streamChat: mockStreamChat,
      listModels: mockListModels,
      dispose: mockDispose,
      sanitizeMessages: vi.fn((messages: Message[]) => messages),
    });

    // Setup mockListModels to return test models
    mockListModels.mockResolvedValue([
      {
        name: 'gpt-4',
        contextWindow: 4096,
        supportReasoning: false,
        supportTools: true,
        supportStreaming: true,
        cost: { input: 0.001, output: 0.002 },
        description: 'Test model for unit tests',
      },
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
      ...TEST_SETTINGS,
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

  it('SP4: unmarked abort error is not retried and is reported to Rust', async () => {
    // Throw an AbortError without an intentional cancellation marker.
    mockStreamChat.mockImplementation(async function* () {
      yield;
      const err = new DOMException('Transport aborted', 'AbortError');
      throw err;
    });

    renderHook(() => useLLMService(), { wrapper: TestWrapper });
    await waitFor(() => expect(eventHandler).toBeDefined());

    await act(async () => {
      await triggerEvent();
    });
    // An unmarked abort must settle the backend session instead of leaving it Busy.
    expect(agentCommands.handleLLMResponse).not.toHaveBeenCalled();
    expect(agentCommands.handleLLMError).toHaveBeenCalledWith(
      'sp4-session',
      expect.objectContaining({
        displayMessage: 'LLM completion aborted unexpectedly',
      }),
    );

    // Only 1 attempt — no retries on abort
    expect(mockStreamChat).toHaveBeenCalledTimes(1);
  });

  describe('recovery cancel toast', () => {
    const registerCancelHandler = async () => {
      let cancelHandler: ((event: unknown) => void) | undefined;
      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-cancel') {
            cancelHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );
      renderHook(() => useLLMService(), { wrapper: TestWrapper });
      await waitFor(() => expect(cancelHandler).toBeDefined());
      return (willRetry: boolean) =>
        cancelHandler?.({
          payload: {
            sessionId: 'sp4-session',
            responseMessageId: 'resp-1',
            reason: 'reasoning-budget-exceeded',
            willRetry,
          },
        });
    };

    beforeEach(() => {
      toastMessage.mockClear();
    });

    it('shows the "Retrying" toast when Rust starts a fresh turn', async () => {
      const cancel = await registerCancelHandler();

      act(() => {
        cancel(true);
      });

      expect(toastMessage).toHaveBeenCalledTimes(1);
    });

    it('shows no "Retrying" toast when the turn ends in failure', async () => {
      const cancel = await registerCancelHandler();

      act(() => {
        cancel(false);
      });

      expect(toastMessage).not.toHaveBeenCalled();
    });
  });

  describe('cancel between attempts', () => {
    const fallbackSettings = {
      ...TEST_SETTINGS,
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

    const triggerWithResponseId = () =>
      eventHandler?.({
        payload: {
          sessionId: 'sp4-session',
          responseMessageId: 'resp-1',
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
        },
      });

    const usedFallbackProvider = () =>
      (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mock.calls.some(
        ([provider]) => provider === 'anthropic',
      );

    it('Rust cancel during the retry wait stops retries and the fallback', async () => {
      let cancelHandler: ((event: unknown) => void) | undefined;
      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'llm:completion-request') {
            eventHandler = handler as (event: unknown) => Promise<void>;
          }
          if (eventName === 'llm:completion-cancel') {
            cancelHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );
      mockStreamChat.mockImplementation(async function* () {
        yield;
        throw new Error('api err');
      });
      vi.mocked(sleep).mockImplementationOnce(async () => {
        cancelHandler?.({
          payload: {
            sessionId: 'sp4-session',
            responseMessageId: 'resp-1',
            reason: 'user',
          },
        });
      });

      renderHook(() => useLLMService(), { wrapper: WrapperWithFallback });
      await waitFor(() => {
        expect(eventHandler).toBeDefined();
        expect(cancelHandler).toBeDefined();
      });

      await act(async () => {
        await triggerWithResponseId();
      });

      expect(mockStreamChat).toHaveBeenCalledTimes(1);
      expect(usedFallbackProvider()).toBe(false);
      expect(agentCommands.handleLLMResponse).not.toHaveBeenCalled();
      expect(agentCommands.handleLLMError).not.toHaveBeenCalled();
    });

    it('UI stop just before the fallback starts prevents the fallback call', async () => {
      const { result } = renderHook(() => useLLMService(), {
        wrapper: WrapperWithFallback,
      });
      await waitFor(() => expect(eventHandler).toBeDefined());

      let primaryAttempts = 0;
      mockStreamChat.mockImplementation(async function* () {
        yield;
        primaryAttempts += 1;
        if (primaryAttempts === 4) {
          // Last primary attempt: user stops, then the attempt fails.
          result.current.cancelCompletionRequest('sp4-session');
        }
        throw new Error('api err');
      });

      await act(async () => {
        await triggerWithResponseId();
      });

      expect(mockStreamChat).toHaveBeenCalledTimes(4);
      expect(usedFallbackProvider()).toBe(false);
      expect(agentCommands.handleLLMResponse).not.toHaveBeenCalled();
      expect(agentCommands.handleLLMError).not.toHaveBeenCalled();
    });
  });
});
