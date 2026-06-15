import { describe, expect, it } from 'vitest';
import {
  messagesToMarkdown,
  summarizeMessageForLog,
  toRustMessage,
} from '../message-utils';
import type { Message } from '@/models/chat';

function createMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'user',
    content: [{ type: 'text', text: 'Hello' }],
    ...overrides,
  };
}

describe('messagesToMarkdown', () => {
  it('formats user and assistant messages as readable markdown', () => {
    const messages: Message[] = [
      createMessage({ role: 'user', content: [{ type: 'text', text: 'Hi' }] }),
      createMessage({
        id: 'msg-2',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hello there' }],
      }),
    ];

    const { content, truncated } = messagesToMarkdown(messages);

    expect(truncated).toBe(false);
    expect(content).toContain('## User');
    expect(content).toContain('Hi');
    expect(content).toContain('## Assistant');
    expect(content).toContain('Hello there');
    expect(content).not.toContain('"sessionId"');
    expect(content).not.toContain('base64');
  });

  it('excludes system messages and synthetic compaction sources by default', () => {
    const messages: Message[] = [
      createMessage({ role: 'system', content: [{ type: 'text', text: 'System' }] }),
      createMessage({
        id: 'msg-2',
        source: 'compact-summary',
        role: 'assistant',
        content: [{ type: 'text', text: 'Summary' }],
      }),
      createMessage({
        id: 'msg-3',
        role: 'user',
        content: [{ type: 'text', text: 'Visible' }],
      }),
    ];

    const { content } = messagesToMarkdown(messages);

    expect(content).not.toContain('System');
    expect(content).not.toContain('Summary');
    expect(content).toContain('Visible');
  });

  it('skips streaming messages', () => {
    const messages: Message[] = [
      createMessage({
        isStreaming: true,
        content: [{ type: 'text', text: 'In progress' }],
      }),
      createMessage({
        id: 'msg-2',
        role: 'assistant',
        content: [{ type: 'text', text: 'Done' }],
      }),
    ];

    const { content } = messagesToMarkdown(messages);

    expect(content).not.toContain('In progress');
    expect(content).toContain('Done');
  });

  it('includes tool calls and tool role metadata when enabled', () => {
    const messages: Message[] = [
      createMessage({
        role: 'assistant',
        tool_calls: [
          {
            id: 'call-1',
            type: 'function',
            function: { name: 'read_file', arguments: '{"path":"a.txt"}' },
          },
        ],
        content: [],
      }),
      createMessage({
        id: 'msg-2',
        role: 'tool',
        tool_call_id: 'call-1',
        content: [{ type: 'text', text: 'file contents' }],
      }),
    ];

    const { content } = messagesToMarkdown(messages);

    expect(content).toContain('**Tool:** read_file');
    expect(content).toContain('"path": "a.txt"');
    expect(content).toContain('## Tool');
    expect(content).toContain('Tool Call ID: call-1');
    expect(content).toContain('file contents');
  });

  it('summarizes binary and resource content without embedding payloads', () => {
    const messages: Message[] = [
      createMessage({
        role: 'assistant',
        content: [
          { type: 'image', mimeType: 'image/png', data: 'aGVsbG8=' },
          {
            type: 'resource_link',
            uri: 'file:///tmp/report.pdf',
            name: 'report.pdf',
          },
          {
            type: 'resource',
            resource: {
              mimeType: 'text/html',
              uri: 'ui://widget',
              text: '<p>hidden</p>',
            },
          },
        ],
      }),
    ];

    const { content } = messagesToMarkdown(messages);

    expect(content).toContain('[Image: image/png]');
    expect(content).toContain('[Resource Link: report.pdf](file:///tmp/report.pdf)');
    expect(content).toContain('[UI Resource: text/html - ui://widget]');
    expect(content).not.toContain('aGVsbG8=');
    expect(content).not.toContain('<p>hidden</p>');
  });

  it('includes attachment filename and preview without inline binary data', () => {
    const messages: Message[] = [
      createMessage({
        attachments: [
          {
            sessionId: 'session-1',
            filename: 'notes.txt',
            mimeType: 'text/plain',
            size: 12,
            lineCount: 2,
            preview: 'line one\nline two',
            uploadedAt: '2026-01-01T00:00:00.000Z',
            status: 'inline',
            inlineContent: {
              type: 'image',
              data: 'secret-base64',
              mimeType: 'image/png',
            },
          },
        ],
      }),
    ];

    const { content } = messagesToMarkdown(messages);

    expect(content).toContain('**Attachment:** notes.txt');
    expect(content).toContain('> line one');
    expect(content).toContain('> line two');
    expect(content).not.toContain('secret-base64');
  });

  it('marks truncation when byte budget is exceeded', () => {
    const messages: Message[] = Array.from({ length: 5 }, (_, index) =>
      createMessage({
        id: `msg-${index}`,
        content: [{ type: 'text', text: 'x'.repeat(200) }],
      }),
    );

    const { content, truncated, omittedCount } = messagesToMarkdown(messages, {
      maxBytes: 300,
    });

    expect(truncated).toBe(true);
    expect(omittedCount).toBeGreaterThan(0);
    expect(content).toContain('message(s) omitted due to size limits');
  });

  it('omits older messages when maxMessages is set', () => {
    const messages: Message[] = Array.from({ length: 4 }, (_, index) =>
      createMessage({
        id: `msg-${index}`,
        content: [{ type: 'text', text: `message-${index}` }],
      }),
    );

    const { content, truncated, omittedCount } = messagesToMarkdown(messages, {
      maxMessages: 2,
    });

    expect(truncated).toBe(true);
    expect(omittedCount).toBe(2);
    expect(content).toContain('message-2');
    expect(content).toContain('message-3');
    expect(content).not.toContain('message-0');
  });
});

