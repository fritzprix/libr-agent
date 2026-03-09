import { describe, it, expect } from 'vitest';
import { MessageNormalizer } from '../message-normalizer';
import { AIServiceProvider } from '../types';
import type { Message } from '@/models/chat';

describe('MessageNormalizer - OpenAI Tool Call Pairing', () => {
  it('should preserve valid tool-call/tool-response pairs', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result' }],
        tool_call_id: 'call_1',
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(2);
    expect(result[0].tool_calls).toHaveLength(1);
    expect(result[1].role).toBe('tool');
  });

  it('should remove orphaned tool messages', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [{ type: 'text', text: 'response' }],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result' }],
        tool_call_id: 'call_999', // No matching tool_call
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(1);
    expect(result[0].role).toBe('assistant');
  });

  it('should handle partial tool_calls matching', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test1', arguments: '{}' },
          },
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'test2', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result1' }],
        tool_call_id: 'call_1',
      },
      // call_2 has no tool response
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result[0].tool_calls).toHaveLength(1);
    expect(result[0].tool_calls![0].id).toBe('call_1');
    expect(result).toHaveLength(2);
  });

  it('should remove all tool_calls when none match', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [{ type: 'text', text: 'I will call functions' }],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
      },
      // No tool response
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(1);
    expect(result[0].tool_calls).toBeUndefined();
    expect(result[0].content).toEqual([
      { type: 'text', text: 'I will call functions' },
    ]);
  });

  it('should preserve non-tool messages', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Hello' }],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hi' }],
      },
      {
        id: '3',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'system',
        content: [{ type: 'text', text: 'You are helpful' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(3);
    expect(result[0].role).toBe('user');
    expect(result[1].role).toBe('assistant');
    expect(result[2].role).toBe('system');
  });

  it('should remove tool messages from conversation start', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'orphan' }],
        tool_call_id: 'call_orphan',
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Hello' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result[0].role).toBe('user');
    expect(result).toHaveLength(1);
  });

  it('should not affect Anthropic provider behavior', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.Anthropic,
    );

    // Anthropic's fixAnthropicToolCallChain logic should apply
    expect(result).toBeDefined();
    // The incomplete tool_call should be removed, and since content is empty, the message should be removed
    expect(result).toHaveLength(0);
  });

  it('should apply to Groq provider', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'orphan' }],
        tool_call_id: 'call_999',
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.Groq,
    );

    expect(result).toHaveLength(0); // Orphan removed
  });

  it('should apply to Cerebras provider', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result' }],
        tool_call_id: 'call_1',
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.Cerebras,
    );

    expect(result).toHaveLength(2);
    expect(result[0].tool_calls).toHaveLength(1);
  });

  it('should apply to Fireworks provider', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result' }],
        tool_call_id: 'call_1',
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.Fireworks,
    );

    expect(result).toHaveLength(2);
    expect(result[0].tool_calls).toHaveLength(1);
  });

  it('should handle empty message arrays', () => {
    const messages: Message[] = [];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(0);
  });

  it('should handle messages with no tool_calls or tool responses', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Hello' }],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hi there!' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toEqual(messages);
  });

  it('should handle multiple assistant messages with interleaved tool responses', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test1', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result1' }],
        tool_call_id: 'call_1',
      },
      {
        id: '3',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'test2', arguments: '{}' },
          },
        ],
      },
      {
        id: '4',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result2' }],
        tool_call_id: 'call_2',
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(4);
    expect(result[0].tool_calls).toHaveLength(1);
    expect(result[1].role).toBe('tool');
    expect(result[2].tool_calls).toHaveLength(1);
    expect(result[3].role).toBe('tool');
  });

  it('should handle tool messages with null or undefined tool_call_id', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'test-session',

        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'result without id' }],
        tool_call_id: undefined,
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    // The orphaned tool message should be removed
    // The assistant's tool_call should also be removed since no matching response
    // Since assistant message has no content, it should be removed too
    expect(result).toHaveLength(0);
  });
});

describe('MessageNormalizer - Consecutive User Message Merging', () => {
  it('should merge consecutive user messages into one', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'First message' }],
      },
      {
        id: '2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Second message' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(1);
    expect(result[0].role).toBe('user');
    const texts = result[0].content
      .filter((c) => c.type === 'text')
      .map((c) => (c as { type: 'text'; text: string }).text);
    expect(texts.join('')).toContain('First message');
    expect(texts.join('')).toContain('Second message');
  });

  it('should NOT merge tool messages with adjacent user messages', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'User ask' }],
      },
      {
        id: '2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'search', arguments: '{}' },
          },
        ],
      },
      {
        id: '3',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'tool result' }],
        tool_call_id: 'call_1',
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    // user / assistant / tool must remain separate — no merging across roles
    expect(result).toHaveLength(3);
    expect(result[0].role).toBe('user');
    expect(result[1].role).toBe('assistant');
    expect(result[2].role).toBe('tool');
  });

  // Regression: Bug fix — attachments from second user message were silently dropped
  it('should preserve attachments from both merged user messages', () => {
    const attachment1 = {
      sessionId: 'test-session',
      contentId: 'content-1',
      filename: 'file1.txt',
      mimeType: 'text/plain',
      size: 100,
      lineCount: 10,
      preview: 'line1\nline2',
      uploadedAt: new Date().toISOString(),
      status: 'committed' as const,
    };
    const attachment2 = {
      sessionId: 'test-session',
      contentId: 'content-2',
      filename: 'file2.txt',
      mimeType: 'text/plain',
      size: 200,
      lineCount: 20,
      preview: 'line1\nline2',
      uploadedAt: new Date().toISOString(),
      status: 'committed' as const,
    };

    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Analyze file1' }],
        attachments: [attachment1],
      },
      {
        id: '2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'And also file2' }],
        attachments: [attachment2],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(1);
    expect(result[0].attachments).toHaveLength(2);
    expect(result[0].attachments?.map((a) => a.contentId)).toEqual([
      'content-1',
      'content-2',
    ]);
  });

  // Regression: attachment from first message must not be lost when second has none
  it('should preserve attachments from first message when second has none', () => {
    const attachment1 = {
      sessionId: 'test-session',
      contentId: 'content-1',
      filename: 'file1.txt',
      mimeType: 'text/plain',
      size: 100,
      lineCount: 10,
      preview: 'line1\nline2',
      uploadedAt: new Date().toISOString(),
      status: 'committed' as const,
    };

    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Look at this' }],
        attachments: [attachment1],
      },
      {
        id: '2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'And summarize it' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI,
    );

    expect(result).toHaveLength(1);
    expect(result[0].attachments).toHaveLength(1);
    expect(result[0].attachments?.[0].contentId).toBe('content-1');
  });
});
