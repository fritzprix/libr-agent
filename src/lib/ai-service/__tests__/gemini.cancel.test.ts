import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Message } from '@/models/chat';

const generateContentStreamMock = vi.fn();
const generateContentMock = vi.fn();

vi.mock('@google/genai', () => ({
  GoogleGenAI: vi.fn().mockImplementation(() => ({
    models: {
      generateContentStream: generateContentStreamMock,
      generateContent: generateContentMock,
    },
  })),
  FinishReason: {
    STOP: 'STOP',
  },
  FunctionCallingConfigMode: {
    ANY: 'ANY',
    AUTO: 'AUTO',
    MODE_UNSPECIFIED: 'MODE_UNSPECIFIED',
    NONE: 'NONE',
    VALIDATED: 'VALIDATED',
  },
  Type: {
    OBJECT: 'OBJECT',
    STRING: 'STRING',
    NUMBER: 'NUMBER',
    BOOLEAN: 'BOOLEAN',
    ARRAY: 'ARRAY',
  },
  HarmCategory: {},
  HarmBlockThreshold: {},
}));

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('../gemini/models', () => ({
  getDefaultModel: () => 'gemini-2.5-flash',
  fetchGeminiModels: vi.fn(),
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

describe('GeminiService cancellation wiring', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    generateContentStreamMock.mockResolvedValue(createEmptyStream());
  });

  it('passes an AbortSignal through stream config and aborts it on cancel', async () => {
    const { GeminiService } = await import('../gemini');
    const service = new GeminiService('test-key');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gemini-2.5-flash',
      }),
    );

    const request = generateContentStreamMock.mock.calls[0]?.[0] as
      | { config?: { abortSignal?: AbortSignal } }
      | undefined;

    expect(request?.config?.abortSignal).toBeInstanceOf(AbortSignal);
    expect(request?.config?.abortSignal?.aborted).toBe(false);

    service.cancel();

    expect(request?.config?.abortSignal?.aborted).toBe(true);
  });

  it('passes an AbortSignal through non-stream config', async () => {
    generateContentMock.mockResolvedValueOnce({
      candidates: [
        {
          finishReason: 'STOP',
          content: { parts: [{ text: 'ok' }] },
        },
      ],
      usageMetadata: {
        promptTokenCount: 1,
        candidatesTokenCount: 1,
        totalTokenCount: 2,
      },
    });

    const { GeminiService } = await import('../gemini');
    const service = new GeminiService('test-key');

    await service.sampleText('hello', {
      modelName: 'gemini-2.5-flash',
    });

    const request = generateContentMock.mock.calls[0]?.[0] as
      | { config?: { abortSignal?: AbortSignal } }
      | undefined;

    expect(request?.config?.abortSignal).toBeInstanceOf(AbortSignal);
  });
});
