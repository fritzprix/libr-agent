import { describe, expect, it, vi } from 'vitest';
import type { Message } from '@/models/chat';
import { convertToOpenAIMessages } from '../openai/message-converter';
import type { MCPContent } from '@/lib/mcp';

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('convertToOpenAIMessages', () => {
  const dummyArgs = {
    logger: {
      debug: vi.fn(),
      info: vi.fn(),
      warn: vi.fn(),
      error: vi.fn(),
    },
    processMessageContent: (content: MCPContent[]) => {
      const first = content[0];
      return first && 'text' in first ? first.text || '' : '';
    },
    processMultiModalContent: () => [],
    extractMediaContent: () => [],
  };

  it('should unconditionally map assistant message thinking content to reasoning_content in OpenAI payload', () => {
    const messages: Message[] = [
      {
        id: 'msg-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hello, I am Qwen!' }],
        thinking: 'I need to greet the user politely.',
        createdAt: new Date(),
      },
    ];

    const result = convertToOpenAIMessages({
      messages,
      ...dummyArgs,
    });

    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({
      role: 'assistant',
      content: 'Hello, I am Qwen!',
    });
    expect(result[0]).toHaveProperty('reasoning_content', 'I need to greet the user politely.');
  });

  it('should not inject reasoning_content if thinking is undefined', () => {
    const messages: Message[] = [
      {
        id: 'msg-2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hello!' }],
        createdAt: new Date(),
      },
    ];

    const result = convertToOpenAIMessages({
      messages,
      ...dummyArgs,
    });

    expect(result).toHaveLength(1);
    expect(result[0]).not.toHaveProperty('reasoning_content');
  });

  it('should include both reasoning_content and tool_calls on assistant message', () => {
    const messages: Message[] = [
      {
        id: 'msg-3',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Let me check the files.' }],
        thinking: 'I need to check the files first.',
        tool_calls: [
          {
            id: 'tc1',
            type: 'function',
            function: {
              name: 'read_file',
              arguments: '{"path": "test.txt"}',
            },
          },
        ],
        createdAt: new Date(),
      },
    ];

    const result = convertToOpenAIMessages({
      messages,
      ...dummyArgs,
    });

    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({
      role: 'assistant',
      content: 'Let me check the files.',
      tool_calls: [
        {
          id: 'tc1',
          type: 'function',
          function: {
            name: 'read_file',
            arguments: '{"path": "test.txt"}',
          },
        },
      ],
      reasoning_content: 'I need to check the files first.',
    });
  });
});
