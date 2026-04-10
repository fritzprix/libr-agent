import { describe, it, expect } from 'vitest';
import {
  formatToolResultForLlm,
  normalizeRustMessage,
  parseToolResultForLlm,
} from '../utils';
import type { RustMessage, Message } from '@/models/chat';

describe('normalizeRustMessage', () => {
  it('should convert toolCalls to tool_calls', () => {
    const rustMsg: RustMessage = {
      id: '1',
      sessionId: 'session-1',
      role: 'assistant',
      content: [],
      toolCalls: [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'test', arguments: '{}' },
        },
      ],
      createdAt: 1234567890000,
      updatedAt: 1234567890000,
    };

    const normalized = normalizeRustMessage(rustMsg);
    expect(normalized.tool_calls).toBeDefined();
    expect(normalized.tool_calls).toHaveLength(1);
    expect(normalized.tool_calls![0].id).toBe('call_1');
    expect('toolCalls' in normalized).toBe(false);
    expect(normalized.createdAt).toBeInstanceOf(Date);
    expect(normalized.createdAt?.getTime()).toBe(1234567890000);
  });

  it('should convert toolCallId to tool_call_id', () => {
    const rustMsg: RustMessage = {
      id: '2',
      sessionId: 'session-1',
      role: 'tool',
      content: [],
      toolCallId: 'call_1',
      createdAt: 1234567890000,
      updatedAt: 1234567890000,
    };

    const normalized = normalizeRustMessage(rustMsg);
    expect(normalized.tool_call_id).toBe('call_1');
    expect('toolCallId' in normalized).toBe(false);
  });

  it('should leave already normalized messages alone', () => {
    const msg: Message = {
      id: '3',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'assistant',
      content: [],
      tool_calls: [],
      createdAt: new Date(1234567890000),
    };

    const normalized = normalizeRustMessage(msg);
    expect(normalized.tool_calls).toBeDefined();
    expect('toolCalls' in normalized).toBe(false);
    expect(normalized.createdAt).toBeInstanceOf(Date);
    expect(normalized.createdAt?.getTime()).toBe(1234567890000);
  });
});

describe('tool result helpers', () => {
  it('uses only content text for LLM-facing tool results', () => {
    const message: Message = {
      id: 'tool-1',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'tool',
      tool_call_id: 'call_1',
      content: [{ type: 'text', text: 'Human summary only' }],
      metadata: {
        structuredContent: {
          toolName: 'checkSession',
          status: 'idle',
          responseStatus: 'success',
          result: 'Final answer',
        },
      },
    };

    expect(formatToolResultForLlm(message)).toBe('Human summary only');
    expect(parseToolResultForLlm(message)).toEqual({
      result: 'Human summary only',
    });
  });

  it('does not auto-parse JSON-looking tool text', () => {
    const message: Message = {
      id: 'tool-2',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'tool',
      tool_call_id: 'call_2',
      content: [{ type: 'text', text: '{"temp":25}' }],
    };

    expect(formatToolResultForLlm(message)).toBe('{"temp":25}');
    expect(parseToolResultForLlm(message)).toEqual({
      result: '{"temp":25}',
    });
  });
});
