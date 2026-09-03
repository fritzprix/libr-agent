import { beforeEach, describe, expect, it, vi } from 'vitest';

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
  repairMalformedToolCalls: vi
    .fn()
    .mockImplementation((msgs: Message[]) => msgs),
  validateToolCallPairing: vi
    .fn()
    .mockImplementation((msgs: Message[]) => msgs),
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

describe('OpenAIService temperature payload', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createMock.mockReset();
    createMock.mockResolvedValue(createEmptyStream());
  });

  it('omits temperature from stream requests when unset', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gpt-4o-mini',
      }),
    );

    const request = createMock.mock.calls[0]?.[0] as Record<string, unknown>;
    expect(request).not.toHaveProperty('temperature');
  });

  it('includes temperature on stream requests when set on config', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gpt-4o-mini',
        config: { temperature: 0.4 },
      }),
    );

    const request = createMock.mock.calls[0]?.[0] as Record<string, unknown>;
    expect(request.temperature).toBe(0.4);
  });

  it('sends reasoning_effort when thinkingEffort is set even if model metadata lacks support', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gpt-4o',
        config: { thinkingEffort: 'high' },
      }),
    );

    const request = createMock.mock.calls[0]?.[0] as Record<string, unknown>;
    expect(request.reasoning_effort).toBe('high');
  });
});
