export function calculateEffectiveContextLimit(
  modelInfo: { contextWindow?: number } | null,
  maxOutputTokens: number,
  maxInputContext?: number,
): { effectiveLimit: number; modelMaxLimit: number } {
  const defaultContextWindow = 64 * 1024;
  const modelMaxLimit = modelInfo?.contextWindow || defaultContextWindow;

  let safeInputLimit = modelMaxLimit;
  const reserved = maxOutputTokens + 100; // Reserve output tokens + safety buffer
  if (reserved < modelMaxLimit) {
    safeInputLimit = modelMaxLimit - reserved;
  }

  const effectiveLimit =
    maxInputContext != null && maxInputContext > 0 && maxInputContext < safeInputLimit
      ? maxInputContext
      : safeInputLimit;

  return { effectiveLimit, modelMaxLimit };
}

export function calculateCompactThreshold(effectiveLimit: number): number {
  return Math.floor(effectiveLimit * 0.9);
}

/** Minimal shape required by findCompactionSplitIndex */
interface HasId {
  id: string;
}

/**
 * Finds the index at which to split the message stack for compaction.
 * Messages before this index will be compacted (summarized).
 * Messages from this index onwards will be kept as "recent context".
 */
export function findCompactionSplitIndex<T extends HasId>(
  messages: T[],
  estimateTokens: (m: T) => number,
  threshold: number,
  systemPromptTokens: number,
  toolsTokens: number,
): number {
  // How much room is actually available for messages?
  const messageBudget = Math.max(
    0,
    threshold - systemPromptTokens - toolsTokens,
  );
  // Keep the most recent messages up to 50% of the available budget, or at least 1000 tokens
  const keepThreshold = Math.max(1000, messageBudget * 0.5);

  let currentSum = 0;
  let splitIdx = 0;
  let splitFound = false;

  for (let i = messages.length - 1; i >= 0; i--) {
    currentSum += estimateTokens(messages[i]);
    if (currentSum >= keepThreshold) {
      splitIdx = i;
      splitFound = true;
      break;
    }
  }

  // Fallback: If no split point was found within the keep budget, force a
  // half-split when the list is large enough.
  // Using >= 10 ensures splitIdx is at least 5.
  if (!splitFound && messages.length >= 10) {
    splitIdx = Math.floor(messages.length / 2);
  }

  return splitIdx;
}

/**
 * Strips the `compact-summary-{fromId}~{toId}` prefix from a synthetic
 * summary message ID, returning the original base message ID.
 *
 * This is needed in two places:
 *  1. `buildCandidateStack` — when rendering the display ID for a new summary
 *  2. The async compaction IIFE — when computing `fromId` for the new cache entry
 *
 * @example
 * stripCompactSummaryPrefix('compact-summary-msg_001~msg_050') // → 'msg_001'
 * stripCompactSummaryPrefix('msg_001')                         // → 'msg_001'
 */
export function stripCompactSummaryPrefix(id: string): string {
  if (!id.startsWith('compact-summary-')) return id;
  return id.replace('compact-summary-', '').split('~')[0];
}
