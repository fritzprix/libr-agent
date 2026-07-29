import type { ModelInfo } from '@/lib/llm-config-manager';

const PERSISTENT_MODEL_CACHE_PREFIX = 'libragent:models:cache:';

/**
 * Reads persisted dynamic model list for a given provider from localStorage.
 */
export function getStoredModelCache(
  provider: string,
): Record<string, ModelInfo> | null {
  if (!provider) return null;
  try {
    if (typeof localStorage === 'undefined') return null;
    const raw = localStorage.getItem(
      `${PERSISTENT_MODEL_CACHE_PREFIX}${provider}`,
    );
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Record<string, ModelInfo>;
    if (
      parsed &&
      typeof parsed === 'object' &&
      Object.keys(parsed).length > 0
    ) {
      return parsed;
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * Persists dynamic model list for a provider into localStorage.
 */
export function setStoredModelCache(
  provider: string,
  models: Record<string, ModelInfo>,
): void {
  if (!provider || !models) return;
  try {
    if (typeof localStorage === 'undefined') return;
    if (Object.keys(models).length > 0) {
      localStorage.setItem(
        `${PERSISTENT_MODEL_CACHE_PREFIX}${provider}`,
        JSON.stringify(models),
      );
    }
  } catch {
    // Ignore quota errors
  }
}

/**
 * Clears stored model cache for a provider or all providers.
 */
export function clearStoredModelCache(provider?: string): void {
  try {
    if (typeof localStorage === 'undefined') return;
    if (provider) {
      localStorage.removeItem(`${PERSISTENT_MODEL_CACHE_PREFIX}${provider}`);
    } else {
      const keysToRemove: string[] = [];
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key && key.startsWith(PERSISTENT_MODEL_CACHE_PREFIX)) {
          keysToRemove.push(key);
        }
      }
      for (const k of keysToRemove) {
        localStorage.removeItem(k);
      }
    }
  } catch {
    // Ignore storage errors
  }
}
