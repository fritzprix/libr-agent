import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';

import { AIServiceFactory } from '../factory';
import { GeminiService } from '../gemini';

const generateContentStreamMock = vi.fn();

vi.mock('@google/genai', () => ({
  GoogleGenAI: vi.fn().mockImplementation(() => ({
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

async function consumeStream(
  stream: AsyncGenerator<string, void, void>,
): Promise<void> {
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

function createSessionContextMessage(text: string): Message {
  return {
    id: 'gemini-session-context-msg-hello',
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'user',
    source: 'session-context',
    content: [{ type: 'text', text }],
  };
}

function createAssistantMessage(id: string, text: string): Message {
  return {
    id,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'assistant',
    content: [{ type: 'text', text }],
  };
}

function createAssistantToolCallMessage(
  id: string,
  toolCallId: string,
  toolName: string,
  args: Record<string, unknown>,
): Message {
  return {
    id,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'assistant',
    content: [],
    tool_calls: [
      {
        id: toolCallId,
        type: 'function',
        function: {
          name: toolName,
          arguments: JSON.stringify(args),
        },
      },
    ],
  };
}

function createToolResultMessage(
  id: string,
  toolCallId: string,
  text: string,
): Message {
  return {
    id,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'tool',
    tool_call_id: toolCallId,
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

describe('GeminiService request assembly', () => {
  beforeEach(() => {
    AIServiceFactory.disposeAll();
    vi.clearAllMocks();
    generateContentStreamMock.mockResolvedValue(createEmptyStream());
  });

  it('sends the full conversation with the stable system prompt for implicit caching', async () => {
    const service = new GeminiService('test-key');
    const messages = [
      createUserMessage('history question'),
      createAssistantMessage('msg-history-assistant', 'A'.repeat(10000)),
      {
        ...createUserMessage('current question'),
        id: 'msg-current-user',
      },
    ];

    await consumeStream(
      service.streamChat(messages, {
        modelName: 'gemini-2.5-flash',
        systemPrompt: 'Stable system prompt',
      }),
    );

    const streamCall = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: {
        cachedContent?: string;
        systemInstruction?: Array<{ text: string }>;
      };
      contents?: Array<{ role?: string; parts?: Array<{ text?: string }> }>;
    };

    expect(streamCall.config?.cachedContent).toBeUndefined();
    expect(streamCall.config?.systemInstruction).toEqual([
      { text: 'Stable system prompt' },
    ]);
    expect(streamCall.contents).toEqual([
      { role: 'user', parts: [{ text: 'history question' }] },
      { role: 'model', parts: [{ text: 'A'.repeat(10000) }] },
      { role: 'user', parts: [{ text: 'current question' }] },
    ]);
  });

  it('keeps tool declarations visible when tool usage is disabled', async () => {
    const service = new GeminiService('test-key');
    const messages = [createUserMessage('hello')];
    const tool = createTool('short tool description');

    await consumeStream(
      service.streamChat(messages, {
        modelName: 'gemini-2.5-flash',
        systemPrompt: 'stable prompt',
        availableTools: [tool],
        disableToolUse: true,
      }),
    );

    const request = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: {
        cachedContent?: string;
        tools?: unknown[];
        toolConfig?: { functionCallingConfig?: { mode?: string } };
        systemInstruction?: Array<{ text: string }>;
      };
    };

    expect(request.config?.cachedContent).toBeUndefined();
    expect(request.config?.tools).toBeDefined();
    expect(request.config?.systemInstruction).toEqual([
      { text: 'stable prompt' },
    ]);
    expect(request.config?.toolConfig?.functionCallingConfig?.mode).toBe(
      'NONE',
    );
  });

  it('keeps forceToolUse compatible with implicit-caching request shaping', async () => {
    const service = new GeminiService('test-key');

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'gemini-2.5-flash',
        systemPrompt: 'stable prompt',
        availableTools: [createTool('tool description')],
        forceToolUse: true,
      }),
    );

    const request = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: {
        cachedContent?: string;
        tools?: unknown[];
        toolConfig?: { functionCallingConfig?: { mode?: string } };
      };
    };

    expect(request.config?.cachedContent).toBeUndefined();
    expect(request.config?.tools).toBeDefined();
    expect(request.config?.toolConfig?.functionCallingConfig?.mode).toBe(
      'ANY',
    );
  });

  it('keeps Gemini session context as a synthetic tail message in the provider request', async () => {
    const service = new GeminiService('test-key');

    await consumeStream(
      service.streamChat(
        [
          createUserMessage('hello'),
          createSessionContextMessage(
            `[Current session context — background reference only, do not respond to this block]\n\n# Current Context Information\nvolatile bits\n\n[End of session context]`,
          ),
        ],
        {
          modelName: 'gemini-2.5-flash',
          systemPrompt: 'Stable system prompt',
        },
      ),
    );

    const request = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: {
        cachedContent?: string;
        systemInstruction?: Array<{ text: string }>;
      };
      contents?: Array<{ role?: string; parts?: Array<{ text?: string }> }>;
    };

    expect(request.config?.cachedContent).toBeUndefined();
    expect(request.config?.systemInstruction).toEqual([
      { text: 'Stable system prompt' },
    ]);
    expect(request.contents).toHaveLength(2);
    expect(request.contents?.[0]).toMatchObject({
      role: 'user',
      parts: [{ text: 'hello' }],
    });
    expect(request.contents?.[1]).toMatchObject({
      role: 'user',
      parts: [
        {
          text: '[Current session context — background reference only, do not respond to this block]\n\n# Current Context Information\nvolatile bits\n\n[End of session context]',
        },
      ],
    });
  });

  it('keeps tool-loop turns in the same request body without explicit cachedContent', async () => {
    const service = new GeminiService('test-key');
    const messages = [
      createUserMessage('history question'),
      createAssistantMessage('msg-history-assistant', 'B'.repeat(10000)),
      {
        ...createUserMessage('current question'),
        id: 'msg-current-user',
      },
      createAssistantToolCallMessage(
        'msg-tool-call',
        'call_1',
        'workspace__readFile',
        { path: 'README.md' },
      ),
      createToolResultMessage('msg-tool-result', 'call_1', 'file contents'),
    ];

    await consumeStream(
      service.streamChat(messages, {
        modelName: 'gemini-2.5-flash',
        systemPrompt: 'Stable system prompt',
      }),
    );

    const request = generateContentStreamMock.mock.calls[0]?.[0] as {
      config?: {
        cachedContent?: string;
        systemInstruction?: Array<{ text: string }>;
      };
      contents?: Array<{
        role?: string;
        parts?: Array<{
          text?: string;
          functionCall?: { name?: string };
          functionResponse?: { name?: string };
        }>;
      }>;
    };

    expect(request.config?.cachedContent).toBeUndefined();
    expect(request.config?.systemInstruction).toEqual([
      { text: 'Stable system prompt' },
    ]);
    expect(request.contents?.[0]).toMatchObject({
      role: 'user',
      parts: [{ text: 'history question' }],
    });
    expect(request.contents?.[2]).toMatchObject({
      role: 'user',
      parts: [{ text: 'current question' }],
    });
    expect(request.contents?.[3]).toMatchObject({
      role: 'model',
      parts: [{ functionCall: { name: 'workspace__readFile' } }],
    });
    expect(request.contents?.[4]).toMatchObject({
      role: 'user',
      parts: [{ functionResponse: { name: 'workspace__readFile' } }],
    });
  });

  it('emits full Gemini tool calls as soon as the stream includes a functionCall part', async () => {
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

    const toolCallChunks = observedChunks.filter(
      (chunk) =>
        Array.isArray((chunk as { tool_calls?: unknown[] }).tool_calls) &&
        ((chunk as { tool_calls?: unknown[] }).tool_calls?.length ?? 0) > 0,
    ) as Array<{
      tool_calls?: Array<{
        id?: string;
        type?: string;
        function?: { name?: string; arguments?: string };
      }>;
    }>;
    const signatureChunk = observedChunks.find(
      (chunk) =>
        typeof (chunk as { thinkingSignature?: unknown }).thinkingSignature ===
        'string',
    ) as { thinkingSignature?: string } | undefined;

    expect(
      observedChunks.some((chunk) =>
        Array.isArray((chunk as { tool_call_starts?: unknown[] }).tool_call_starts),
      ),
    ).toBe(false);
    expect(toolCallChunks).toHaveLength(1);
    expect(signatureChunk).toBeDefined();

    expect(toolCallChunks[0]?.tool_calls?.[0]).toEqual({
      id: 'call_gemini_1',
      type: 'function',
      function: {
        name: 'workspace__writeFile',
        arguments: JSON.stringify({ path: 'foo.txt', content: 'hello' }),
      },
    });
    expect(signatureChunk?.thinkingSignature).toBe('sig_123');
  });

  it('emits parallel Gemini tool calls as full snapshots in one chunk', async () => {
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
                              id: 'call_gemini_a',
                              name: 'workspace__readFile',
                              args: { path: 'a.txt' },
                            },
                            thoughtSignature: 'sig_parallel',
                          },
                          {
                            functionCall: {
                              id: 'call_gemini_b',
                              name: 'workspace__listDirectory',
                              args: { path: 'src' },
                            },
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
    const observedChunks: Array<Record<string, unknown>> = [];
    for await (const chunk of service.streamChat([createUserMessage('inspect')], {
      modelName: 'gemini-2.5-flash',
    })) {
      observedChunks.push(JSON.parse(chunk) as Record<string, unknown>);
    }

    const toolCallChunk = observedChunks.find(
      (chunk) =>
        Array.isArray((chunk as { tool_calls?: unknown[] }).tool_calls) &&
        ((chunk as { tool_calls?: unknown[] }).tool_calls?.length ?? 0) === 2,
    ) as
      | {
          tool_calls?: Array<{
            id?: string;
            type?: string;
            function?: { name?: string; arguments?: string };
          }>;
        }
      | undefined;

    expect(
      observedChunks.some((chunk) =>
        Array.isArray((chunk as { tool_call_starts?: unknown[] }).tool_call_starts),
      ),
    ).toBe(false);
    expect(toolCallChunk?.tool_calls).toEqual([
      {
        id: 'call_gemini_a',
        type: 'function',
        function: {
          name: 'workspace__readFile',
          arguments: JSON.stringify({ path: 'a.txt' }),
        },
      },
      {
        id: 'call_gemini_b',
        type: 'function',
        function: {
          name: 'workspace__listDirectory',
          arguments: JSON.stringify({ path: 'src' }),
        },
      },
    ]);
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
