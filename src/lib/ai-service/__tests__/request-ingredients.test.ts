import { describe, expect, it } from 'vitest';

import {
  summarizeCompactionRequestSizes,
  summarizeMessageIngredients,
  type RequestIngredientMessageLike,
} from '../request-ingredients';

describe('summarizeMessageIngredients', () => {
  it('counts roles, sources, and assistant tool calls consistently', () => {
    const messages: RequestIngredientMessageLike[] = [
      { role: 'user', source: 'ui', tool_calls: undefined },
      { role: 'assistant', source: 'compact-summary', tool_calls: undefined },
      {
        role: 'assistant',
        source: 'session-context',
        tool_calls: [
          {
            id: 'tool-1',
            type: 'function',
            function: { name: 'workspace__read', arguments: '{}' },
          },
        ],
      },
      { role: 'tool', source: 'api', tool_calls: undefined },
      { role: 'user', source: 'compaction-instruction', tool_calls: undefined },
      { role: 'user', source: undefined, tool_calls: undefined },
    ];

    expect(summarizeMessageIngredients(messages)).toEqual({
      messageCount: 6,
      roleCounts: {
        user: 3,
        assistant: 2,
        tool: 1,
      },
      sourceCounts: {
        ui: 1,
        'compact-summary': 1,
        'session-context': 1,
        api: 1,
        'compaction-instruction': 1,
        none: 1,
      },
      compactSummaryCount: 1,
      compactionInstructionCount: 1,
      sessionContextCount: 1,
      externalRequestCount: 2,
      assistantToolCallCount: 1,
    });
  });

  describe('summarizeCompactionRequestSizes', () => {
    it('captures compaction payload size by component without raw JSON dumps', () => {
      expect(
        summarizeCompactionRequestSizes({
          systemPrompt: 'system prompt',
          availableTools: [
            {
              name: 'workspace__read',
              description: 'Read a workspace file',
              inputSchema: {
                type: 'object',
                properties: {
                  path: { type: 'string' },
                },
              },
            },
          ],
          messages: [
            {
              id: 'user-1',
              sessionId: 'session-1',
              threadId: 'session-1',
              role: 'user',
              source: 'ui',
              content: [{ type: 'text', text: 'hello world' }],
            },
            {
              id: 'instruction-1',
              sessionId: 'session-1',
              threadId: 'session-1',
              role: 'user',
              source: 'compaction-instruction',
              content: [
                {
                  type: 'text',
                  text: 'Summarise the previous conversation history using strict compact Markdown.',
                },
              ],
            },
            {
              id: 'assistant-1',
              sessionId: 'session-1',
              threadId: 'session-1',
              role: 'assistant',
              source: 'compact-summary',
              tool_calls: [
                {
                  id: 'tool-1',
                  type: 'function',
                  function: {
                    name: 'workspace__read',
                    arguments: '{"path":"README.md"}',
                  },
                },
              ],
              attachments: [
                {
                  sessionId: 'session-1',
                  filename: 'note.txt',
                  mimeType: 'text/plain',
                  size: 4,
                  lineCount: 1,
                  preview: 'note',
                  uploadedAt: '2026-05-30T00:00:00.000Z',
                  status: 'committed',
                },
              ],
              content: [
                { type: 'text', text: 'tool result' },
                { type: 'thinking', thinking: 'reasoning' },
                {
                  type: 'tool_call',
                  id: 'tool-call-1',
                  name: 'workspace__read',
                  arguments: '{"path":"README.md"}',
                },
              ],
            },
          ],
        }),
      ).toMatchObject({
        messageCount: 3,
        contentPartCount: 5,
        contentTypeCounts: {
          text: 3,
          thinking: 1,
          tool_call: 1,
        },
        textChars: 96,
        thinkingChars: 9,
        contentToolCallArgumentChars: 20,
        assistantToolCallArgumentChars: 20,
        attachmentCount: 1,
        totalMessagePayloadChars: 145,
        averageMessagePayloadChars: 48.333333333333336,
        maxMessagePayloadChars: 74,
        maxMessageContentParts: 3,
        systemPromptLength: 13,
        toolsCount: 1,
        roleCounts: {
          user: 2,
          assistant: 1,
        },
        sourceCounts: {
          ui: 1,
          'compaction-instruction': 1,
          'compact-summary': 1,
        },
        compactSummaryCount: 1,
        compactionInstructionCount: 1,
        externalRequestCount: 1,
        assistantToolCallCount: 1,
        compactionInstruction: {
          included: true,
          placement: 'messages[last-user]',
          messageIndex: 1,
          contentPartIndex: 0,
          role: 'user',
          source: 'compaction-instruction',
          textChars: 74,
          preview:
            'Summarise the previous conversation history using strict compact Markdown.',
        },
      });
    });
  });
});
