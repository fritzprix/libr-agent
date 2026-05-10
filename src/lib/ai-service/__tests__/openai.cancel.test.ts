import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Message } from '@/models/chat';

const createMock = vi.fn();

vi.mock('openai', () => ({
  default: vi.fn().mockImplementation(() => ({
    chat: {
      completions: {
        create: createMock,
      },
    },
  })),
}));

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('../../llm-config-manager', () => ({
  llmConfigManager: {
    getModelsForProvider: vi.fn().mockReturnValue({}),
    getModel: vi.fn().mockReturnValue(null),
  },
}));

vi.mock('../message-normalizer', () => ({
  filterSystemErrors: vi.fn().mockImplementation((msgs: Message[]) => msgs),
  repairMalformedToolCalls: vi.fn().mockImplementation((msgs: Message[]) => msgs),
  validateToolCallPairing: vi.fn().mockImplementation((msgs: Message[]) => msgs),
}));

vi.mock('../model-capabilities', () => ({
  supportsThinking: vi.fn().mockResolvedValue(false),
  getContextWindow: vi.fn().mockResolvedValue(128000),
}));

function createUserMessage(text: string): Message {
  return {
    id: `msg-${text}`,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'user',
    content: [{ type: 'text', text }],
  };
}

function createEmptyStream(): AsyncIterable<never> {
  return {
    [Symbol.asyncIterator]() {
      return {
        next: async () => ({
          done: true,
          value: undefined as never,
        }),
      };
    },
  };
}

async function consumeStream(
  stream: AsyncGenerator<string, void, void>,
): Promise<void> {
  for await (const chunk of stream) {
    void chunk;
  }
}

describe('OpenAIService cancellation wiring', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createMock.mockReset();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('keeps the provided AbortSignal across stream retry scheduling and aborts before retrying', async () => {
    createMock
      .mockRejectedValueOnce({ status: 500 })
      .mockResolvedValueOnce(createEmptyStream());

    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test', { retryDelay: 1000 });
    const controller = new AbortController();

    const streamPromise = consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gpt-4o-mini',
        signal: controller.signal,
      }),
    );
    const streamExpectation = expect(streamPromise).rejects.toMatchObject({
      name: 'AbortError',
    });

    await Promise.resolve();
    controller.abort();
    await vi.runAllTimersAsync();
    await streamExpectation;

    const firstOptions = createMock.mock.calls[0]?.[1] as
      | { signal?: AbortSignal }
      | undefined;

    expect(createMock).toHaveBeenCalledTimes(1);
    expect(firstOptions?.signal).toBe(controller.signal);
    expect(firstOptions?.signal?.aborted).toBe(true);
  });

  it('keeps the provided AbortSignal across non-stream retry scheduling and aborts before retrying', async () => {
    createMock
      .mockRejectedValueOnce({ status: 500 })
      .mockResolvedValueOnce({
        choices: [
          {
            finish_reason: 'stop',
            message: { content: 'ok' },
          },
        ],
        usage: {
          prompt_tokens: 1,
          completion_tokens: 1,
          total_tokens: 2,
        },
        model: 'gpt-4o-mini',
      });

    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test', { retryDelay: 1000 });
    const controller = new AbortController();

    const samplePromise = service.sampleText('hello', {
      modelName: 'gpt-4o-mini',
      signal: controller.signal,
    });
    const sampleExpectation = expect(samplePromise).rejects.toMatchObject({
      name: 'AbortError',
    });

    await Promise.resolve();
    controller.abort();
    await vi.runAllTimersAsync();
    await sampleExpectation;

    const firstOptions = createMock.mock.calls[0]?.[1] as
      | { signal?: AbortSignal }
      | undefined;

    expect(createMock).toHaveBeenCalledTimes(1);
    expect(firstOptions?.signal).toBe(controller.signal);
    expect(firstOptions?.signal?.aborted).toBe(true);
  });
});
