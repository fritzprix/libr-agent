/**
 * Unified thinking effort → provider-native parameter mapping.
 *
 * Settings expose a single `thinkingEffort` enum. Known providers are mapped to
 * native API parameters here. Mapping is best-effort and always applied when the
 * user enables an effort level — providers that reject the parameter return an
 * API error the UI surfaces so the user can turn effort off or change models.
 */

import { AIServiceProvider } from './types';

/** User-facing thinking effort preset stored in settings. */
export type ThinkingEffort = 'off' | 'low' | 'medium' | 'high' | 'auto';

export const THINKING_EFFORT_VALUES = [
  'off',
  'low',
  'medium',
  'high',
  'auto',
] as const satisfies readonly ThinkingEffort[];

/** Internal Gemini token budgets — not shown in UI. */
const GEMINI_EFFORT_TOKEN_BUDGET: Record<
  Exclude<ThinkingEffort, 'off'>,
  number
> = {
  low: 1024,
  medium: 8192,
  high: 24576,
  auto: -1,
};

/** Result of mapping thinking effort to provider-native params. */
export interface ThinkingNativeParams {
  /** Whether extended reasoning/thinking is enabled. */
  enabled: boolean;
  /** Provider-native reasoning effort level (OpenAI / Ollama). */
  reasoningEffort?: 'low' | 'medium' | 'high';
  /** Provider-native extended thinking flag (Anthropic). */
  extendedThinking?: boolean;
  /** Provider-native thinking budget (Gemini). */
  thinkingBudget?: number;
}

/**
 * Normalize persisted or legacy values into a thinking effort enum.
 */
export function normalizeThinkingEffort(
  effort: unknown,
  legacyBudget?: unknown,
): ThinkingEffort {
  if (
    effort === 'off' ||
    effort === 'low' ||
    effort === 'medium' ||
    effort === 'high' ||
    effort === 'auto'
  ) {
    return effort;
  }

  if (typeof legacyBudget === 'number') {
    if (legacyBudget === 0) return 'off';
    if (legacyBudget === -1) return 'auto';
    if (legacyBudget <= 2048) return 'low';
    if (legacyBudget <= 16384) return 'medium';
    return 'high';
  }

  return 'off';
}

/**
 * Map unified thinking effort to provider-specific native parameters.
 */
export function mapThinkingEffort(
  provider: AIServiceProvider,
  effort: ThinkingEffort | undefined,
): ThinkingNativeParams {
  const normalized = effort ?? 'off';
  if (normalized === 'off') {
    return { enabled: false };
  }

  switch (provider) {
    case AIServiceProvider.OpenAI:
    case AIServiceProvider.Fireworks:
    case AIServiceProvider.Cerebras:
    case AIServiceProvider.OpenRouter:
    case AIServiceProvider.Ollama:
      return {
        enabled: true,
        reasoningEffort: normalized === 'auto' ? 'medium' : normalized,
      };

    case AIServiceProvider.Groq:
      // Groq uses reasoning_format (parsed) rather than effort levels.
      return { enabled: true };

    case AIServiceProvider.Anthropic:
      return { enabled: true, extendedThinking: true };

    case AIServiceProvider.Gemini:
      return {
        enabled: true,
        thinkingBudget: GEMINI_EFFORT_TOKEN_BUDGET[normalized],
      };

    case AIServiceProvider.Empty:
    default:
      return { enabled: false };
  }
}