describe('toRustMessage', () => {
  it('maps tool_calls to toolCalls and converts Date timestamps to milliseconds', () => {
    const createdAt = new Date('2026-06-15T10:00:00.000Z');
    const updatedAt = new Date('2026-06-15T10:05:00.000Z');
    const message = createMessage({
      createdAt,
      updatedAt,
      tool_calls: [
        {
          id: 'call-1',
          type: 'function',
          function: { name: 'search', arguments: '{"q":"test"}' },
        },
      ],
      tool_call_id: 'call-1',
    });

    const rustMessage = toRustMessage(message);

    expect(rustMessage.toolCalls).toEqual(message.tool_calls);
    expect(rustMessage.toolCallId).toBe('call-1');
    expect(rustMessage.createdAt).toBe(createdAt.getTime());
    expect(rustMessage.updatedAt).toBe(updatedAt.getTime());
  });

  it('falls back to numeric timestamps and now when dates are missing', () => {
    const createdAt = 1_700_000_000_000;
    const message = {
      ...createMessage(),
      createdAt: createdAt as unknown,
      updatedAt: undefined,
    } as Message;

    const rustMessage = toRustMessage(message);

    expect(rustMessage.createdAt).toBe(createdAt);
    expect(rustMessage.updatedAt).toBe(createdAt);
  });
});

describe('summarizeMessageForLog', () => {
  it('returns null for undefined input', () => {
    expect(summarizeMessageForLog(undefined)).toBeNull();
  });

  it('summarizes content types, lengths, and tool call metadata', () => {
    const message = createMessage({
      role: 'assistant',
      isStreaming: true,
      content: [
        { type: 'text', text: 'Hello' },
        { type: 'image', mimeType: 'image/png', data: 'abc' },
      ],
      thinking: 'reasoning',
      tool_calls: [
        {
          id: 'call-1',
          type: 'function',
          function: { name: 'search', arguments: '{"q":"test"}' },
        },
      ],
    });

    expect(summarizeMessageForLog(message)).toEqual({
      id: message.id,
      role: 'assistant',
      isStreaming: true,
      contentTypes: ['text', 'image'],
      textLength: 5,
      thinkingLength: 9,
      toolCallCount: 1,
      toolCalls: [
        {
          id: 'call-1',
          name: 'search',
          argumentsLength: '{"q":"test"}'.length,
        },
      ],
    });
  });
});
