export interface GeminiContextCacheEntry {
  name: string;
  createdAt: number;
  lastUsedAt: number;
}

export class GeminiContextCacheStore {
  private readonly entriesByNamespace = new Map<
    string,
    Map<string, GeminiContextCacheEntry>
  >();

  get(
    namespace: string,
    cacheKey: string,
  ): GeminiContextCacheEntry | undefined {
    return this.entriesByNamespace.get(namespace)?.get(cacheKey);
  }

  set(
    namespace: string,
    cacheKey: string,
    entry: GeminiContextCacheEntry,
  ): void {
    const namespaceEntries = this.ensureNamespace(namespace);
    namespaceEntries.set(cacheKey, entry);
  }

  delete(
    namespace: string,
    cacheKey: string,
  ): GeminiContextCacheEntry | undefined {
    const namespaceEntries = this.entriesByNamespace.get(namespace);
    if (!namespaceEntries) {
      return undefined;
    }

    const entry = namespaceEntries.get(cacheKey);
    namespaceEntries.delete(cacheKey);

    if (namespaceEntries.size === 0) {
      this.entriesByNamespace.delete(namespace);
    }

    return entry;
  }

  list(namespace: string): Array<[string, GeminiContextCacheEntry]> {
    return [...(this.entriesByNamespace.get(namespace)?.entries() ?? [])];
  }

  size(namespace: string): number {
    return this.entriesByNamespace.get(namespace)?.size ?? 0;
  }

  clearNamespace(namespace: string): GeminiContextCacheEntry[] {
    const namespaceEntries = this.entriesByNamespace.get(namespace);
    if (!namespaceEntries) {
      return [];
    }

    this.entriesByNamespace.delete(namespace);
    return [...namespaceEntries.values()];
  }

  clearAll(): void {
    this.entriesByNamespace.clear();
  }

  private ensureNamespace(
    namespace: string,
  ): Map<string, GeminiContextCacheEntry> {
    const existingEntries = this.entriesByNamespace.get(namespace);
    if (existingEntries) {
      return existingEntries;
    }

    const nextEntries = new Map<string, GeminiContextCacheEntry>();
    this.entriesByNamespace.set(namespace, nextEntries);
    return nextEntries;
  }
}

const sharedGeminiContextCacheStore = new GeminiContextCacheStore();

export function getSharedGeminiContextCacheStore(): GeminiContextCacheStore {
  return sharedGeminiContextCacheStore;
}
