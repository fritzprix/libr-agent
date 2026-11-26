import { TestWrapper, useLLMServiceHarness } from "./llm-service-test-utils";
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
        buildTestMessage({ id: 'msg1' }),
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
        buildTestMessage({ id: 'msg1' }),
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
        ));
      });

      expect(resultMessage.tool_calls).toHaveLength(1);
      expect(resultMessage.tool_calls?.[0]).toMatchObject({
        id: 'call1',
        type: 'function',
        function: { name: 'test_tool', arguments: '{"arg": "value"}' },
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
        buildTestMessage({ id: 'msg1' }),
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
        ));
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
        buildTestMessage({ id: 'msg1' }),
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

    it('surfaces an unmarked transport abort as an error', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementation(async function* () {
        yield JSON.stringify({ content: 'partial response' });
        const error = new Error('Request aborted');
        error.name = 'AbortError';
        throw error;
      });

      const messages: Message[] = [
        buildTestMessage({ id: 'msg1' }),
      ];

      await expect(
        result.current.executeCompletionRequest(
          'test-session',
          'response-msg-unexpected-abort',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        ),
      ).rejects.toThrow('LLM completion aborted unexpectedly');

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
        buildTestMessage({ id: 'msg1' }),
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

  });
});
