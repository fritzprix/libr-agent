import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { MutableRefObject } from 'react';

import { DEFAULT_REASONING_BUDGET_MESSAGE } from '@/lib/ai-service/openai/reasoning-budget';
import { DEFAULT_SETTING, type Settings } from '@/lib/services/settings-service';
import { StreamAccumulator } from '../execute-completion/stream-accumulator';

const { reportLLMStreamingIssue } = vi.hoisted(() => ({
  reportLLMStreamingIssue: vi.fn().mockResolvedValue({ success: true }),
}));

vi.mock('@/lib/backend/agent-commands', () => ({
  reportLLMStreamingIssue,
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const settingsRef: MutableRefObject<Settings> = {
  current: DEFAULT_SETTING,
};

function createAccumulator(options?: {
  reasoningBudget?: number;
  reasoningBudgetMessage?: string;
}) {
  return new StreamAccumulator(
    'session-1',
    'response-1',
    settingsRef,
    Date.now(),
    options,
  );
}

describe('StreamAccumulator reasoning budget', () => {
  beforeEach(() => {
    reportLLMStreamingIssue.mockClear();
  });

  it('reports REASONING_BUDGET_EXCEEDED once when thinking reaches the cap', () => {
    const accumulator = createAccumulator({
      reasoningBudget: 2,
      reasoningBudgetMessage: 'stop now',
    });

    accumulator.processChunk({ thinking: 'abcdefgh' });
    accumulator.processChunk({ thinking: 'ijkl' });

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith({
      sessionId: 'session-1',
      responseMessageId: 'response-1',
      issueKind: 'REASONING_BUDGET_EXCEEDED',
      observedTailChars: 8,
      patternLength: 2,
      repetitionCount: 2,
      reasoningBudgetTokens: 2,
      estimatedThinkingTokens: 2,
      nudgeMessage: 'stop now',
    });
  });

  it('uses the default nudge when no custom message is stored', () => {
    const accumulator = createAccumulator({ reasoningBudget: 2 });

    accumulator.processChunk({ thinking: 'abcdefgh' });

    expect(reportLLMStreamingIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        issueKind: 'REASONING_BUDGET_EXCEEDED',
        nudgeMessage: DEFAULT_REASONING_BUDGET_MESSAGE,
      }),
    );
  });

  it('skips the budget abort when a tool call already appeared in the stream', () => {
    const accumulator = createAccumulator({ reasoningBudget: 2 });

    accumulator.processChunk({
      tool_calls: [
        {
          index: 0,
          id: 'call_1',
          type: 'function',
          function: { name: 'workspace__read', arguments: '{}' },
        },
      ],
    });
    accumulator.processChunk({ thinking: 'abcdefgh' });

    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
  });

  it('does not report when no reasoning budget is configured', () => {
    const accumulator = createAccumulator();

    accumulator.processChunk({ thinking: 'abcdefghijklmnop' });

    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
  });
});
