import { describe, it, expect } from 'vitest';
import { batchToolCallsInMessages } from '../token-utils';
import type { Message } from '@/models/chat';

describe('batchToolCallsInMessages', () => {
  it('should not modify messages with fewer than maxToolCallsPerMessage tool calls', () => {
    const messages: Message[] = [
      {
        id: 'msg1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'assistant',
        content: [{ type: 'text', text: 'Using tools' }],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'tool1', arguments: '{}' },
          },
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'tool2', arguments: '{}' },
          },
        ],
      },
      {
        id: 'tool1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result1' }],
        tool_call_id: 'call_1',
      },
      {
        id: 'tool2',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result2' }],
        tool_call_id: 'call_2',
      },
    ];

    const result = batchToolCallsInMessages(messages, 4);

    expect(result).toHaveLength(3);
    expect(result[0].id).toBe('msg1');
    expect(result[0].tool_calls).toHaveLength(2);
  });

  it('should batch messages with 6 tool calls into 2 batches of 4 and 2', () => {
    const messages: Message[] = [
      {
        id: 'msg1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'assistant',
        content: [{ type: 'text', text: 'Using many tools' }],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'tool1', arguments: '{}' },
          },
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'tool2', arguments: '{}' },
          },
          {
            id: 'call_3',
            type: 'function',
            function: { name: 'tool3', arguments: '{}' },
          },
          {
            id: 'call_4',
            type: 'function',
            function: { name: 'tool4', arguments: '{}' },
          },
          {
            id: 'call_5',
            type: 'function',
            function: { name: 'tool5', arguments: '{}' },
          },
          {
            id: 'call_6',
            type: 'function',
            function: { name: 'tool6', arguments: '{}' },
          },
        ],
      },
      {
        id: 'tool1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result1' }],
        tool_call_id: 'call_1',
      },
      {
        id: 'tool2',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result2' }],
        tool_call_id: 'call_2',
      },
      {
        id: 'tool3',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result3' }],
        tool_call_id: 'call_3',
      },
      {
        id: 'tool4',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result4' }],
        tool_call_id: 'call_4',
      },
      {
        id: 'tool5',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result5' }],
        tool_call_id: 'call_5',
      },
      {
        id: 'tool6',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'tool',
        content: [{ type: 'text', text: 'result6' }],
        tool_call_id: 'call_6',
      },
    ];

    const result = batchToolCallsInMessages(messages, 4);

    // Should create 2 assistant messages + 6 tool responses = 8 messages total
    expect(result).toHaveLength(8); // 2 batches + 6 tool responses

    // First batch should have 4 tool calls
    expect(result[0].id).toBe('msg1_batch_0');
    expect(result[0].tool_calls).toHaveLength(4);
    expect(result[0].tool_calls?.[0].id).toBe('call_1');
    expect(result[0].tool_calls?.[3].id).toBe('call_4');

    // First batch should be followed by its 4 tool responses
    expect(result[1].role).toBe('tool');
    expect(result[1].tool_call_id).toBe('call_1');
    expect(result[4].role).toBe('tool');
    expect(result[4].tool_call_id).toBe('call_4');

    // Second batch should have 2 tool calls
    expect(result[5].id).toBe('msg1_batch_1');
    expect(result[5].tool_calls).toHaveLength(2);
    expect(result[5].tool_calls?.[0].id).toBe('call_5');
    expect(result[5].tool_calls?.[1].id).toBe('call_6');

    // Second batch should be followed by its 2 tool responses
    expect(result[6].role).toBe('tool');
    expect(result[6].tool_call_id).toBe('call_5');
    expect(result[7].role).toBe('tool');
    expect(result[7].tool_call_id).toBe('call_6');
  });

  it('should handle 20+ tool calls correctly', () => {
    // Create 20 tool calls
    const toolCalls = Array.from({ length: 20 }, (_, i) => ({
      id: `call_${i + 1}`,
      type: 'function' as const,
      function: { name: `tool${i + 1}`, arguments: '{}' },
    }));

    const toolResponses = Array.from({ length: 20 }, (_, i) => ({
      id: `tool${i + 1}`,
      sessionId: 'test-session',
      threadId: 'test-thread',
      role: 'tool' as const,
      content: [{ type: 'text' as const, text: `result${i + 1}` }],
      tool_call_id: `call_${i + 1}`,
    }));

    const messages: Message[] = [
      {
        id: 'msg1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'assistant',
        content: [{ type: 'text', text: 'Using 20 tools' }],
        tool_calls: toolCalls,
      },
      ...toolResponses,
    ];

    const result = batchToolCallsInMessages(messages, 4);

    // Should create 5 batches (20 tool calls / 4 = 5 batches)
    // 5 batches + 20 tool responses = 25 messages total
    expect(result).toHaveLength(25);

    // Verify first batch
    expect(result[0].id).toBe('msg1_batch_0');
    expect(result[0].tool_calls).toHaveLength(4);
    expect(result[0].content).toEqual([{ type: 'text', text: 'Using 20 tools' }]);

    // Verify last batch
    const lastBatchIndex = 20; // 4 batches * 5 messages each
    expect(result[lastBatchIndex].id).toBe('msg1_batch_4');
    expect(result[lastBatchIndex].tool_calls).toHaveLength(4);
    expect(result[lastBatchIndex].content).toEqual([
      { type: 'text', text: '[Continuing tool calls - Batch 5/5]' },
    ]);
  });

  it('should preserve non-assistant messages unchanged', () => {
    const messages: Message[] = [
      {
        id: 'user1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'user',
        content: [{ type: 'text', text: 'Hello' }],
      },
      {
        id: 'system1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'system',
        content: [{ type: 'text', text: 'System message' }],
      },
    ];

    const result = batchToolCallsInMessages(messages, 4);

    expect(result).toHaveLength(2);
    expect(result[0]).toEqual(messages[0]);
    expect(result[1]).toEqual(messages[1]);
  });

  it('should use default maxToolCallsPerMessage of 4 when invalid value provided', () => {
    const messages: Message[] = [
      {
        id: 'msg1',
        sessionId: 'test-session',
        threadId: 'test-thread',
        role: 'assistant',
        content: [{ type: 'text', text: 'Using tools' }],
        tool_calls: Array.from({ length: 10 }, (_, i) => ({
          id: `call_${i + 1}`,
          type: 'function' as const,
          function: { name: `tool${i + 1}`, arguments: '{}' },
        })),
      },
    ];

    // Try with invalid value (0)
    const result = batchToolCallsInMessages(messages, 0);

    // Should fall back to default of 4, creating 3 batches (10/4 = 2.5 = 3 batches)
    expect(result.length).toBeGreaterThan(1);
  });
});
