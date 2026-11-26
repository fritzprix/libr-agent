import { TestWrapper, useLLMServiceHarness, createDeferred } from "./llm-service-test-utils";
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { Message } from '@/models/chat';
import { buildTestMessage } from './agent-session-test-utils';
import { __resetLLMListenerStartupLogStateForTests } from '../llm/useLLMListener';

describe('LLMServiceContext – Completion Execution', () => {
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

  describe('Cancellation and supersede', () => {
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
        buildTestMessage({ id: 'msg1' }),
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
      const firstChunkGate = createDeferred<void>();

      mockStreamChat.mockImplementation(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        await firstChunkGate.promise;
        if (options?.signal?.aborted) {
          return;
        }
        yield JSON.stringify({ content: 'late reply' });
      });

      const messages: Message[] = [
        buildTestMessage({ id: 'msg1' }),
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
      const abortedExpectation = expect(requestPromise).rejects.toThrow(
        'Request aborted',
      );

      await act(async () => {
        firstChunkGate.resolve();
      });

      await abortedExpectation;
      expect(mockDispose).not.toHaveBeenCalled();
    });

    it('treats a silent pre-first-chunk supersede as superseded instead of empty response', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });
      const firstChunkGate = createDeferred<void>();
      const secondChunkGate = createDeferred<void>();

      mockStreamChat.mockImplementationOnce(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        await firstChunkGate.promise;
        if (options?.signal?.aborted) {
          return;
        }
        yield JSON.stringify({ content: 'stale reply' });
      });
      mockStreamChat.mockImplementationOnce(async function* (
        _messages: Message[],
        options?: { signal?: AbortSignal },
      ) {
        await secondChunkGate.promise;
        if (options?.signal?.aborted) {
          return;
        }
        yield JSON.stringify({ content: 'fresh reply' });
      });

      const messages: Message[] = [
        buildTestMessage({ id: 'msg1' }),
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
      const firstExpectation = expect(firstPromise).rejects.toThrow(
        'Request superseded',
      );
      const secondExpectation = expect(secondPromise).resolves.toMatchObject({
        content: [{ type: 'text', text: 'fresh reply' }],
      });

      await act(async () => {
        firstChunkGate.resolve();
        secondChunkGate.resolve();
      });

      await firstExpectation;
      await secondExpectation;
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
        buildTestMessage({ id: 'msg1' }),
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
