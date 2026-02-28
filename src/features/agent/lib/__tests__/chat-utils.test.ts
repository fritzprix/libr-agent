import { describe, it, expect } from 'vitest';
import { computeDisplayContent } from '../chat-utils';
import type { Message, ToolCall } from '@/models/chat';
import type { MCPToolCallContent } from '@/lib/mcp';

describe('chat-utils', () => {
  describe('computeDisplayContent', () => {
    const baseMessage: Message = {
      id: 'msg-1',
      role: 'assistant',
      content: [{ type: 'text', text: 'Base message text' }],
      createdAt: new Date(),
    };

    it('returns undefined when neither groupedMessages nor groupedToolCalls are provided', () => {
      expect(computeDisplayContent(baseMessage)).toBeUndefined();
    });

    it('returns undefined when empty groupedMessages and no groupedToolCalls are provided', () => {
      expect(computeDisplayContent(baseMessage, [])).toBeUndefined();
    });

    it('processes groupedMessages with regular text content', () => {
      const groupedMessages: Message[] = [
        {
          id: 'grp-1',
          role: 'assistant',
          content: [{ type: 'text', text: 'Grouped message 1' }],
          createdAt: new Date(),
        },
        {
          id: 'grp-2',
          role: 'assistant',
          content: [{ type: 'text', text: 'Grouped message 2' }],
          createdAt: new Date(),
        },
      ];

      const result = computeDisplayContent(baseMessage, groupedMessages);

      expect(result).toEqual([
        { type: 'text', text: 'Grouped message 1' },
        { type: 'text', text: 'Grouped message 2' },
      ]);
    });

    it('filters out existing tool_call content and recreates them from tool_calls property in groupedMessages', () => {
      const toolCallContent: MCPToolCallContent = {
        type: 'tool_call',
        id: 'tc-old',
        name: 'old_tool',
        arguments: { arg: 'old' },
      };

      const groupedMessages: Message[] = [
        {
          id: 'grp-1',
          role: 'assistant',
          content: [
            { type: 'text', text: 'Some text' },
            toolCallContent,
          ],
          tool_calls: [
            {
              id: 'tc-1',
              type: 'function',
              function: {
                name: 'test_tool',
                arguments: JSON.stringify({ key: 'value' }),
              },
            },
          ],
          createdAt: new Date(),
        },
      ];

      const result = computeDisplayContent(baseMessage, groupedMessages);

      expect(result).toEqual([
        { type: 'text', text: 'Some text' },
        {
          type: 'tool_call',
          id: 'tc-1',
          name: 'test_tool',
          arguments: JSON.stringify({ key: 'value' }),
        },
      ]);
    });

    it('handles groupedMessages when content is a string', () => {
      const groupedMessages: Message[] = [
        {
          id: 'grp-1',
          role: 'assistant',
          content: 'This is a string content',
          createdAt: new Date(),
        },
      ];

      const result = computeDisplayContent(baseMessage, groupedMessages);

      // If content is not an array, originalContent becomes [].
      // nonToolContent becomes [], toolContent becomes [] because tool_calls is empty.
      // So result is []
      expect(result).toEqual([]);
    });

    it('processes groupedToolCalls and combines with primary message non-tool content', () => {
      const messageWithToolCalls: Message = {
        ...baseMessage,
        content: [
          { type: 'text', text: 'Primary text' },
          { type: 'tool_call', id: 'tc-old', name: 'old', arguments: {} } as MCPToolCallContent,
        ],
      };

      const groupedToolCalls: ToolCall[] = [
        {
          id: 'tc-2',
          type: 'function',
          function: {
            name: 'another_tool',
            arguments: JSON.stringify({ p: 1 }),
          },
        },
      ];

      const result = computeDisplayContent(messageWithToolCalls, undefined, groupedToolCalls);

      expect(result).toEqual([
        { type: 'text', text: 'Primary text' },
        {
          type: 'tool_call',
          id: 'tc-2',
          name: 'another_tool',
          arguments: JSON.stringify({ p: 1 }),
        },
      ]);
    });

    it('handles msg.content as a string when processing groupedToolCalls', () => {
      const stringMessage: Message = {
        id: 'msg-str',
        role: 'assistant',
        content: 'String content',
        createdAt: new Date(),
      };

      const groupedToolCalls: ToolCall[] = [
        {
          id: 'tc-3',
          type: 'function',
          function: {
            name: 'str_tool',
            arguments: '{}',
          },
        },
      ];

      const result = computeDisplayContent(stringMessage, undefined, groupedToolCalls);

      expect(result).toEqual([
        {
          type: 'tool_call',
          id: 'tc-3',
          name: 'str_tool',
          arguments: '{}',
        },
      ]);
    });

    it('handles empty groupedToolCalls', () => {
      const message: Message = {
        ...baseMessage,
        content: [
          { type: 'text', text: 'Primary text' },
        ],
      };

      const result = computeDisplayContent(message, undefined, []);

      expect(result).toEqual([
        { type: 'text', text: 'Primary text' },
      ]);
    });
  });
});
