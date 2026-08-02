const STORAGE_PREFIX = 'libragent:models:last-selected:';

const memoryCache = new Map<string, string>();

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
 * Returns the last model id the user configured for a provider, if any.
 */
export function getLastSelectedModel(provider: string): string | null {
  if (!provider) {
    return null;
  }

  try {
    const storage = getLocalStorage();
    if (storage) {
      const raw = storage.getItem(`${STORAGE_PREFIX}${provider}`);
      if (typeof raw === 'string' && raw.trim().length > 0) {
        return raw;
      }
    }
  } catch {
    // Fall through to memory cache
  }

  const mem = memoryCache.get(provider);
  return mem && mem.trim().length > 0 ? mem : null;
}

/**
 * Remembers the model id last configured for a provider.
 */
export function setLastSelectedModel(provider: string, model: string): void {
  if (!provider || !model.trim()) {
    return;
  }

  const normalized = model.trim();
  memoryCache.set(provider, normalized);

  try {
    const storage = getLocalStorage();
    storage?.setItem(`${STORAGE_PREFIX}${provider}`, normalized);
  } catch {
    // Ignore quota / private-mode errors
  }
}

/**
 * Clears last-selected model memory for one provider or all providers.
 */
export function clearLastSelectedModel(provider?: string): void {
  if (provider) {
    memoryCache.delete(provider);
  } else {
    memoryCache.clear();
  }

  try {
    const storage = getLocalStorage();
    if (!storage) {
      return;
    }
    if (provider) {
      storage.removeItem(`${STORAGE_PREFIX}${provider}`);
      return;
    }
    const keysToRemove: string[] = [];
    for (let i = 0; i < storage.length; i++) {
      const key = storage.key(i);
      if (key && key.startsWith(STORAGE_PREFIX)) {
        keysToRemove.push(key);
      }
    }
    for (const key of keysToRemove) {
      storage.removeItem(key);
    }
  } catch {
    // Ignore storage errors
  }
}
