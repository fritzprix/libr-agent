import { beforeEach, describe, expect, it, vi } from 'vitest';
import { StreamAccumulator } from '../execute-completion/stream-accumulator';
import type { Settings } from '@/lib/services/settings-service';
import {
  estimateOutputBudgetTokens,
  REASONING_BUDGET_CHARS_PER_TOKEN,
} from '@/lib/ai-service/openai/reasoning-budget';

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

function charsForThresholdTokens(tokenThreshold: number): number {
  return tokenThreshold * REASONING_BUDGET_CHARS_PER_TOKEN;
}

describe('StreamAccumulator reasoning/output budget', () => {
  beforeEach(() => {
    reportLLMStreamingIssue.mockClear();
  });

  it('reports REASONING_BUDGET_EXCEEDED once when thinking reaches 90% of maxTokens', () => {
    const maxTokens = 40;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);
    const thinking = 'x'.repeat(charsForThresholdTokens(threshold));

    const first = accumulator.processChunk({ thinking });

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith({
      sessionId: 'session-1',
      responseMessageId: 'response-1',
      issueKind: 'REASONING_BUDGET_EXCEEDED',
      observedTailChars: thinking.length,
      patternLength: threshold,
      repetitionCount: estimateOutputBudgetTokens({
        thinkingText: thinking,
        contentText: '',
      }),
    });
    expect(first.shouldAbortStream).toBe(true);
    expect(accumulator.didExceedReasoningBudget()).toBe(true);

    const second = accumulator.processChunk({ thinking: 'more' });
    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(second.shouldAbortStream).toBe(true);
  });

  it('reports when assistant content (no thinking) reaches 90% with no tool calls', () => {
    const maxTokens = 40;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);
    const content = 'y'.repeat(charsForThresholdTokens(threshold));

    const result = accumulator.processChunk({ content });

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        issueKind: 'REASONING_BUDGET_EXCEEDED',
        patternLength: threshold,
        observedTailChars: content.length,
      }),
    );
    expect(result.shouldAbortStream).toBe(true);
    expect(accumulator.didExceedReasoningBudget()).toBe(true);
  });

  it('reports from provider completion_tokens when the char estimate underestimates', () => {
    const maxTokens = 100;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);

    // Short content would not trip the char estimate, but usage does.
    const early = accumulator.processChunk({ content: 'short analysis' });
    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
    expect(early.shouldAbortStream).toBe(false);

    const withUsage = accumulator.processChunk({
      usage: {
        promptTokens: 10,
        completionTokens: threshold,
        totalTokens: 10 + threshold,
      },
    });

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        issueKind: 'REASONING_BUDGET_EXCEEDED',
        patternLength: threshold,
        repetitionCount: threshold,
      }),
    );
    expect(withUsage.shouldAbortStream).toBe(true);
    expect(accumulator.didExceedReasoningBudget()).toBe(true);
  });

  it('does not report when a tool call is present even if content is huge', () => {
    const maxTokens = 40;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);

    accumulator.processChunk({
      tool_calls: [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'workspace__runShell', arguments: '{}' },
        },
      ],
    });
    const huge = accumulator.processChunk({
      content: 'z'.repeat(charsForThresholdTokens(threshold)),
    });
    const withUsage = accumulator.processChunk({
      usage: {
        promptTokens: 1,
        completionTokens: threshold,
        totalTokens: 1 + threshold,
      },
    });

    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
    expect(huge.shouldAbortStream).toBe(false);
    expect(withUsage.shouldAbortStream).toBe(false);
    expect(accumulator.didExceedReasoningBudget()).toBe(false);
  });

  it('finalizeOutputBudgetCheck reports after stream when usage already set', () => {
    const maxTokens = 50;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);

    const withUsage = accumulator.processChunk({
      usage: {
        promptTokens: 5,
        completionTokens: threshold,
        totalTokens: 5 + threshold,
      },
    });
    // Usage path may already report; clear and ensure finalize is idempotent.
    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(withUsage.shouldAbortStream).toBe(true);
    expect(accumulator.finalizeOutputBudgetCheck()).toBe(false);
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

    const thinking = accumulator.processChunk({
      thinking: 'x'.repeat(100_000),
    });
    const content = accumulator.processChunk({ content: 'y'.repeat(100_000) });
    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
    expect(thinking.shouldAbortStream).toBe(false);
    expect(content.shouldAbortStream).toBe(false);
  });
});
