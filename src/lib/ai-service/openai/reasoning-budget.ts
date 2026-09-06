import { isCustomOpenAIProviderId } from '@/lib/ai-service/custom-providers';

/** Fraction of max output tokens used as the abort/retry threshold. */
export const REASONING_BUDGET_MAX_TOKENS_RATIO = 0.9;

/**
 * Approximate tokens from streamed text.
 * Character/4 is only used as a client-side retry threshold, not as a
 * prompt-steering signal. Prefer provider `completion_tokens` when available.
 */
export function estimateThinkingTokens(text: string): number {
  if (!text) {
    return 0;
  }
  return Math.ceil(text.length / 4);
}

/**
 * Estimate non-tool output tokens from thinking and/or assistant content.
 *
 * Some OpenAI-compatible hosts dump long analysis as `content` instead of
 * `reasoning_content`. Overlapping channels (identical or prefix/suffix) are
 * counted once via the longer string so we do not double-count.
 */
export function estimateNonToolOutputTokens(
  thinkingText: string,
  contentText: string,
): number {
  const thinking = thinkingText || '';
  const content = contentText || '';
  if (!thinking) {
    return estimateThinkingTokens(content);
  }
  if (!content) {
    return estimateThinkingTokens(thinking);
  }
  if (
    thinking === content ||
    thinking.startsWith(content) ||
    content.startsWith(thinking)
  ) {
    return estimateThinkingTokens(
      thinking.length >= content.length ? thinking : content,
    );
  }
  return estimateThinkingTokens(thinking) + estimateThinkingTokens(content);
}

/**
 * Combined signal for abort: char estimate and/or provider completion usage.
 * Usage catches hosts whose tokenization is denser than chars/4.
 */
export function estimateOutputBudgetTokens(args: {
  thinkingText: string;
  contentText: string;
  completionTokens?: number | null;
}): number {
  const fromText = estimateNonToolOutputTokens(
    args.thinkingText,
    args.contentText,
  );
  const fromUsage =
    typeof args.completionTokens === 'number' &&
    Number.isFinite(args.completionTokens) &&
    args.completionTokens > 0
      ? Math.floor(args.completionTokens)
      : 0;
  return Math.max(fromText, fromUsage);
}

/** Abort/retry when estimated non-tool output tokens reach this many. */
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
