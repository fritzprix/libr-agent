import { describe, it, expect } from 'vitest';
import { arePropsEqual } from '../ToolCallCompactItem';
import type { ToolCall } from '@/models/chat';

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

describe('ToolCallCompactItem arePropsEqual', () => {
  it('returns true for reconstructed toolCall objects with identical content', () => {
    const prevProps = {
      toolCall: createToolCall('{"path":"src/main.ts"}'),
      toolResult: undefined,
      isLast: true,
    };

    const nextProps = {
      toolCall: createToolCall('{"path":"src/main.ts"}'),
      toolResult: undefined,
      isLast: true,
    };

    expect(arePropsEqual(prevProps, nextProps)).toBe(true);
  });

  it('returns false when relevant toolCall fields change', () => {
    const prevProps = {
      toolCall: createToolCall('{"path":"src/main.ts"}'),
      toolResult: undefined,
      isLast: true,
    };

    const nextProps = {
      toolCall: createToolCall('{"path":"src/updated.ts"}'),
      toolResult: undefined,
      isLast: true,
    };

    expect(arePropsEqual(prevProps, nextProps)).toBe(false);
  });
});
