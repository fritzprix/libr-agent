import {
  TestWrapper,
  useLLMServiceHarness,
  setupLLMCompletionMocks,
} from './llm-service-test-utils';
import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import type { Message } from '@/models/chat';
import { buildTestMessage } from './agent-session-test-utils';
import * as streamEvents from '@/lib/ai-service/stream-events';

describe('LLMServiceContext – Completion Recovery', () => {
  const { mockStreamChat } = setupLLMCompletionMocks();

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
