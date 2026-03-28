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

vi.mock('../../retry-utils', () => ({
  withRetry: vi.fn().mockImplementation((fn: () => unknown) => fn()),
  withTimeout: vi.fn().mockImplementation((p: Promise<unknown>) => p),
}));

vi.mock('../../llm-config-manager', () => ({
  llmConfigManager: {
    getModelsForProvider: vi.fn().mockReturnValue({}),
    getModel: vi.fn().mockReturnValue(null),
  },
  ModelInfo: {},
}));

vi.mock('../message-normalizer', () => ({
  filterSystemErrors: vi.fn().mockImplementation((msgs: Message[]) => msgs),
  validateToolCallPairing: vi.fn().mockImplementation((msgs: Message[]) => msgs),
  MessageNormalizer: {
    sanitizeMessagesForProvider: vi.fn().mockImplementation((msgs: Message[]) => msgs),
    filterSystemErrors: vi.fn().mockImplementation((msgs: Message[]) => msgs),
    validateToolCallPairing: vi.fn().mockImplementation((msgs: Message[]) => msgs),
  },
}));

vi.mock('../model-capabilities', () => ({
  supportsThinking: vi.fn().mockResolvedValue(false),
  getContextWindow: vi.fn().mockResolvedValue(128000),
}));

async function* fakeStream() {
  yield {
    choices: [{ delta: { content: 'cached hello' } }],
    usage: {
      prompt_tokens: 10,
      completion_tokens: 2,
      total_tokens: 12,
    },
  };
}

const message: Message = {
  id: 'm1',
  sessionId: 's1',
  threadId: 't1',
  role: 'user',
  content: [{ type: 'text', text: 'hello' }],
  createdAt: new Date(),
};

describe('OpenAIService prompt cache extensions', () => {
  beforeEach(() => {
    createMock.mockReset();
    createMock.mockImplementation((request: { stream?: boolean }) => {
      if (request.stream) {
        return Promise.resolve(fakeStream());
      }

      return Promise.resolve({
        choices: [{ message: { content: 'ok' }, finish_reason: 'stop' }],
        usage: {
          prompt_tokens: 10,
          completion_tokens: 2,
          total_tokens: 12,
          prompt_tokens_details: { cached_tokens: 8 },
        },
        model: 'llama.cpp',
      });
    });
  });

  it('sends cache_prompt at the top level for custom OpenAI-compatible streaming endpoints', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test', {
      baseUrl: 'http://127.0.0.1:8080/v1',
    });

    const chunks: string[] = [];
    for await (const chunk of service.streamChat([message], {
      modelName: 'local-model',
    })) {
      chunks.push(chunk);
    }

    expect(chunks.length).toBeGreaterThan(0);

    const [request] = createMock.mock.calls[0] as [Record<string, unknown>];
    expect(request.cache_prompt).toBe(true);
    expect(request.stream).toBe(true);
    expect(request).not.toHaveProperty('extra_body');
  });

  it('does not send cache_prompt to the default OpenAI endpoint unless explicitly enabled', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    await service.sampleText('hello', { modelName: 'gpt-4o' });

    const [request] = createMock.mock.calls[0] as [Record<string, unknown>];
    expect(request.cache_prompt).toBeUndefined();
  });

  it('supports explicit prompt cache enablement for non-streaming requests', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test', {
      baseUrl: 'https://llama.example.com/v1',
      enablePromptCache: true,
    });

    await service.sampleText('hello', { modelName: 'local-model' });

    const [request] = createMock.mock.calls[0] as [Record<string, unknown>];
    expect(request.cache_prompt).toBe(true);
    expect(request.stream).toBe(false);
    expect(request).not.toHaveProperty('extra_body');
  });
});
