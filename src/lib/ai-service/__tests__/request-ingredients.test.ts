import { describe, expect, it } from 'vitest';

import {
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
});
