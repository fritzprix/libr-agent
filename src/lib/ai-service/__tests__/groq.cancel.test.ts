import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Message } from '@/models/chat';

const createMock = vi.fn();

vi.mock('groq-sdk', () => ({
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

function createUserMessage(text: string): Message {
  return {
    id: `msg-${text}`,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'user',
    content: [{ type: 'text', text }],
  };
}

async function consumeStream(
  stream: AsyncGenerator<string, void, void>,
): Promise<void> {
  for await (const chunk of stream) {
    void chunk;
  }
}

describe('GroqService cancellation wiring', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('passes an AbortSignal to streaming requests and aborts it on cancel', async () => {
    createMock.mockResolvedValueOnce(createEmptyStream());

    const { GroqService } = await import('../groq');
    const service = new GroqService('test-key');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'llama-3.1-8b-instant',
      }),
    );

    const requestOptions = createMock.mock.calls[0]?.[1] as
      | { signal?: AbortSignal }
      | undefined;

    expect(requestOptions?.signal).toBeInstanceOf(AbortSignal);
    expect(requestOptions?.signal?.aborted).toBe(false);

    service.cancel();

    expect(requestOptions?.signal?.aborted).toBe(true);
  });

  it('passes an AbortSignal to non-streaming requests', async () => {
    createMock.mockResolvedValueOnce({
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
      model: 'llama-3.1-8b-instant',
    });

    const { GroqService } = await import('../groq');
    const service = new GroqService('test-key');

    await service.sampleText('hello', {
      modelName: 'llama-3.1-8b-instant',
    });

    const requestOptions = createMock.mock.calls[0]?.[1] as
      | { signal?: AbortSignal }
      | undefined;

    expect(requestOptions?.signal).toBeInstanceOf(AbortSignal);
  });
});
