import { TestWrapper, useLLMServiceHarness } from "./llm-service-test-utils";
import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { Message } from '@/models/chat';
import { buildTestMessage } from './agent-session-test-utils';
import { __resetLLMListenerStartupLogStateForTests } from '../llm/useLLMListener';
import * as streamEvents from '@/lib/ai-service/stream-events';

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

  describe('Thinking-only completion recovery', () => {
    it('returns thinking-only completion to Rust recovery instead of throwing in the frontend', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementationOnce(async function* () {
        yield JSON.stringify({ thinking: 'Thinking about the problem...' });
      });

      const messages: Message[] = [
        buildTestMessage({ id: 'msg1' }),
      ];

      let completedMessage: Message | undefined;
      await act(async () => {
        completedMessage = await result.current.executeCompletionRequest(
          'test-session',
          'response-msg-thinking-only',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      expect(completedMessage).toBeDefined();
      expect(completedMessage?.thinking).toBe('Thinking about the problem...');
      expect(completedMessage?.tool_calls).toBeUndefined();
    });

    it('returns thinking-only completion with whitespace-only text to Rust recovery', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      mockStreamChat.mockImplementationOnce(async function* () {
        yield JSON.stringify({
          thinking: 'Thinking about the problem...',
          content: '   \n  ',
        });
      });

      const messages: Message[] = [
        buildTestMessage({ id: 'msg1' }),
      ];

      let completedMessage: Message | undefined;
      await act(async () => {
        completedMessage = await result.current.executeCompletionRequest(
          'test-session',
          'response-msg-thinking-empty-text',
          messages,
          'gpt-4',
          'openai',
          'test-key',
        );
      });

      expect(completedMessage).toBeDefined();
      expect(completedMessage?.thinking).toBe('Thinking about the problem...');
      expect(completedMessage?.tool_calls).toBeUndefined();
    });

    it('does NOT throw when thinking is accompanied by non-text content (e.g. image)', async () => {
      const { result } = renderHook(() => useLLMServiceHarness(), {
        wrapper: TestWrapper,
      });

      const parseSpy = vi.spyOn(streamEvents, 'parseStreamChunk').mockImplementation((rawChunk: unknown) => {
        if (typeof rawChunk !== 'string') {
          return (rawChunk as streamEvents.ParsedStreamChunk) ?? {};
        }
        try {
          return JSON.parse(rawChunk);
        } catch {
          return { content: rawChunk };
        }
      });

      mockStreamChat.mockImplementationOnce(async function* () {
        yield JSON.stringify({
          thinking: 'Thinking about the image...',
          content: [{ type: 'image', data: 'base64...' }],
        });
      });

      const messages: Message[] = [
        buildTestMessage({ id: 'msg1' }),
      ];

      let errorThrown: unknown;
      await act(async () => {
        try {
          await result.current.executeCompletionRequest(
            'test-session',
            'response-msg-thinking-and-image',
            messages,
            'gpt-4',
            'openai',
            'test-key',
          );
        } catch (error) {
          errorThrown = error;
        }
      });

      expect(errorThrown).toBeUndefined();
      parseSpy.mockRestore();
    });

  });
});
