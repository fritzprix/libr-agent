import { beforeEach, describe, expect, it, vi } from 'vitest';
import { StreamAccumulator } from '../execute-completion/stream-accumulator';
import type { Settings } from '@/lib/services/settings-service';
import { estimateThinkingTokens } from '@/lib/ai-service/openai/reasoning-budget';

const reportLLMStreamingIssue = vi.fn().mockResolvedValue(undefined);

vi.mock('@/lib/backend/agent-commands', () => ({
  reportLLMStreamingIssue: (...args: unknown[]) =>
    reportLLMStreamingIssue(...args),
}));

function createAccumulator(maxTokens: number): StreamAccumulator {
  const settingsRef = {
    current: { advanced: {} } as Settings,
  };
  return new StreamAccumulator(
    'session-1',
    'response-1',
    settingsRef,
    performance.now(),
    { reasoningBudgetMaxTokens: maxTokens },
  );
}

describe('StreamAccumulator reasoning budget', () => {
  beforeEach(() => {
    reportLLMStreamingIssue.mockClear();
  });

  it('reports REASONING_BUDGET_EXCEEDED once when thinking reaches 90% of maxTokens', () => {
    const maxTokens = 40;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);
    const thinking = 'x'.repeat(threshold * 4);

    accumulator.processChunk({ thinking });

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith({
      sessionId: 'session-1',
      responseMessageId: 'response-1',
      issueKind: 'REASONING_BUDGET_EXCEEDED',
      observedTailChars: thinking.length,
      patternLength: threshold,
      repetitionCount: estimateThinkingTokens(thinking),
    });

    accumulator.processChunk({ thinking: 'more' });
    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
  });

  it('does not report when reasoningBudgetMaxTokens is unset', () => {
    const settingsRef = {
      current: { advanced: {} } as Settings,
    };
    const accumulator = new StreamAccumulator(
      'session-1',
      'response-1',
      settingsRef,
      performance.now(),
    );

    accumulator.processChunk({ thinking: 'x'.repeat(100_000) });
    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
  });
});
