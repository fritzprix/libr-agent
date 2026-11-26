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
