import { describe, expect, it } from 'vitest';

import {
  DEFAULT_REASONING_BUDGET_MESSAGE,
  MAX_REASONING_BUDGET_TOKENS,
  estimateThinkingTokens,
  normalizeReasoningBudgetMessage,
  normalizeReasoningBudgetTokens,
  parseReasoningBudgetInput,
} from '../reasoning-budget';

describe('reasoning budget helpers', () => {
  it('estimates thinking tokens from character length', () => {
    expect(estimateThinkingTokens('')).toBe(0);
    expect(estimateThinkingTokens('abcd')).toBe(1);
    expect(estimateThinkingTokens('abcde')).toBe(2);
  });

  it('normalizes token budgets to a positive integer cap', () => {
    expect(normalizeReasoningBudgetTokens(undefined)).toBeUndefined();
    expect(normalizeReasoningBudgetTokens(Number.NaN)).toBeUndefined();
    expect(normalizeReasoningBudgetTokens(0)).toBeUndefined();
    expect(normalizeReasoningBudgetTokens(0.5)).toBeUndefined();
    expect(normalizeReasoningBudgetTokens(-8)).toBeUndefined();
    expect(normalizeReasoningBudgetTokens(512.9)).toBe(512);
    expect(normalizeReasoningBudgetTokens(MAX_REASONING_BUDGET_TOKENS + 1)).toBe(
      MAX_REASONING_BUDGET_TOKENS,
    );
  });

  it('parses settings input as positive integers only', () => {
    expect(parseReasoningBudgetInput('')).toBeUndefined();
    expect(parseReasoningBudgetInput('0')).toBeUndefined();
    expect(parseReasoningBudgetInput('0.5')).toBeUndefined();
    expect(parseReasoningBudgetInput('512.9')).toBeUndefined();
    expect(parseReasoningBudgetInput('512')).toBe(512);
  });

  it('trims optional budget messages and exports default message', () => {
    expect(normalizeReasoningBudgetMessage(undefined)).toBeUndefined();
    expect(normalizeReasoningBudgetMessage('   ')).toBeUndefined();
    expect(normalizeReasoningBudgetMessage('  wrap up  ')).toBe('wrap up');
    expect(typeof DEFAULT_REASONING_BUDGET_MESSAGE).toBe('string');
  });
});
