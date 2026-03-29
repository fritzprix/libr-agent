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

  it('reuses the same Gemini cache entry regardless of available tool order', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];
    const firstTool: MCPTool = {
      name: 'workspace__readFile',
      description: 'A'.repeat(70000),
      inputSchema: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'File path' },
        },
        required: ['path'],
      },
    };
    const secondTool: MCPTool = {
      name: 'workspace__writeFile',
      description: 'B'.repeat(70000),
      inputSchema: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'File path' },
          content: { type: 'string', description: 'Content' },
        },
        required: ['content', 'path'],
      },
    };

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'stable prompt',
      availableTools: [firstTool, secondTool],
    }));

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'stable prompt',
      availableTools: [secondTool, firstTool],
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(1);

    const firstCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: { cachedContent?: string };
    };
    const secondCall = generateContentStreamMock.mock.calls[1]?.[0] as {
      config?: { cachedContent?: string };
    };

    expect(firstCall.config?.cachedContent).toBe('cachedContents/1');
    expect(secondCall.config?.cachedContent).toBe('cachedContents/1');
  });

  it('moves volatile session context into a synthetic tail message for Gemini', () => {
    const service = new GeminiService('test-key');

    const prepared = service.prepareContextInjection(
      'Stable system prompt',
      '# Current Context Information\nvolatile bits',
      [createUserMessage('hello')],
    );

    expect(prepared.systemPrompt).toBe('Stable system prompt');
    expect(prepared.sessionContext).toBeUndefined();
    expect(prepared.messages).toHaveLength(2);
    expect(prepared.messages[1]).toMatchObject({
      id: 'gemini-session-context-msg-hello',
      sessionId: 'session-1',
      threadId: 'thread-1',
      role: 'user',
      content: [
        {
          type: 'text',
          text: `[Current session context — background reference only, do not respond to this block]\n\n# Current Context Information\nvolatile bits\n\n[End of session context]`,
        },
      ],
    });
  });

  it('emits tool calls as soon as Gemini stream includes a functionCall part', async () => {
    generateContentStreamMock.mockResolvedValue({
      [Symbol.asyncIterator]() {
        let step = 0;
        return {
          next: async () => {
            step += 1;
            if (step === 1) {
              return {
                done: false,
                value: {
                  candidates: [
                    {
                      content: {
                        parts: [
                          {
                            functionCall: {
                              id: 'call_gemini_1',
                              name: 'workspace__writeFile',
                              args: { path: 'foo.txt', content: 'hello' },
                            },
                            thoughtSignature: 'sig_123',
                          },
                        ],
                      },
                    },
                  ],
                },
              };
            }

            return { done: true, value: undefined };
          },
        };
      },
    });

    const service = new GeminiService('test-key');
    const stream = service.streamChat([createUserMessage('write file')], {
      modelName: 'gemini-2.5-flash',
    });

    const observedChunks: Array<Record<string, unknown>> = [];
    for await (const chunk of stream) {
      observedChunks.push(JSON.parse(chunk) as Record<string, unknown>);
    }

    const toolCallChunk = observedChunks.find(
      (chunk) =>
        Array.isArray((chunk as { tool_calls?: unknown[] }).tool_calls) &&
        ((chunk as { tool_calls?: unknown[] }).tool_calls?.length ?? 0) > 0,
    ) as
      | {
          tool_calls?: Array<{
            id?: string;
            function?: { name?: string; arguments?: string };
          }>;
        }
      | undefined;
    const signatureChunk = observedChunks.find(
      (chunk) =>
        typeof (chunk as { thinkingSignature?: unknown }).thinkingSignature ===
        'string',
    ) as { thinkingSignature?: string } | undefined;

    expect(toolCallChunk).toBeDefined();
    expect(signatureChunk).toBeDefined();

    expect(toolCallChunk?.tool_calls?.[0]).toEqual({
      id: 'call_gemini_1',
      function: {
        name: 'workspace__writeFile',
        arguments: JSON.stringify({ path: 'foo.txt', content: 'hello' }),
      },
    });
    expect(signatureChunk?.thinkingSignature).toBe('sig_123');
  });

  it('emits tool call before later plain-text chunks once functionCall appears', async () => {
    generateContentStreamMock.mockResolvedValue({
      [Symbol.asyncIterator]() {
        let step = 0;
        return {
          next: async () => {
            step += 1;
            if (step === 1) {
              return {
                done: false,
                value: {
                  candidates: [
                    {
                      content: {
                        parts: [
                          {
                            functionCall: {
                              name: 'workspace__writeFile',
                              args: { path: 'foo.txt' },
                            },
                          },
                        ],
                      },
                    },
                  ],
                },
              };
            }
            if (step === 2) {
              return {
                done: false,
                value: {
                  text: 'after tool call',
                  candidates: [{ content: { parts: [] } }],
                },
              };
            }

            return { done: true, value: undefined };
          },
        };
      },
    });

    const service = new GeminiService('test-key');
    const observedChunks: Array<Record<string, unknown>> = [];
    for await (const chunk of service.streamChat([createUserMessage('write file')], {
      modelName: 'gemini-2.5-flash',
    })) {
      observedChunks.push(JSON.parse(chunk) as Record<string, unknown>);
    }

    const toolCallIndex = observedChunks.findIndex(
      (chunk) =>
        Array.isArray((chunk as { tool_calls?: unknown[] }).tool_calls) &&
        ((chunk as { tool_calls?: unknown[] }).tool_calls?.length ?? 0) > 0,
    );
    const textIndex = observedChunks.findIndex(
      (chunk) => typeof (chunk as { content?: unknown }).content === 'string',
    );

    expect(toolCallIndex).toBeGreaterThanOrEqual(0);
    expect(textIndex).toBeGreaterThanOrEqual(0);
    expect(toolCallIndex).toBeLessThan(textIndex);
  });
});
