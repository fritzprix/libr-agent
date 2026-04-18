interface DisplayMediaCacheEntry {
  url: string;
  size: number;
}

interface SessionDisplayMediaCache {
  entries: Map<string, DisplayMediaCacheEntry>;
  totalBytes: number;
  mountedRenderers: number;
}

const DISPLAY_MEDIA_CACHE_MAX_BYTES = 64 * 1024 * 1024;
const displayMediaCaches = new Map<string, SessionDisplayMediaCache>();

export function estimateBase64Bytes(value: string): number {
  return Math.floor((value.length * 3) / 4);
}

function getSessionDisplayMediaCache(
  sessionId: string,
): SessionDisplayMediaCache {
  const existing = displayMediaCaches.get(sessionId);
  if (existing) {
    return existing;
  }

  const created: SessionDisplayMediaCache = {
    entries: new Map(),
    totalBytes: 0,
    mountedRenderers: 0,
  };
  displayMediaCaches.set(sessionId, created);
  return created;
}

function pruneDisplayMediaCache(
  sessionCache: SessionDisplayMediaCache,
  maxBytes: number,
): void {
  while (sessionCache.totalBytes > maxBytes && sessionCache.entries.size > 0) {
    const oldestKey = sessionCache.entries.keys().next().value;
    if (!oldestKey) {
      break;
    }

    const entry = sessionCache.entries.get(oldestKey);
    if (!entry) {
      sessionCache.entries.delete(oldestKey);
      continue;
    }

    sessionCache.totalBytes -= entry.size;
    sessionCache.entries.delete(oldestKey);
  }
}

export function getDisplayMediaCacheEntry(
  sessionId: string,
  uri: string,
): DisplayMediaCacheEntry | undefined {
  return displayMediaCaches.get(sessionId)?.entries.get(uri);
}

export function touchDisplayMediaCacheEntry(
  sessionId: string,
  uri: string,
): DisplayMediaCacheEntry | undefined {
  const sessionCache = displayMediaCaches.get(sessionId);
  if (!sessionCache) {
    return undefined;
  }

  const cached = sessionCache.entries.get(uri);
  if (!cached) {
    return undefined;
  }

  sessionCache.entries.delete(uri);
  sessionCache.entries.set(uri, cached);
  return cached;
}

export function updateDisplayMediaCache(
  sessionId: string,
  uri: string,
  url: string,
  size: number,
): void {
  const sessionCache = getSessionDisplayMediaCache(sessionId);
  const existing = sessionCache.entries.get(uri);
  if (existing) {
    sessionCache.totalBytes -= existing.size;
    sessionCache.entries.delete(uri);
  }

  sessionCache.entries.set(uri, { url, size });
  sessionCache.totalBytes += size;
  pruneDisplayMediaCache(sessionCache, DISPLAY_MEDIA_CACHE_MAX_BYTES);
}

export function retainDisplayMediaCacheSession(sessionId: string): void {
  const sessionCache = getSessionDisplayMediaCache(sessionId);
  sessionCache.mountedRenderers += 1;
}

export function releaseDisplayMediaCacheSession(sessionId: string): void {
  const sessionCache = displayMediaCaches.get(sessionId);
  if (!sessionCache) {
    return;
  }

  sessionCache.mountedRenderers = Math.max(
    0,
    sessionCache.mountedRenderers - 1,
  );
  if (sessionCache.mountedRenderers === 0) {
    displayMediaCaches.delete(sessionId);
  }
}
