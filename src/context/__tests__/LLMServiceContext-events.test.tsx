import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LLMServiceProvider, useLLMService } from '../LLMServiceContext';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import * as agentCommands from '@/lib/backend/agent-commands';
import type { Message } from '@/models/chat';
import { SettingsProvider, SettingsContext, DEFAULT_SETTING } from '../SettingsContext';
import type { ReactNode } from 'react';

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

// Mock retry-utils
vi.mock('@/lib/retry-utils', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/retry-utils')>();
  return { ...actual, sleep: vi.fn().mockResolvedValue(undefined) };
});

// Test wrappers
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

describe('LLMServiceContext – Event Handling', () => {
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
      prepareContextInjection: vi.fn((systemPrompt, _sessionContext, messages) => ({
        systemPrompt,
        messages,
      })),
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
    ]);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

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
    await act(async () => {
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
    await act(async () => {
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          messages: [],
          model: 'gpt-4',
          provider: 'openai',
          apiKey: 'test-key',
        },
      });
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

    await act(async () => {
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
        },
      });
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

    await act(async () => {
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
        },
      });
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
