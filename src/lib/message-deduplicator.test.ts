import { describe, it, expect } from 'vitest';
import { deduplicateToolCallPairs } from './message-deduplicator';
import { Message } from '@/models/chat';

describe('deduplicateToolCallPairs', () => {
  const createToolCallMessage = (
    id: string,
    toolCallId: string,
    toolName: string,
    args: string,
  ): Message => ({
    id,
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'assistant',
    content: [],
    tool_calls: [
      {
        id: toolCallId,
        type: 'function',
        function: {
          name: toolName,
          arguments: args,
        },
      },
    ],
  });

  const createToolResponseMessage = (
    id: string,
    toolCallId: string,
    responseText: string,
  ): Message => ({
    id,
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'tool',
    tool_call_id: toolCallId,
    content: [
      {
        type: 'text',
        text: responseText,
      },
    ],
  });

  it('should return messages unchanged if count is below minimum', () => {
    const messages: Message[] = [
      createToolCallMessage('1', 'call_1', 'read_file', '{"path":"test.txt"}'),
      createToolResponseMessage('2', 'call_1', 'Error: File not found'),
    ];

    const result = deduplicateToolCallPairs(messages, { minMessageCount: 10, preserveRecentN: 3 });

    expect(result).toHaveLength(2);
    expect(result).toEqual(messages);
  });

  it('should deduplicate repeated error messages', () => {
    const messages: Message[] = [
      // First occurrence
      createToolCallMessage('1', 'call_1', 'read_file', '{"path":"missing.txt"}'),
      createToolResponseMessage('2', 'call_1', 'Error: File not found'),
      // Duplicate
      createToolCallMessage('3', 'call_2', 'read_file', '{"path":"missing.txt"}'),
      createToolResponseMessage('4', 'call_2', 'Error: File not found'),
      // Duplicate
      createToolCallMessage('5', 'call_3', 'read_file', '{"path":"missing.txt"}'),
      createToolResponseMessage('6', 'call_3', 'Error: File not found'),
      // Different message
      {
        id: '7',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'user',
        content: [{ type: 'text', text: 'Try again' }],
      },
      // Recent messages (preserved)
      createToolCallMessage('8', 'call_4', 'read_file', '{"path":"missing.txt"}'),
      createToolResponseMessage('9', 'call_4', 'Error: File not found'),
      {
        id: '10',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'user',
        content: [{ type: 'text', text: 'Final message' }],
      },
    ];

    const result = deduplicateToolCallPairs(messages, { minMessageCount: 10, preserveRecentN: 3 });

    // Should remove 4 messages (2 duplicate pairs)
    expect(result.length).toBeLessThan(messages.length);
    
    // First occurrence should have metadata
    const firstToolResponse = result.find(m => m.id === '2');
    expect(firstToolResponse?.metadata?.dedupCount).toBe(3);
    expect(firstToolResponse?.content[0]?.type).toBe('text');
    if (firstToolResponse?.content[0]?.type === 'text') {
      expect(firstToolResponse.content[0].text).toContain('(repeated 3x)');
    }

    // Recent messages should be preserved
    expect(result.find(m => m.id === '8')).toBeDefined();
    expect(result.find(m => m.id === '9')).toBeDefined();
    expect(result.find(m => m.id === '10')).toBeDefined();
  });

  it('should deduplicate repeated successful reads', () => {
    const messages: Message[] = [
      // First occurrence
      createToolCallMessage('1', 'call_1', 'read_file', '{"path":"config.json"}'),
      createToolResponseMessage('2', 'call_1', '{"version":"1.0"}'),
      // Duplicate
      createToolCallMessage('3', 'call_2', 'read_file', '{"path":"config.json"}'),
      createToolResponseMessage('4', 'call_2', '{"version":"1.0"}'),
      // Different result (should keep)
      createToolCallMessage('5', 'call_3', 'read_file', '{"path":"config.json"}'),
      createToolResponseMessage('6', 'call_3', '{"version":"1.1"}'),
      // Padding to reach minMessageCount
      { id: '7', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg1' }] },
      { id: '8', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg2' }] },
      { id: '9', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg3' }] },
      { id: '10', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg4' }] },
    ];

    const result = deduplicateToolCallPairs(messages, { minMessageCount: 10, preserveRecentN: 3 });

    // Should remove the duplicate pair (2 messages)
    expect(result.length).toBe(messages.length - 2);

    // First occurrence metadata
    const firstToolResponse = result.find(m => m.id === '2');
    expect(firstToolResponse?.metadata?.dedupCount).toBe(2);

    // Different result should still exist
    expect(result.find(m => m.id === '6')).toBeDefined();
  });

  it('should preserve recent N messages', () => {
    const messages: Message[] = [];
    
    // Add 10 duplicate pairs
    for (let i = 0; i < 10; i++) {
      messages.push(
        createToolCallMessage(`${i * 2 + 1}`, `call_${i}`, 'test_tool', '{"arg":"value"}'),
        createToolResponseMessage(`${i * 2 + 2}`, `call_${i}`, 'result'),
      );
    }

    const result = deduplicateToolCallPairs(messages, { minMessageCount: 10, preserveRecentN: 4 });

    // Last 4 messages should be preserved (2 pairs)
    const lastFourOriginal = messages.slice(-4);
    const lastFourResult = result.slice(-4);
    
    expect(lastFourResult.map(m => m.id)).toEqual(lastFourOriginal.map(m => m.id));
  });

  it('should not break tool_call_id pairing', () => {
    const messages: Message[] = [
      createToolCallMessage('1', 'call_1', 'tool_a', '{"x":1}'),
      createToolResponseMessage('2', 'call_1', 'result_a'),
      createToolCallMessage('3', 'call_2', 'tool_a', '{"x":1}'),
      createToolResponseMessage('4', 'call_2', 'result_a'),
      { id: '5', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg1' }] },
      { id: '6', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg2' }] },
      { id: '7', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg3' }] },
      { id: '8', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg4' }] },
      { id: '9', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg5' }] },
      { id: '10', sessionId: 'test-session', threadId: 'test-thread', role: 'user', content: [{ type: 'text', text: 'msg6' }] },
    ];

    const result = deduplicateToolCallPairs(messages, { minMessageCount: 10, preserveRecentN: 3 });

    // Verify all remaining tool messages have corresponding tool_calls
    result.forEach((msg, idx) => {
      if (msg.role === 'tool') {
        // Previous message should be assistant with matching tool_call
        const prevMsg = result[idx - 1];
        expect(prevMsg?.role).toBe('assistant');
        expect(prevMsg?.tool_calls?.[0]?.id).toBe(msg.tool_call_id);
      }
    });
  });
});
