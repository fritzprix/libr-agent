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

  it('passes through the provided AbortSignal in stream config', async () => {
    const { GeminiService } = await import('../gemini');
    const service = new GeminiService('test-key');
    const controller = new AbortController();

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gemini-2.5-flash',
        signal: controller.signal,
      }),
    );

    const request = generateContentStreamMock.mock.calls[0]?.[0] as
      | { config?: { abortSignal?: AbortSignal } }
      | undefined;

    expect(request?.config?.abortSignal).toBe(controller.signal);
    expect(request?.config?.abortSignal?.aborted).toBe(false);

    controller.abort();

    expect(request?.config?.abortSignal?.aborted).toBe(true);
  });

  it('does not inject a synthetic AbortSignal when none is provided', async () => {
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

    expect(request?.config?.abortSignal).toBeUndefined();
  });

  it('passes through the provided AbortSignal in non-stream config', async () => {
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
    const controller = new AbortController();

    await service.sampleText('hello', {
      modelName: 'gemini-2.5-flash',
      signal: controller.signal,
    });

    const request = generateContentMock.mock.calls[0]?.[0] as
      | { config?: { abortSignal?: AbortSignal } }
      | undefined;

    expect(request?.config?.abortSignal).toBe(controller.signal);
  });

  it('retries malformed function call failures once without tools', async () => {
    generateContentStreamMock
      .mockRejectedValueOnce(new Error('MALFORMED_FUNCTION_CALL'))
      .mockResolvedValueOnce(createEmptyStream());

    const { GeminiService } = await import('../gemini');
    const service = new GeminiService('test-key');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gemini-2.5-flash',
        availableTools: [
          {
            name: 'workspace__writeFile',
            description: 'Write a file',
            inputSchema: {
              type: 'object',
              properties: {},
            },
          },
        ],
      }),
    );

    expect(generateContentStreamMock).toHaveBeenCalledTimes(2);

    const firstRequest = generateContentStreamMock.mock.calls[0]?.[0] as
      | { config?: { tools?: unknown[] } }
      | undefined;
    const secondRequest = generateContentStreamMock.mock.calls[1]?.[0] as
      | { config?: { tools?: unknown[] } }
      | undefined;

    expect(firstRequest?.config?.tools).toBeDefined();
    expect(secondRequest?.config?.tools).toBeUndefined();
  });
});
