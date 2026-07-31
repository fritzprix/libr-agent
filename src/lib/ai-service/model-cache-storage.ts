import type { ModelInfo } from '@/lib/llm-config-manager';

const PERSISTENT_MODEL_CACHE_PREFIX = 'libragent:models:cache:';

const memoryCache = new Map<string, Record<string, ModelInfo>>();

function getLocalStorage(): Storage | null {
  try {
    if (
      typeof window !== 'undefined' &&
      window.localStorage &&
      typeof window.localStorage.setItem === 'function'
    ) {
      return window.localStorage;
    }
    if (
      typeof localStorage !== 'undefined' &&
      typeof localStorage.setItem === 'function'
    ) {
      return localStorage;
    }
  } catch {
    // Ignore error
  }
  return null;
}

/**
 * Reads persisted dynamic model list for a given provider from localStorage or memory cache.
 */
export function getStoredModelCache(
  provider: string,
): Record<string, ModelInfo> | null {
  if (!provider) return null;
  try {
    const storage = getLocalStorage();
    if (storage) {
      const raw = storage.getItem(
        `${PERSISTENT_MODEL_CACHE_PREFIX}${provider}`,
      );
      if (raw) {
        const parsed = JSON.parse(raw) as Record<string, ModelInfo>;
        if (
          parsed &&
          typeof parsed === 'object' &&
          Object.keys(parsed).length > 0
        ) {
          return parsed;
        }
      }
    }
  } catch {
    // Fall through to memoryCache
  }
  const mem = memoryCache.get(provider);
  return mem && Object.keys(mem).length > 0 ? mem : null;
}

/**
 * Persists dynamic model list for a provider into localStorage and memory cache.
 */
export function setStoredModelCache(
  provider: string,
  models: Record<string, ModelInfo>,
): void {
  if (!provider || !models) return;
  if (Object.keys(models).length > 0) {
    memoryCache.set(provider, models);
  }
  try {
    const storage = getLocalStorage();
    if (storage && Object.keys(models).length > 0) {
      storage.setItem(
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
  if (provider) {
    memoryCache.delete(provider);
  } else {
    memoryCache.clear();
  }
  try {
    const storage = getLocalStorage();
    if (storage) {
      if (provider) {
        storage.removeItem(`${PERSISTENT_MODEL_CACHE_PREFIX}${provider}`);
      } else {
        const keysToRemove: string[] = [];
        for (let i = 0; i < storage.length; i++) {
          const key = storage.key(i);
          if (key && key.startsWith(PERSISTENT_MODEL_CACHE_PREFIX)) {
            keysToRemove.push(key);
          }
        }
        for (const k of keysToRemove) {
          storage.removeItem(k);
        }
      }
    }
  } catch {
    // Ignore storage errors
  }
}
