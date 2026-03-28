import { beforeEach, describe, expect, it, vi } from 'vitest';
import { GeminiService } from '../gemini';
import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';

const createCacheMock = vi.fn();
const deleteCacheMock = vi.fn();
const generateContentStreamMock = vi.fn();

vi.mock('@google/genai', () => ({
  GoogleGenAI: vi.fn().mockImplementation(() => ({
    caches: {
      create: createCacheMock,
      delete: deleteCacheMock,
    },
    models: {
      generateContentStream: generateContentStreamMock,
    },
  })),
  createPartFromFunctionResponse: vi.fn((id, name, response) => ({
    functionResponse: { id, name, response },
  })),
  FinishReason: {
    STOP: 'STOP',
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

async function consumeStream(stream: AsyncGenerator<string, void, void>): Promise<void> {
  for await (const chunk of stream) {
    void chunk;
  }
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

function createTool(description: string): MCPTool {
  return {
    name: 'workspace__readFile',
    description,
    inputSchema: {
      type: 'object',
      properties: {
        path: {
          type: 'string',
          description: 'File path',
        },
      },
      required: ['path'],
    },
  };
}

describe('GeminiService context cache', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    let cacheCounter = 0;

    createCacheMock.mockImplementation(async () => {
      cacheCounter += 1;
      return { name: `cachedContents/${cacheCounter}` };
    });
    deleteCacheMock.mockResolvedValue(undefined);
    generateContentStreamMock.mockResolvedValue(createEmptyStream());
  });

  it('reuses separate cached-content entries for different stable prefixes', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];
    const stablePrefixA = 'A'.repeat(131072);
    const stablePrefixB = 'B'.repeat(131072);

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: stablePrefixA,
    }));

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: stablePrefixB,
    }));

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: stablePrefixA,
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(2);
    expect(generateContentStreamMock).toHaveBeenCalledTimes(3);

    const firstCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: { cachedContent?: string };
    };
    const secondCall = generateContentStreamMock.mock.calls[1]?.[0] as {
      config?: { cachedContent?: string };
    };
    const thirdCall = generateContentStreamMock.mock.calls[2]?.[0] as {
      config?: { cachedContent?: string };
    };

    expect(firstCall.config?.cachedContent).toBe('cachedContents/1');
    expect(secondCall.config?.cachedContent).toBe('cachedContents/2');
    expect(thirdCall.config?.cachedContent).toBe('cachedContents/1');
  });

  it('counts tool declarations toward Gemini cache eligibility', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];
    const tool = createTool('D'.repeat(131072));

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'short stable prompt',
      availableTools: [tool],
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(1);

    const firstCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: { cachedContent?: string };
    };
    expect(firstCall.config?.cachedContent).toBe('cachedContents/1');
  });

  it('skips cached-content mode when tool usage is explicitly disabled', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];
    const tool = createTool('D'.repeat(131072));

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'A'.repeat(131072),
      availableTools: [tool],
      disableToolUse: true,
    }));

    expect(createCacheMock).not.toHaveBeenCalled();

    const firstCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: { cachedContent?: string; functionCallingConfig?: { mode?: string } };
    };
    expect(firstCall.config?.cachedContent).toBeUndefined();
    expect(firstCall.config?.functionCallingConfig?.mode).toBe('none');
  });
});
