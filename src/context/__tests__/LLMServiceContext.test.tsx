import { loggerInfo, loggerError, TestWrapper, useLLMServiceHarness } from "./llm-service-test-utils";
import type { Message } from '@/models/chat';
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import { useLLMService } from '../LLMServiceContext';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import * as agentCommands from '@/lib/backend/agent-commands';
import React, { type ReactNode } from 'react';
import { __resetLLMListenerStartupLogStateForTests } from '../llm/useLLMListener';

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

    it('cleans up the compact request listener when compact state registration fails', async () => {
      const completionCleanup = vi.fn();
      const cancelCleanup = vi.fn();
      const compactRequestCleanup = vi.fn();
      const registrationError = new Error('compact state registration failed');

      (listen as ReturnType<typeof vi.fn>)
        .mockReset()
        .mockResolvedValueOnce(completionCleanup)
        .mockResolvedValueOnce(cancelCleanup)
        .mockResolvedValueOnce(compactRequestCleanup)
        .mockRejectedValueOnce(registrationError);

      renderHook(() => useLLMService(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(loggerError).toHaveBeenCalledWith(
          'Failed to register compact listeners',
          registrationError,
        );
      });

      expect(compactRequestCleanup).toHaveBeenCalledTimes(1);
      expect(cancelCleanup).not.toHaveBeenCalled();
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

});
