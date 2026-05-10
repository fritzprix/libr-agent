import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';

const chatMock = vi.fn();
const listMock = vi.fn();

vi.mock('ollama/browser', () => ({
  Ollama: vi.fn().mockImplementation(() => ({
    chat: chatMock,
    list: listMock,
    abort: vi.fn(),
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

vi.mock('../model-capabilities', () => ({
  supportsThinking: vi.fn().mockResolvedValue(false),
  getContextWindow: vi.fn().mockResolvedValue(128000),
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
    createdAt: new Date('2026-04-04T10:00:00.000Z'),
  };
}

const alphaTool: MCPTool = {
  name: 'alpha_tool',
  description: 'Alpha tool',
  inputSchema: {
    type: 'object',
    properties: {
      alpha: { type: 'string' },
    },
    required: ['alpha'],
  },
};

const betaTool: MCPTool = {
  name: 'beta_tool',
  description: 'Beta tool',
  inputSchema: {
    type: 'object',
    properties: {
      beta: { type: 'number' },
    },
    required: ['beta'],
  },
};

describe('OllamaService prompt layout', () => {
  beforeEach(() => {
    chatMock.mockReset();
    listMock.mockReset();
    chatMock.mockResolvedValue(createEmptyStream());
  });

  it('moves volatile session context into a synthetic tail message for Ollama', async () => {
    const { OllamaService } = await import('../ollama');
    const service = new OllamaService('ollama-local');

    const prepared = service.prepareContextInjection(
      'Stable system prompt',
      '# Current Context Information\nvolatile bits',
      [createUserMessage('hello')],
    );

    expect(prepared.systemPrompt).toBe('Stable system prompt');
    expect(prepared.sessionContext).toBeUndefined();
    expect(prepared.messages).toHaveLength(2);
    expect(prepared.messages[1]).toMatchObject({
      id: 'ollama-session-context-msg-hello',
      sessionId: 'session-1',
      threadId: 'thread-1',
      role: 'user',
      content: [
        {
          type: 'text',
          text: '[Current session context — background reference only, do not respond to this block]\n\n# Current Context Information\nvolatile bits\n\n[End of session context]',
        },
      ],
    });
  });

  it('keeps stable system prompt and normalized tool order across Ollama turns', async () => {
    const { OllamaService } = await import('../ollama');
    const service = new OllamaService('ollama-local');

    const prepared = service.prepareContextInjection(
      'Stable system prompt',
      '# Current Context Information\nvolatile bits',
      [createUserMessage('hello')],
    );

    await consumeStream(
      service.streamChat(prepared.messages, {
        modelName: 'llama3.1',
        systemPrompt: prepared.systemPrompt,
        sessionContext: prepared.sessionContext,
        availableTools: [betaTool, alphaTool],
      }),
    );

    const request = chatMock.mock.calls[0]?.[0] as {
      messages?: Array<{ role?: string; content?: string }>;
      tools?: Array<{ function?: { name?: string } }>;
      keep_alive?: string;
    };

    expect(request.keep_alive).toBe('5m');
    expect(request.messages).toEqual([
      { role: 'system', content: 'Stable system prompt' },
      { role: 'user', content: 'hello' },
      {
        role: 'user',
        content:
          '[Current session context — background reference only, do not respond to this block]\n\n# Current Context Information\nvolatile bits\n\n[End of session context]',
      },
    ]);
    expect(request.tools?.map((tool) => tool.function?.name)).toEqual([
      'alpha_tool',
      'beta_tool',
    ]);
  });

  it('omits tools from Ollama requests when tool use is disabled', async () => {
    const { OllamaService } = await import('../ollama');
    const service = new OllamaService('ollama-local');

    const prepared = service.prepareContextInjection(
      'Stable system prompt',
      '# Current Context Information\nvolatile bits',
      [createUserMessage('summarize this')],
    );

    await consumeStream(
      service.streamChat(prepared.messages, {
        modelName: 'llama3.1',
        systemPrompt: prepared.systemPrompt,
        sessionContext: prepared.sessionContext,
        availableTools: [betaTool, alphaTool],
        disableToolUse: true,
      }),
    );

    const request = chatMock.mock.calls[0]?.[0] as {
      tools?: Array<{ function?: { name?: string } }>;
    };

    expect(request.tools).toBeUndefined();
  });

  it('removes the abort listener after stream cleanup', async () => {
    const { OllamaService } = await import('../ollama');
    const service = new OllamaService('ollama-local');
    const controller = new AbortController();
    const addEventListenerSpy = vi.spyOn(controller.signal, 'addEventListener');
    const removeEventListenerSpy = vi.spyOn(
      controller.signal,
      'removeEventListener',
    );

    const abortSpy = vi.fn();
    chatMock.mockResolvedValue({
      abort: abortSpy,
      [Symbol.asyncIterator]() {
        return {
          next: async () => ({ done: true, value: undefined as never }),
        };
      },
    });

    await consumeStream(
      service.streamChat([createUserMessage('hello')], {
        modelName: 'llama3.1',
        signal: controller.signal,
      }),
    );

    expect(addEventListenerSpy).toHaveBeenCalledWith(
      'abort',
      expect.any(Function),
      { once: true },
    );
    expect(removeEventListenerSpy).toHaveBeenCalledWith(
      'abort',
      expect.any(Function),
    );
    expect(abortSpy).not.toHaveBeenCalled();
  });
});
