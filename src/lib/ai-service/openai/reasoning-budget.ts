import { isCustomOpenAIProviderId } from '@/lib/ai-service/custom-providers';

/** Fraction of max output tokens used as the thinking abort/retry threshold. */
export const REASONING_BUDGET_MAX_TOKENS_RATIO = 0.9;

/**
 * Approximate thinking tokens from streamed text.
 * Character/4 is only used as a client-side retry threshold, not as a
 * prompt-steering signal.
 */
export function estimateThinkingTokens(text: string): number {
  if (!text) {
    return 0;
  }
  return Math.ceil(text.length / 4);
}

/** Abort/retry when estimated thinking tokens reach this many. */
export function reasoningBudgetThresholdTokens(maxTokens: number): number {
  if (!Number.isFinite(maxTokens) || maxTokens < 1) {
    return 1;
  }
  return Math.max(1, Math.floor(maxTokens * REASONING_BUDGET_MAX_TOKENS_RATIO));
}

/** Builtin OpenAI + custom OpenAI-compatible providers only. */
export function providerSupportsReasoningBudgetCap(provider: string): boolean {
  return provider === 'openai' || isCustomOpenAIProviderId(provider);
}
