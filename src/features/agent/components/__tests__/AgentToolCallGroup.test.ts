import { describe, it, expect } from 'vitest';
import { arePropsEqual } from '../AgentToolCallGroup';
import type { Message, ToolCall } from '@/models/chat';

function createToolCall(args: string): ToolCall {
  return {
    id: 'call-1',
    type: 'function',
    function: {
      name: 'workspace_read_file',
      arguments: args,
    },
  };
}

function createMessage(id: string): Message {
  return {
    id,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'assistant',
    content: [{ type: 'text', text: 'test' }],
  };
}

describe('AgentToolCallGroup arePropsEqual', () => {
  it('returns true when tool calls are reconstructed with identical content', () => {
    const prevProps = {
      message: createMessage('msg-1'),
      toolGroup: {
        calls: [createToolCall('{"path":"src/main.ts"}')],
      },
      toolResults: [],
      isLast: true,
      visibleCount: 3,
    };

    const nextProps = {
      message: createMessage('msg-1'),
      toolGroup: {
        calls: [createToolCall('{"path":"src/main.ts"}')],
      },
      toolResults: [],
      isLast: true,
      visibleCount: 3,
    };

    expect(arePropsEqual(prevProps, nextProps)).toBe(true);
  });

  it('returns false when only function.arguments changes', () => {
    const prevProps = {
      message: createMessage('msg-1'),
      toolGroup: {
        calls: [createToolCall('{"path":"src/main.ts"}')],
      },
      toolResults: [],
      isLast: true,
      visibleCount: 3,
    };

    const nextProps = {
      message: createMessage('msg-1'),
      toolGroup: {
        calls: [createToolCall('{"path":"src/updated.ts"}')],
      },
      toolResults: [],
      isLast: true,
      visibleCount: 3,
    };

    expect(arePropsEqual(prevProps, nextProps)).toBe(false);
  });
});
