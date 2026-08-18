import { AIServiceProvider, type AIServiceConfig } from '../types';
import { isOfficialOpenAIEndpoint } from './prompt-cache';

/** Default first-person thinking continuation from llama.cpp (ggerganov). */
export const DEFAULT_REASONING_BUDGET_MESSAGE =
  '... I am thinking for too long -- let me gather more info about the task.';

/** Upper bound persisted on custom OpenAI providers (tokens, not characters). */
export const MAX_REASONING_BUDGET_TOKENS = 1_000_000;

export interface OpenAICompatibleReasoningBudgetFields {
  reasoning_budget_tokens: number;
  thinking_budget_tokens: number;
  reasoning_budget_message: string;
}

/**
 * Approximate thinking tokens from streamed text.
 * Character/4 is conservative vs cl100k and typical Qwen/DeepSeek tokenizers.
 */
export function estimateThinkingTokens(text: string): number {
  if (!text) {
    return 0;
  }
  return Math.ceil(text.length / 4);
}

export function normalizeReasoningBudgetTokens(
  value: number | undefined | null,
): number | undefined {
  if (value == null || !Number.isFinite(value) || value < 1) {
    return undefined;
  }
  const tokens = Math.floor(value);
  if (tokens <= 0) {
    return undefined;
  }
  return Math.min(tokens, MAX_REASONING_BUDGET_TOKENS);
}

/** Parse settings UI input: positive integers only (rejects decimals like 0.5). */
export function parseReasoningBudgetInput(raw: string): number | undefined {
  const trimmed = raw.trim();
  if (trimmed === '') {
    return undefined;
  }
  if (!/^[1-9]\d*$/.test(trimmed)) {
    return undefined;
  }
  const parsed = Number(trimmed);
  return Math.min(parsed, MAX_REASONING_BUDGET_TOKENS);
}

export function normalizeReasoningBudgetMessage(
  value: string | undefined | null,
): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

/**
 * llama.cpp-style top-level request fields for OpenAI-compatible servers.
 *
 * Only returned when the caller opted in (`sendNativeReasoningBudget`) on a
 * third-party endpoint. Official OpenAI rejects unknown keys with HTTP 400.
 */
export function buildOpenAICompatibleReasoningBudgetFields(
  provider: AIServiceProvider,
  config: AIServiceConfig,
): OpenAICompatibleReasoningBudgetFields | undefined {
  if (!config.use3rdParty || config.sendNativeReasoningBudget !== true) {
    return undefined;
  }
  if (isOfficialOpenAIEndpoint(provider, config)) {
    return undefined;
  }

  const tokens = normalizeReasoningBudgetTokens(config.reasoningBudget);
  if (!tokens) {
    return undefined;
  }

  return {
    reasoning_budget_tokens: tokens,
    thinking_budget_tokens: tokens,
    reasoning_budget_message:
      normalizeReasoningBudgetMessage(config.reasoningBudgetMessage) ??
      DEFAULT_REASONING_BUDGET_MESSAGE,
  };
}

export function withOpenAICompatibleReasoningBudget<T extends object>(
  request: T,
  provider: AIServiceProvider,
  config: AIServiceConfig,
): T {
  const fields = buildOpenAICompatibleReasoningBudgetFields(provider, config);
  if (!fields) {
    return request;
  }
  return { ...request, ...fields };
}
