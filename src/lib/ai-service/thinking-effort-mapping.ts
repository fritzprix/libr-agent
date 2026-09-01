/**
 * Unified thinking budget → provider-native parameter mapping.
 *
 * All provider services import from this module instead of reading
 * `enableReasoning` / `reasoningEffort` directly from `AIServiceConfig`.
 *
 * Budget semantics (set once in `AdvancedSettings.thinkingBudget`):
 * - `0` or `undefined`: disabled
 * - `-1`: dynamic — model auto-adjusts
 * - `> 0`: explicit token budget
 *
 * Provider-native mapping:
 * | Provider   | Native param                    | Budget range                    |
 * |------------|--------------------------------|--------------------------------|
 * | OpenAI     | `reasoning_effort`             | low=1024, med=8192, high=24576 |
 * | Anthropic  | `extended_thinking: true`      | any > 0                        |
 * | Gemini     | `thinkingConfig.thinkingBudget`| direct pass-through            |
 * | Ollama     | `think`                        | low=1024, med=8192, high=24576 |
 */

import { AIServiceProvider } from './types';

/** Result of mapping a thinking budget to provider-native params. */
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
 * Map a unified thinking budget to provider-specific native parameters.
 * @param provider The AI service provider.
 * @param budget The thinking budget from `AIServiceConfig.thinkingBudget`.
 * @returns Native parameter set for the provider.
 */
export function mapThinkingBudget(
  provider: AIServiceProvider,
  budget: number | undefined,
): ThinkingNativeParams {
  // Disabled
  if (budget === undefined || budget === 0) {
    return { enabled: false };
  }

  // Dynamic mode — provider-native mapping still applies
  if (budget === -1) {
    switch (provider) {
      case AIServiceProvider.OpenAI:
      case AIServiceProvider.Fireworks:
      case AIServiceProvider.Cerebras:
      case AIServiceProvider.OpenRouter:
        return { enabled: true, reasoningEffort: 'medium' };

      case AIServiceProvider.Anthropic:
        return { enabled: true, extendedThinking: true };

      case AIServiceProvider.Gemini:
        return { enabled: true, thinkingBudget: -1 };

      case AIServiceProvider.Ollama:
        return { enabled: true, reasoningEffort: 'medium' };

      case AIServiceProvider.Groq:
      case AIServiceProvider.Empty:
      default:
        return { enabled: false };
    }
  }

  // Explicit budget — derive effort level from token range
  const effort = deriveEffortLevel(budget);

  switch (provider) {
    case AIServiceProvider.OpenAI:
    case AIServiceProvider.Fireworks:
    case AIServiceProvider.Cerebras:
    case AIServiceProvider.OpenRouter:
      return { enabled: true, reasoningEffort: effort };

    case AIServiceProvider.Anthropic:
      return { enabled: true, extendedThinking: true };

    case AIServiceProvider.Gemini:
      return { enabled: true, thinkingBudget: budget };

    case AIServiceProvider.Ollama:
      return { enabled: true, reasoningEffort: effort };

    case AIServiceProvider.Groq:
    case AIServiceProvider.Empty:
    default:
      // Providers that don't support thinking — return enabled=false
      return { enabled: false };
  }
}

/**
 * Derive an effort level from a token budget value.
 * @internal
 */
function deriveEffortLevel(budget: number): 'low' | 'medium' | 'high' {
  if (budget <= 2048) return 'low';
  if (budget <= 16384) return 'medium';
  return 'high';
}
