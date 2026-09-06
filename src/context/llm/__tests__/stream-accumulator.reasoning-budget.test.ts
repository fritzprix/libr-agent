import { beforeEach, describe, expect, it, vi } from 'vitest';
import { StreamAccumulator } from '../execute-completion/stream-accumulator';
import type { Settings } from '@/lib/services/settings-service';
import { estimateOutputBudgetTokens } from '@/lib/ai-service/openai/reasoning-budget';

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

describe('StreamAccumulator reasoning/output budget', () => {
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
      repetitionCount: estimateOutputBudgetTokens({
        thinkingText: thinking,
        contentText: '',
      }),
    });

    accumulator.processChunk({ thinking: 'more' });
    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
  });

  it('reports when assistant content (no thinking) reaches 90% with no tool calls', () => {
    const maxTokens = 40;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);
    const content = 'y'.repeat(threshold * 4);

    accumulator.processChunk({ content });

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        issueKind: 'REASONING_BUDGET_EXCEEDED',
        patternLength: threshold,
        observedTailChars: content.length,
      }),
    );
  });

  it('reports from provider completion_tokens when chars/4 underestimates', () => {
    const maxTokens = 100;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);

    // Short content would not trip chars/4, but usage does.
    accumulator.processChunk({ content: 'short analysis' });
    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();

    accumulator.processChunk({
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
    accumulator.processChunk({ content: 'z'.repeat(threshold * 4) });
    accumulator.processChunk({
      usage: {
        promptTokens: 1,
        completionTokens: threshold,
        totalTokens: 1 + threshold,
      },
    });

    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
  });

  it('finalizeOutputBudgetCheck reports after stream when usage already set', () => {
    const maxTokens = 50;
    const threshold = Math.floor(maxTokens * 0.9);
    const accumulator = createAccumulator(maxTokens);

    accumulator.processChunk({
      usage: {
        promptTokens: 5,
        completionTokens: threshold,
        totalTokens: 5 + threshold,
      },
    });
    // Usage path may already report; clear and ensure finalize is idempotent.
    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
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

    accumulator.processChunk({ thinking: 'x'.repeat(100_000) });
    accumulator.processChunk({ content: 'y'.repeat(100_000) });
    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
  });
});
