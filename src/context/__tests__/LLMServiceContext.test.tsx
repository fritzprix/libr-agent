import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LLMServiceProvider, useLLMService } from '../LLMServiceContext';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { Message } from '@/models/chat';
import { SettingsProvider } from '../SettingsContext';
import type { ReactNode } from 'react';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock AIServiceFactory
vi.mock('@/lib/ai-service/factory', () => ({
  AIServiceFactory: {
    getService: vi.fn(),
  },
}));

import { SystemPromptProvider } from '../SystemPromptContext';

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

// Test wrapper with required providers
function TestWrapper({ children }: { children: ReactNode }) {
  return (
    <SettingsProvider>
      <SystemPromptProvider>
        <LLMServiceProvider>{children}</LLMServiceProvider>
      </SystemPromptProvider>
    </SettingsProvider>
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

      const resultMessage = await result.current.executeCompletionRequest(
        'test-session',
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

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

      const resultMessage = await result.current.executeCompletionRequest(
        'test-session',
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

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

      const resultMessage = await result.current.executeCompletionRequest(
        'test-session',
        messages,
        'gpt-4',
        'openai',
        'test-key',
      );

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
        expect(invoke).toHaveBeenCalledWith(
          'agent_handle_llm_response',
          expect.objectContaining({
            sessionId: 'test-session',
          }),
        );
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
        expect(invoke).toHaveBeenCalledWith(
          'agent_handle_llm_error',
          expect.objectContaining({
            sessionId: 'test-session',
            error: 'Test error',
          }),
        );
      });
    });
  });
});

