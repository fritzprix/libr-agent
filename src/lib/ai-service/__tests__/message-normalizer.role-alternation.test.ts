import { describe, it, expect } from 'vitest';
import { MessageNormalizer } from '../message-normalizer';
import { AIServiceProvider } from '../types';
import type { Message } from '@/models/chat';
import { MCPTextContent } from '@/lib/mcp';

describe('MessageNormalizer - Role Alternation', () => {
  it('should merge consecutive user messages', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: 'Hello' }],
      },
      {
        id: '2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: 'World' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI
    );

    expect(result).toHaveLength(1);
    expect(result[0].role).toBe('user');
    expect(result[0].content).toHaveLength(2);
    expect((result[0].content[0] as MCPTextContent).text).toBe('Hello');
    expect((result[0].content[1] as MCPTextContent).text).toBe('World');
  });

  it('should merge consecutive assistant messages', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response 1' }],
      },
      {
        id: '2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response 2' }],
      },
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI
    );

    expect(result).toHaveLength(1);
    expect(result[0].role).toBe('assistant');
    expect(result[0].content).toHaveLength(2);
    expect((result[0].content[0] as MCPTextContent).text).toBe('Response 1');
    expect((result[0].content[1] as MCPTextContent).text).toBe('Response 2');
  });

  it('should merge consecutive assistant messages with tool calls and thinking', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [],
        thinking: 'Let me think...',
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'tool_1', arguments: '{}' },
          },
        ],
      },
      {
        id: '2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'And also...' }],
        thinking: 'Thinking more...',
        tool_calls: [
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'tool_2', arguments: '{}' },
          },
        ],
      },
    ];

    // Add matching tool results to prevent validateToolCallPairing from stripping them
    messages.push(
      {
        id: '3',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        content: [{ type: 'text', text: 'result 1' }],
        tool_call_id: 'call_1',
      },
      {
        id: '4',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        content: [{ type: 'text', text: 'result 2' }],
        tool_call_id: 'call_2',
      }
    );

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.Anthropic
    );

    // result should be [Merged Assistant, Tool 1, Tool 2]
    expect(result).toHaveLength(3);
    expect(result[0].role).toBe('assistant');
    expect(result[0].thinking).toBe('Let me think...\n\nThinking more...');
    expect(result[0].tool_calls).toHaveLength(2);
    expect(result[0].content).toHaveLength(1);
    expect((result[0].content[0] as MCPTextContent).text).toBe('And also...');
    expect(result[1].role).toBe('tool');
    expect(result[2].role).toBe('tool');
  });

  it('should handle role alternation issues created by filtering', () => {
    const messages: Message[] = [
      {
        id: '1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: 'Prompt' }],
      },
      {
        id: '2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [], // Empty assistant message (will be filtered by sanitizer)
      },
      {
        id: '3',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: 'Follow-up' }],
      },
    ];

    // sanitizeMessagesForProvider calls sanitizeSingleMessage after ensureRoleAlternation
    // But ensureRoleAlternation is called FIRST.
    // If sanitizeSingleMessage returns null, we might still have alternation issues.
    // Wait, the current pipeline is:
    // 1. filterSystemErrors
    // 2. validateToolCallPairing
    // 3. validateToolCallArguments
    // 4. ensureRoleAlternation
    // 5. sanitizeSingleMessage + filter(null)

    // Actually, if sanitizeSingleMessage filters out an empty message, we might still end up with User-User.
    // I should probably move ensureRoleAlternation to AFTER the map/filter.

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.Anthropic
    );

    // If '2' is filtered out because it's empty AND Anthropic doesn't like empty assistant,
    // we get [User, User].
    // Let's see what happens.
    expect(result).toHaveLength(1); // Combined into one User message
    expect(result[0].role).toBe('user');
  });

  it('should not change anything if roles alternate correctly', () => {
    const messages: Message[] = [
      { id: '1', role: 'user', content: [{ type: 'text', text: 'U1' }] } as unknown as Message,
      { id: '2', role: 'assistant', content: [{ type: 'text', text: 'A1' }] } as unknown as Message,
      { id: '3', role: 'user', content: [{ type: 'text', text: 'U2' }] } as unknown as Message,
    ];

    const result = MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      AIServiceProvider.OpenAI
    );

    expect(result).toHaveLength(3);
    expect(result[0].role).toBe('user');
    expect(result[1].role).toBe('assistant');
    expect(result[2].role).toBe('user');
  });
});
