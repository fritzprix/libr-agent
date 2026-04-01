import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Type } from '@google/genai';
import { GeminiService } from '../gemini';
import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import type { Content } from '@google/genai';

const createCacheMock = vi.fn();
const deleteCacheMock = vi.fn();
const updateCacheMock = vi.fn();
const countTokensMock = vi.fn();
const generateContentStreamMock = vi.fn();
const { loggerMock } = vi.hoisted(() => ({
  loggerMock: {
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@google/genai', () => ({
  GoogleGenAI: vi.fn().mockImplementation(() => ({
    caches: {
      create: createCacheMock,
      delete: deleteCacheMock,
      update: updateCacheMock,
    },
    models: {
      countTokens: countTokensMock,
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
  getLogger: () => loggerMock,
  Logger: {
    shouldLogLevel: vi.fn(() => true),
  },
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

function createAssistantMessage(text: string): Message {
  return {
    id: `assistant-${text}`,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'assistant',
    content: [{ type: 'text', text }],
  };
}

function createAssistantToolCallMessage(
  name: string,
  args: Record<string, unknown>,
  id = `call-${name}`,
): Message {
  return {
    id: `assistant-tool-${id}`,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'assistant',
    content: [],
    tool_calls: [
      {
        id,
        type: 'function',
        function: {
          name,
          arguments: JSON.stringify(args),
        },
      },
    ],
  };
}

function createToolResponseMessage(
  name: string,
  response: Record<string, unknown>,
  id = `call-${name}`,
): Message {
  return {
    id: `tool-${id}`,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'tool',
    tool_call_id: id,
    content: [{ type: 'text', text: JSON.stringify(response) }],
    tool_calls: [
      {
        id,
        type: 'function',
        function: {
          name,
          arguments: JSON.stringify(response),
        },
      },
    ],
  };
}

function estimateGeminiContentLength(contents: unknown): number {
  if (!Array.isArray(contents)) {
    return 0;
  }

  return contents.reduce((total, content) => {
    if (typeof content !== 'object' || content === null) {
      return total;
    }

    const message = content as Content;
    const parts = Array.isArray(message.parts) ? message.parts : [];
    const partLength = parts.reduce((partTotal, part) => {
      if (typeof part !== 'object' || part === null) {
        return partTotal;
      }

      const candidate = part as Record<string, unknown>;
      const textLength =
        typeof candidate.text === 'string' ? candidate.text.length : 0;
      return partTotal + textLength + JSON.stringify(candidate).length;
    }, 0);

    return total + partLength;
  }, 0);
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
    updateCacheMock.mockResolvedValue(undefined);
    countTokensMock.mockImplementation(async (args: {
      contents?: unknown;
      config?: {
        systemInstruction?: string;
        tools?: Array<{ functionDeclarations: unknown[] }>;
      };
    }) => ({
      totalTokens:
        Math.ceil(
          ((args.config?.systemInstruction?.length ?? 0) +
            estimateGeminiContentLength(args.contents)) /
            4,
        ) +
        Math.ceil(
          JSON.stringify(args.config?.tools ?? []).length / 4,
        ),
    }));
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

  it('uses countTokens to skip cache creation when the exact cacheable prefix is below threshold', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];

    countTokensMock.mockResolvedValueOnce({ totalTokens: 2048 });

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'A'.repeat(131072),
    }));

    expect(countTokensMock).toHaveBeenCalledTimes(1);
    expect(createCacheMock).not.toHaveBeenCalled();

    const firstCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: { cachedContent?: string; systemInstruction?: Array<{ text: string }> };
    };
    expect(firstCall.config?.cachedContent).toBeUndefined();
    expect(firstCall.config?.systemInstruction?.[0]?.text).toContain('AAAA');
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

  it('refreshes Gemini cache TTL when a reused cache entry is close to expiry', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];
    const nowSpy = vi.spyOn(Date, 'now');
    const baseNow = 1_700_000_000_000;
    nowSpy.mockReturnValue(baseNow);
    countTokensMock.mockResolvedValueOnce({ totalTokens: 150000 });

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'A'.repeat(131072),
    }));

    const internalEntries = (
      service as unknown as {
        cachedContextEntries: Map<
          string,
          {
            name: string;
            createdAt: number;
            lastUsedAt: number;
            ttlMs: number;
            expiresAt: number;
            cacheableTokenCount: number;
          }
        >;
      }
    ).cachedContextEntries;
    const firstEntry = [...internalEntries.values()][0];
    firstEntry.expiresAt = baseNow + 5 * 60 * 1000;

    nowSpy.mockReturnValue(baseNow + 2 * 60 * 1000);

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'A'.repeat(131072),
    }));

    expect(updateCacheMock).toHaveBeenCalledWith({
      name: 'cachedContents/1',
      config: { ttl: '10800s' },
    });

    nowSpy.mockRestore();
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

  it('logs prompt drift between consecutive Gemini requests with the first differing message index', async () => {
    const service = new GeminiService('test-key');

    const firstPrepared = service.prepareContextInjection(
      'Stable system prompt',
      '# Current Context Information\nfirst state',
      [createUserMessage('hello')],
    );
    await consumeStream(service.streamChat(firstPrepared.messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: firstPrepared.systemPrompt,
      sessionContext: firstPrepared.sessionContext,
    }));

    loggerMock.debug.mockClear();

    const secondPrepared = service.prepareContextInjection(
      'Stable system prompt',
      '# Current Context Information\nsecond state',
      [createUserMessage('hello')],
    );
    await consumeStream(service.streamChat(secondPrepared.messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: secondPrepared.systemPrompt,
      sessionContext: secondPrepared.sessionContext,
    }));

    expect(loggerMock.debug).toHaveBeenCalledWith(
      'Gemini prompt cache drift',
      expect.objectContaining({
        model: 'gemini-2.5-flash',
        firstDivergenceComponent: 'messages',
        firstDivergenceIndex: 1,
        commonPrefixMessages: 1,
      }),
    );
  });

  it('skips expensive Gemini prompt diagnostics when debug logging is disabled', async () => {
    const service = new GeminiService('test-key');
    const diagnosticsSpy = vi.spyOn(
      service as unknown as { logPromptDiagnostics: (args: unknown) => void },
      'logPromptDiagnostics',
    );
    const shouldLogSpy = vi
      .spyOn(
        service as unknown as { shouldLogPromptDiagnostics: () => boolean },
        'shouldLogPromptDiagnostics',
      )
      .mockReturnValue(false);

    await consumeStream(service.streamChat([createUserMessage('hello')], {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    expect(shouldLogSpy).toHaveBeenCalled();
    expect(diagnosticsSpy).not.toHaveBeenCalled();
  });

  it('creates a history checkpoint cache for long Gemini conversations and sends only the tail in the request', async () => {
    const service = new GeminiService('test-key');
    const messages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(1);

    const cacheCall = createCacheMock.mock.calls[0]?.[0] as {
      config?: { contents?: Content[] };
    };
    const requestCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      contents?: Content[];
      config?: { cachedContent?: string };
    };

    expect(cacheCall.config?.contents).toHaveLength(2);
    expect(requestCall.contents).toHaveLength(3);
    expect(requestCall.config?.cachedContent).toBe('cachedContents/1');
  });

  it('creates a history checkpoint cache without system prompt or tools when cached history is large', async () => {
    const service = new GeminiService('test-key');
    const messages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(1);

    const cacheCall = createCacheMock.mock.calls[0]?.[0] as {
      config?: { contents?: Content[]; systemInstruction?: string };
    };
    const requestCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      contents?: Content[];
      config?: { cachedContent?: string; systemInstruction?: Array<{ text: string }> };
    };

    expect(cacheCall.config?.contents).toHaveLength(2);
    expect(cacheCall.config?.systemInstruction).toBe('');
    expect(requestCall.contents).toHaveLength(3);
    expect(requestCall.config?.cachedContent).toBe('cachedContents/1');
    expect(requestCall.config?.systemInstruction).toBeUndefined();
  });

  it('reuses the same history checkpoint cache when only the uncached tail changes', async () => {
    const service = new GeminiService('test-key');
    const firstMessages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];
    const secondMessages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 changed latest question'),
    ];

    await consumeStream(service.streamChat(firstMessages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    await consumeStream(service.streamChat(secondMessages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
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

  it('keeps the same history checkpoint while recent tail turns grow within the hysteresis window', async () => {
    const service = new GeminiService('test-key');
    const firstMessages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];
    const secondMessages: Message[] = [
      ...firstMessages,
      createAssistantMessage('M3 answer'),
      createUserMessage('U4 follow-up question'),
    ];

    await consumeStream(service.streamChat(firstMessages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    await consumeStream(service.streamChat(secondMessages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(1);

    const firstCacheCall = createCacheMock.mock.calls[0]?.[0] as {
      config?: { contents?: Content[] };
    };
    const firstRequestCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      contents?: Content[];
      config?: { cachedContent?: string };
    };
    const secondRequestCall = generateContentStreamMock.mock.calls[1]?.[0] as {
      contents?: Content[];
      config?: { cachedContent?: string };
    };

    expect(firstCacheCall.config?.contents).toHaveLength(2);
    expect(firstRequestCall.contents).toHaveLength(3);
    expect(secondRequestCall.contents).toHaveLength(5);
    expect(firstRequestCall.config?.cachedContent).toBe('cachedContents/1');
    expect(secondRequestCall.config?.cachedContent).toBe('cachedContents/1');
  });

  it('advances the history checkpoint only after the hysteresis interval is exceeded', async () => {
    const service = new GeminiService('test-key');
    const firstMessages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];
    const secondMessages: Message[] = [
      ...firstMessages,
      createAssistantMessage('M3 answer'),
      createUserMessage('U4 follow-up question'),
      createAssistantMessage('M4 answer'),
      createUserMessage('U5 another follow-up'),
      createAssistantMessage('M5 answer'),
      createUserMessage('U6 newest question'),
    ];

    await consumeStream(service.streamChat(firstMessages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    await consumeStream(service.streamChat(secondMessages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(2);

    const firstCacheCall = createCacheMock.mock.calls[0]?.[0] as {
      config?: { contents?: Content[] };
    };
    const secondCacheCall = createCacheMock.mock.calls[1]?.[0] as {
      config?: { contents?: Content[] };
    };
    const secondRequestCall = generateContentStreamMock.mock.calls[1]?.[0] as {
      contents?: Content[];
      config?: { cachedContent?: string };
    };

    expect(firstCacheCall.config?.contents).toHaveLength(2);
    expect(secondCacheCall.config?.contents).toHaveLength(6);
    expect(secondRequestCall.contents).toHaveLength(5);
    expect(secondRequestCall.config?.cachedContent).toBe('cachedContents/2');
  });

  it('disables history checkpoint caching when Gemini tool call turns are present', async () => {
    const service = new GeminiService('test-key');
    const messages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 run the tool'),
      createAssistantToolCallMessage('workspace__readFile', {
        path: 'src/main.ts',
      }),
      createToolResponseMessage('workspace__readFile', {
        content: 'file contents',
      }),
      createUserMessage('U3 latest question'),
    ];

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    expect(createCacheMock).not.toHaveBeenCalled();

    const requestCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      contents?: Content[];
      config?: { cachedContent?: string };
    };

    expect(requestCall.config?.cachedContent).toBeUndefined();

    const requestContents = requestCall.contents ?? [];
    const functionCallIndex = requestContents.findIndex((content) =>
      content.parts?.some((part) => 'functionCall' in part),
    );
    expect(functionCallIndex).toBeGreaterThanOrEqual(0);
    expect(requestContents[functionCallIndex]?.role).toBe('model');
    expect(
      requestContents[functionCallIndex + 1]?.parts?.[0],
    ).toHaveProperty('functionResponse');
    expect(requestContents[functionCallIndex + 1]?.role).toBe('user');
    expect(requestContents[requestContents.length - 1]?.role).toBe('user');
  });

  it('falls back to history-aware cache estimation when Gemini countTokens fails', async () => {
    const service = new GeminiService('test-key');
    const messages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];

    countTokensMock.mockRejectedValueOnce(new Error('countTokens unavailable'));

    await consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
    }));

    expect(createCacheMock).toHaveBeenCalledTimes(1);
    expect(loggerMock.warn).toHaveBeenCalledWith(
      'Gemini countTokens failed during cache eligibility check; falling back to character estimate.',
      expect.objectContaining({
        cacheableContentCount: 2,
        cacheableTokenEstimate: expect.any(Number),
      }),
    );
  });

  it('preserves nested required fields when converting Gemini tool schemas', () => {
    const service = new GeminiService('test-key');
    const tools = service.convertTools([
      {
        name: 'workspace__writeFile',
        description: 'Writes a file',
        inputSchema: {
          type: 'object',
          properties: {
            payload: {
              type: 'object',
              properties: {
                path: { type: 'string', description: 'File path' },
                content: { type: 'string', description: 'File content' },
              },
              required: ['path', 'content'],
            },
          },
          required: ['payload'],
        },
      },
    ]);

    expect(tools[0]).toMatchObject({
      parameters: {
        type: Type.OBJECT,
        required: ['payload'],
        properties: {
          payload: {
            type: Type.OBJECT,
            required: ['path', 'content'],
          },
        },
      },
    });
  });

  it('deduplicates concurrent Gemini cache creation for the same cache key', async () => {
    let resolveCreate: ((value: { name: string }) => void) | undefined;
    createCacheMock.mockImplementationOnce(
      () =>
        new Promise<{ name: string }>((resolve) => {
          resolveCreate = resolve;
        }),
    );

    const service = new GeminiService('test-key');
    const messages: Message[] = [
      createUserMessage('U1 ' + 'A'.repeat(90000)),
      createAssistantMessage('M1 ' + 'B'.repeat(90000)),
      createUserMessage('U2 ' + 'C'.repeat(90000)),
      createAssistantMessage('M2 ' + 'D'.repeat(90000)),
      createUserMessage('U3 latest question'),
    ];

    const firstRun = consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));
    const secondRun = consumeStream(service.streamChat(messages, {
      modelName: 'gemini-2.5-flash',
      systemPrompt: 'Stable system prompt',
    }));

    await vi.waitFor(() => {
      expect(createCacheMock).toHaveBeenCalledTimes(1);
    });

    resolveCreate?.({ name: 'cachedContents/shared' });

    await Promise.all([firstRun, secondRun]);

    expect(createCacheMock).toHaveBeenCalledTimes(1);
    expect(generateContentStreamMock).toHaveBeenCalledTimes(2);

    const firstCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: { cachedContent?: string };
    };
    const secondCall = generateContentStreamMock.mock.calls[1]?.[0] as {
      config?: { cachedContent?: string };
    };

    expect(firstCall.config?.cachedContent).toBe('cachedContents/shared');
    expect(secondCall.config?.cachedContent).toBe('cachedContents/shared');
  });

  it('keeps local cache entry when Gemini cache deletion fails', async () => {
    const service = new GeminiService('test-key');
    const cacheKey = 'gemini:test-cache-key';
    const cacheEntries = (
      service as unknown as {
        cachedContextEntries: Map<
          string,
          {
            name: string;
            createdAt: number;
            lastUsedAt: number;
            ttlMs: number;
            expiresAt: number;
            cacheableTokenCount: number;
          }
        >;
      }
    ).cachedContextEntries;

    cacheEntries.set(cacheKey, {
      name: 'cachedContents/failed-delete',
      createdAt: 1,
      lastUsedAt: 2,
      ttlMs: 1000,
      expiresAt: 2000,
      cacheableTokenCount: 40000,
    });
    deleteCacheMock.mockRejectedValueOnce(new Error('delete failed'));

    const removed = await (
      service as unknown as {
        removeContextCacheEntry: (key: string, reason: string) => Promise<boolean>;
      }
    ).removeContextCacheEntry(cacheKey, 'test cleanup');

    expect(removed).toBe(false);
    expect(cacheEntries.has(cacheKey)).toBe(true);
    expect(cacheEntries.get(cacheKey)?.expiresAt).toBe(0);
  });

  it('prefers evicting caches from the same scope before evicting another scope', async () => {
    const service = new GeminiService('test-key');
    const cacheEntries = (
      service as unknown as {
        cachedContextEntries: Map<
          string,
          {
            name: string;
            scope: string;
            createdAt: number;
            lastUsedAt: number;
            ttlMs: number;
            expiresAt: number;
            cacheableTokenCount: number;
          }
        >;
        evictContextCacheOverflow: (
          preferredScope?: string,
          protectedCacheKey?: string,
        ) => Promise<void>;
      }
    ).cachedContextEntries;

    for (let index = 0; index < 7; index += 1) {
      cacheEntries.set(`scope-a-${index}`, {
        name: `cachedContents/scope-a-${index}`,
        scope: 'session-a:thread-a',
        createdAt: index,
        lastUsedAt: index,
        ttlMs: 1000,
        expiresAt: 10_000 + index,
        cacheableTokenCount: 50_000,
      });
    }

    cacheEntries.set('scope-b-0', {
      name: 'cachedContents/scope-b-0',
      scope: 'session-b:thread-b',
      createdAt: 99,
      lastUsedAt: 99,
      ttlMs: 1000,
      expiresAt: 20_000,
      cacheableTokenCount: 50_000,
    });
    cacheEntries.set('scope-a-new', {
      name: 'cachedContents/scope-a-new',
      scope: 'session-a:thread-a',
      createdAt: 100,
      lastUsedAt: 100,
      ttlMs: 1000,
      expiresAt: 20_001,
      cacheableTokenCount: 50_000,
    });

    await (
      service as unknown as {
        evictContextCacheOverflow: (
          preferredScope?: string,
          protectedCacheKey?: string,
        ) => Promise<void>;
      }
    ).evictContextCacheOverflow('session-a:thread-a', 'scope-a-new');

    expect(deleteCacheMock).toHaveBeenCalledWith({
      name: 'cachedContents/scope-a-0',
    });
    expect(cacheEntries.has('scope-a-0')).toBe(false);
    expect(cacheEntries.has('scope-b-0')).toBe(true);
    expect(cacheEntries.has('scope-a-new')).toBe(true);
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
