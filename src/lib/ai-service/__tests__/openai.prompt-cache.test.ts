import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';

const createMock = vi.fn();
const loggerMock = {
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
};

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
  getLogger: () => loggerMock,
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

const alphaTool: MCPTool = {
  name: 'alpha',
  description: 'Alpha tool',
  inputSchema: {
    type: 'object',
    properties: {
      zeta: { type: 'string' },
      alpha: { type: 'string' },
    },
    required: ['alpha'],
  },
};

const betaTool: MCPTool = {
  name: 'beta',
  description: 'Beta tool',
  inputSchema: {
    type: 'object',
    properties: {
      beta: { type: 'number' },
    },
  },
};

describe('OpenAIService prompt cache extensions', () => {
  beforeEach(() => {
    createMock.mockReset();
    loggerMock.debug.mockReset();
    loggerMock.info.mockReset();
    loggerMock.warn.mockReset();
    loggerMock.error.mockReset();
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
    expect(request.prompt_cache_key).toBeUndefined();
    expect(request.stream).toBe(true);
    expect(request).not.toHaveProperty('extra_body');
  });

  it('uses official OpenAI prompt cache routing fields instead of cache_prompt for the default endpoint', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    const chunks: string[] = [];
    for await (const chunk of service.streamChat([message], {
      modelName: 'gpt-4o',
      systemPrompt: 'Stable instructions',
    })) {
      chunks.push(chunk);
    }

    expect(chunks.length).toBeGreaterThan(0);

    const [request] = createMock.mock.calls[0] as [Record<string, unknown>];
    expect(request.cache_prompt).toBeUndefined();
    expect(request.prompt_cache_key).toMatch(
      /^chat:gpt-4o:[a-f0-9]+:[a-f0-9]+$/,
    );
  });

  it('emits prompt and fetch diagnostics with a request id header for streaming requests', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    for await (const chunk of service.streamChat([message], {
      modelName: 'gpt-4o',
      systemPrompt: 'Stable instructions',
      availableTools: [alphaTool, betaTool],
    })) {
      void chunk;
      break;
    }

    expect(loggerMock.debug).toHaveBeenCalledWith(
      'OpenAI prompt diagnostics',
      expect.objectContaining({
        mode: 'stream',
        model: 'gpt-4o',
      }),
    );
    expect(loggerMock.debug).toHaveBeenCalledWith(
      'OpenAI fetch diagnostics',
      expect.objectContaining({
        mode: 'stream',
        model: 'gpt-4o',
        requestId: expect.stringMatching(/^req_/),
      }),
    );

    const options = createMock.mock.calls[0]?.[1] as
      | { headers?: Record<string, string> }
      | undefined;
    expect(options?.headers?.['x-libragent-request-id']).toMatch(/^req_/);
  });

  it('logs prompt drift between consecutive requests with the first differing message index', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    const firstPrepared = service.prepareContextInjection(
      'Stable instructions',
      '# Current Context Information\nfirst state',
      [message],
    );
    for await (const chunk of service.streamChat(firstPrepared.messages, {
      modelName: 'gpt-4o',
      systemPrompt: firstPrepared.systemPrompt,
      sessionContext: firstPrepared.sessionContext,
      availableTools: [alphaTool, betaTool],
    })) {
      void chunk;
      break;
    }

    loggerMock.debug.mockClear();
    loggerMock.info.mockClear();

    const secondPrepared = service.prepareContextInjection(
      'Stable instructions',
      '# Current Context Information\nsecond state',
      [message],
    );
    for await (const chunk of service.streamChat(secondPrepared.messages, {
      modelName: 'gpt-4o',
      systemPrompt: secondPrepared.systemPrompt,
      sessionContext: secondPrepared.sessionContext,
      availableTools: [alphaTool, betaTool],
    })) {
      void chunk;
      break;
    }

    expect(loggerMock.debug).toHaveBeenCalledWith(
      'OpenAI prompt cache drift',
      expect.objectContaining({
        mode: 'stream',
        model: 'gpt-4o',
        firstDivergenceComponent: 'messages',
        firstDivergenceIndex: 2,
        commonPrefixMessages: 2,
      }),
    );
  });

  it('derives the same prompt_cache_key regardless of available tool order', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    for await (const chunk of service.streamChat([message], {
      modelName: 'gpt-4o',
      systemPrompt: 'Stable instructions',
      availableTools: [betaTool, alphaTool],
    })) {
      void chunk;
      break;
    }

    const [firstRequest] = createMock.mock.calls[0] as [Record<string, unknown>];

    createMock.mockClear();

    for await (const chunk of service.streamChat([message], {
      modelName: 'gpt-4o',
      systemPrompt: 'Stable instructions',
      availableTools: [alphaTool, betaTool],
    })) {
      void chunk;
      break;
    }

    const [secondRequest] = createMock.mock.calls[0] as [
      Record<string, unknown>,
    ];

    expect(firstRequest.prompt_cache_key).toBe(secondRequest.prompt_cache_key);
  });

  it('derives the same prompt_cache_key across sessions with the same stable prefix', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    const firstSessionMessage: Message = {
      ...message,
      id: 'm-session-1',
      sessionId: 'session-1',
      threadId: 'thread-1',
    };

    const secondSessionMessage: Message = {
      ...message,
      id: 'm-session-2',
      sessionId: 'session-2',
      threadId: 'thread-2',
    };

    for await (const chunk of service.streamChat([firstSessionMessage], {
      modelName: 'gpt-4o',
      systemPrompt: 'Stable instructions',
      availableTools: [alphaTool, betaTool],
    })) {
      void chunk;
      break;
    }

    const [firstRequest] = createMock.mock.calls[0] as [Record<string, unknown>];

    createMock.mockClear();

    for await (const chunk of service.streamChat([secondSessionMessage], {
      modelName: 'gpt-4o',
      systemPrompt: 'Stable instructions',
      availableTools: [alphaTool, betaTool],
    })) {
      void chunk;
      break;
    }

    const [secondRequest] = createMock.mock.calls[0] as [
      Record<string, unknown>,
    ];

    expect(firstRequest.prompt_cache_key).toBe(secondRequest.prompt_cache_key);
  });

  it('does not send cache_prompt to the default OpenAI endpoint even when explicitly enabled', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test', {
      enablePromptCache: true,
    });

    await service.sampleText('hello', { modelName: 'gpt-4o' });

    const [request] = createMock.mock.calls[0] as [Record<string, unknown>];
    expect(request.cache_prompt).toBeUndefined();
  });

  it('supports explicit prompt cache enablement for non-streaming compatible endpoints', async () => {
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

  it('forwards official OpenAI prompt cache parameters for non-streaming requests', async () => {
    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test', {
      promptCacheKey: 'project:shared-prefix',
      promptCacheRetention: '24h',
    });

    await service.sampleText('hello', { modelName: 'gpt-4o' });

    const [request] = createMock.mock.calls[0] as [Record<string, unknown>];
    expect(request.cache_prompt).toBeUndefined();
    expect(request.prompt_cache_key).toBe('project:shared-prefix');
    expect(request.prompt_cache_retention).toBe('24h');
  });

  it('streams tool call deltas through without waiting for completion', async () => {
    createMock.mockImplementation((request: { stream?: boolean }) => {
      if (request.stream) {
        return Promise.resolve(
          (async function* () {
            yield {
              choices: [
                {
                  delta: {
                    tool_calls: [
                      {
                        index: 0,
                        id: 'call_123',
                        type: 'function',
                        function: {
                          name: 'workspace__writeFile',
                          arguments: '{"path":"foo.txt"',
                        },
                      },
                    ],
                  },
                },
              ],
            };
            yield {
              choices: [
                {
                  delta: {
                    tool_calls: [
                      {
                        index: 0,
                        function: {
                          arguments: ',"content":"hello"}',
                        },
                      },
                    ],
                  },
                },
              ],
            };
          })(),
        );
      }

      return Promise.resolve({
        choices: [{ message: { content: 'ok' }, finish_reason: 'stop' }],
        usage: {
          prompt_tokens: 10,
          completion_tokens: 2,
          total_tokens: 12,
        },
        model: 'gpt-4o',
      });
    });

    const { OpenAIService } = await import('../openai');
    const service = new OpenAIService('sk-test');

    const stream = service.streamChat([message], {
      modelName: 'gpt-4o',
    });

    const observedChunks: Array<Record<string, unknown>> = [];
    for await (const chunk of stream) {
      observedChunks.push(JSON.parse(chunk) as Record<string, unknown>);
    }

    const toolCallChunks = observedChunks.filter(
      (chunk) =>
        Array.isArray((chunk as { tool_calls?: unknown[] }).tool_calls) &&
        ((chunk as { tool_calls?: unknown[] }).tool_calls?.length ?? 0) > 0,
    ) as Array<{
      tool_calls?: Array<{
        index?: number;
        id?: string;
        type?: string;
        function?: { name?: string; arguments?: string };
      }>;
    }>;

    expect(toolCallChunks).toHaveLength(2);

    expect(toolCallChunks[0]?.tool_calls?.[0]).toEqual({
      index: 0,
      id: 'call_123',
      type: 'function',
      function: {
        name: 'workspace__writeFile',
        arguments: '{"path":"foo.txt"',
      },
    });
    expect(toolCallChunks[1]?.tool_calls?.[0]).toEqual({
      index: 0,
      function: {
        arguments: ',"content":"hello"}',
      },
    });
  });
});
