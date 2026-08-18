import { describe, expect, it } from 'vitest';

import { AIServiceProvider } from '../../types';
import {
  DEFAULT_REASONING_BUDGET_MESSAGE,
  MAX_REASONING_BUDGET_TOKENS,
  buildOpenAICompatibleReasoningBudgetFields,
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

  it('trims optional budget messages', () => {
    expect(normalizeReasoningBudgetMessage(undefined)).toBeUndefined();
    expect(normalizeReasoningBudgetMessage('   ')).toBeUndefined();
    expect(normalizeReasoningBudgetMessage('  wrap up  ')).toBe('wrap up');
  });

  it('does not emit native fields for official OpenAI', () => {
    expect(
      buildOpenAICompatibleReasoningBudgetFields(AIServiceProvider.OpenAI, {
        use3rdParty: true,
        sendNativeReasoningBudget: true,
        reasoningBudget: 512,
      }),
    ).toBeUndefined();

    expect(
      buildOpenAICompatibleReasoningBudgetFields(AIServiceProvider.OpenAI, {
        baseUrl: 'https://api.openai.com/v1',
        use3rdParty: true,
        sendNativeReasoningBudget: true,
        reasoningBudget: 512,
      }),
    ).toBeUndefined();
  });

  it('keeps native llama.cpp fields off unless the caller opts in', () => {
    expect(
      buildOpenAICompatibleReasoningBudgetFields(AIServiceProvider.OpenAI, {
        baseUrl: 'http://127.0.0.1:8080/v1',
        use3rdParty: true,
        reasoningBudget: 512,
      }),
    ).toBeUndefined();
  });

  it('emits native fields for opted-in third-party OpenAI-compatible hosts', () => {
    expect(
      buildOpenAICompatibleReasoningBudgetFields(AIServiceProvider.OpenAI, {
        baseUrl: 'http://127.0.0.1:8080/v1',
        use3rdParty: true,
        sendNativeReasoningBudget: true,
        reasoningBudget: 512,
      }),
    ).toEqual({
      reasoning_budget_tokens: 512,
      thinking_budget_tokens: 512,
      reasoning_budget_message: DEFAULT_REASONING_BUDGET_MESSAGE,
    });
  });
});
