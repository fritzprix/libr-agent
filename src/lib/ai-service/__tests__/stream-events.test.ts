import { describe, expect, it } from 'vitest';

import {
  createSerializableDirectToolCall,
  createSerializableToolCallArgumentDelta,
  isParsedDirectToolCall,
  isParsedIndexedToolCallDelta,
  parseStreamChunk,
  serializeDirectToolCalls,
  serializeToolCallArgumentDeltas,
} from '../stream-events';

describe('stream-events tool call contract', () => {
  it('parses indexed deltas without requiring type, id, or name', () => {
    const chunk = parseStreamChunk(
      JSON.stringify({
        tool_calls: [
          {
            index: 0,
            function: {
              arguments: ',"content":"hello"}',
            },
          },
        ],
      }),
    );

    expect(chunk.tool_calls).toHaveLength(1);
    const toolCall = chunk.tool_calls?.[0];
    expect(isParsedIndexedToolCallDelta(toolCall)).toBe(true);
    expect(isParsedDirectToolCall(toolCall)).toBe(false);
  });

  it('does not classify indexed tool calls as direct snapshots', () => {
    const chunk = parseStreamChunk(
      JSON.stringify({
        tool_calls: [
          {
            index: 0,
            id: 'call_123',
            type: 'function',
            function: {
              name: 'workspace__writeFile',
              arguments: '{"path":"foo.txt"',
            },
          },
        ],
      }),
    );

    const toolCall = chunk.tool_calls?.[0];
    expect(isParsedIndexedToolCallDelta(toolCall)).toBe(true);
    expect(isParsedDirectToolCall(toolCall)).toBe(false);
  });

  it('serializes direct snapshots and indexed deltas as distinct tool_call shapes', () => {
    const directChunk = parseStreamChunk(
      serializeDirectToolCalls([
        createSerializableDirectToolCall(
          'call_gemini_1',
          'workspace__readFile',
          '{"path":"README.md"}',
        ),
      ]),
    );
    const indexedChunk = parseStreamChunk(
      serializeToolCallArgumentDeltas([
        createSerializableToolCallArgumentDelta(
          0,
          '{"path":"foo.txt"',
          {
            id: 'call_openai_1',
            name: 'workspace__writeFile',
          },
        ),
      ]),
    );

    expect(isParsedDirectToolCall(directChunk.tool_calls?.[0])).toBe(true);
    expect(isParsedIndexedToolCallDelta(indexedChunk.tool_calls?.[0])).toBe(
      true,
    );
  });
});
