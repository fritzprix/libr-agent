import { describe, expect, it } from 'vitest';
import {
  estimateNonToolOutputTokens,
  estimateOutputBudgetTokens,
  estimateThinkingTokens,
  providerSupportsReasoningBudgetCap,
  reasoningBudgetThresholdTokens,
  REASONING_BUDGET_MAX_TOKENS_RATIO,
} from '../reasoning-budget';

describe('reasoning-budget', () => {
  it('uses 90% of maxTokens as the retry threshold', () => {
    expect(REASONING_BUDGET_MAX_TOKENS_RATIO).toBe(0.9);
    expect(reasoningBudgetThresholdTokens(8192)).toBe(7372);
    expect(reasoningBudgetThresholdTokens(1)).toBe(1);
    expect(reasoningBudgetThresholdTokens(0)).toBe(1);
  });

  it('estimates thinking tokens conservatively from character length', () => {
    expect(estimateThinkingTokens('')).toBe(0);
    expect(estimateThinkingTokens('abcd')).toBe(1);
    expect(estimateThinkingTokens('abcde')).toBe(2);
  });

  it('counts overlapping thinking/content channels once', () => {
    const dump = 'a'.repeat(40);
    expect(estimateNonToolOutputTokens(dump, dump)).toBe(
      estimateThinkingTokens(dump),
    );
    expect(estimateNonToolOutputTokens(dump, dump.slice(0, 10))).toBe(
      estimateThinkingTokens(dump),
    );
    expect(estimateNonToolOutputTokens('abcd', 'efgh')).toBe(2);
  });

  it('prefers provider completion_tokens when denser than chars/4', () => {
    expect(
      estimateOutputBudgetTokens({
        thinkingText: '',
        contentText: 'short',
        completionTokens: 30000,
      }),
    ).toBe(30000);
    expect(
      estimateOutputBudgetTokens({
        thinkingText: 'x'.repeat(400),
        contentText: '',
        completionTokens: 10,
      }),
    ).toBe(100);
  });

  it('enables the cap only for builtin and custom OpenAI providers', () => {
    expect(providerSupportsReasoningBudgetCap('openai')).toBe(true);
    expect(providerSupportsReasoningBudgetCap('custom:local-qwen')).toBe(true);
    expect(providerSupportsReasoningBudgetCap('gemini')).toBe(false);
    expect(providerSupportsReasoningBudgetCap('anthropic')).toBe(false);
    expect(providerSupportsReasoningBudgetCap('ollama')).toBe(false);
  });
});
