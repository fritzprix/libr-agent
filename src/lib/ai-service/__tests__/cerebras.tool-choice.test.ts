import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';

const createMock = vi.fn();

vi.mock('@cerebras/cerebras_cloud_sdk', () => ({
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

const sampleTool: MCPTool = {
  name: 'workspace__writeFile',
  description: 'Write a file',
  inputSchema: {
    type: 'object',
    properties: {},
  },
};

describe('CerebrasService tool_choice mapping', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createMock.mockResolvedValue(createEmptyStream());
  });

  it('preserves tools but sets tool_choice to none when disableToolUse is true', async () => {
    const { CerebrasService } = await import('../cerebras');
    const service = new CerebrasService('test-key');

    for await (const chunk of service.streamChat(
      [createUserMessage('summarize')],
      {
        modelName: 'llama3.1-8b',
        availableTools: [sampleTool],
        disableToolUse: true,
      },
    )) {
      void chunk;
    }

    const request = createMock.mock.calls[0]?.[0] as {
      tools?: Array<{ function?: { name?: string } }>;
      tool_choice?: string;
    };

    expect(request.tool_choice).toBe('none');
    expect(request.tools?.map((tool) => tool.function?.name)).toEqual([
      'workspace__writeFile',
    ]);
  });

  it('sets tool_choice to required when forceToolUse is true', async () => {
    const { CerebrasService } = await import('../cerebras');
    const service = new CerebrasService('test-key');

    for await (const chunk of service.streamChat(
      [createUserMessage('use a tool')],
      {
        modelName: 'llama3.1-8b',
        availableTools: [sampleTool],
        forceToolUse: true,
      },
    )) {
      void chunk;
    }

    const request = createMock.mock.calls[0]?.[0] as {
      tool_choice?: string;
    };

    expect(request.tool_choice).toBe('required');
  });
});
