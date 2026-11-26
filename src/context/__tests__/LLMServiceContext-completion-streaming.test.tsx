import {
  TestWrapper,
  useLLMServiceHarness,
  setupLLMCompletionMocks,
} from './llm-service-test-utils';
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import type { Message } from '@/models/chat';
import { buildTestMessage } from './agent-session-test-utils';

describe('LLMServiceContext – Completion Streaming', () => {
  const { mockStreamChat } = setupLLMCompletionMocks();

  describe('Streaming placeholders and tool-call deltas', () => {
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
        buildTestMessage({ id: 'msg1' }),
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
        buildTestMessage({ id: 'msg1' }),
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
        buildTestMessage({ id: 'msg1' }),
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

  });
});
