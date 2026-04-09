import { GoogleGenAI, FunctionDeclaration } from '@google/genai';
import { getLogger } from '../../logger';
import { stableHashKeyPart } from '../base-service';
import {
  type GeminiContextCacheEntry,
  getSharedGeminiContextCacheStore,
} from './cache-store';

export class GeminiContextCacheManager {
  private static readonly DEFAULT_MIN_CACHEABLE_PREFIX_TOKENS = 32768;
  private static readonly FLASH_MIN_CACHEABLE_PREFIX_TOKENS = 1024;
  private static readonly PRO_MIN_CACHEABLE_PREFIX_TOKENS = 4096;
  private static readonly MAX_CONTEXT_CACHE_ENTRIES = 8;
  private static readonly CONTEXT_CACHE_TTL_MS = 55 * 60 * 1000;
  private static readonly contextCacheStore =
    getSharedGeminiContextCacheStore();

  private logger = getLogger('GeminiService.contextCacheManager');

  constructor(
    private readonly genAI: GoogleGenAI,
    private readonly cacheNamespace: string,
  ) {}

  static purgeSharedContextCache(apiKey: string): void {
    const cachedEntries =
      GeminiContextCacheManager.contextCacheStore.clearNamespace(apiKey);
    if (cachedEntries.length === 0) {
      return;
    }

    const client = new GoogleGenAI({ apiKey });
    const logger = getLogger('GeminiService.contextCache');

    for (const entry of cachedEntries) {
      void client.caches
        .delete({ name: entry.name })
        .then(() => {
          logger.debug('Deleted Gemini context cache entry', {
            cachedContentName: entry.name,
            reason: 'factory purge',
          });
        })
        .catch((error: unknown) => {
          logger.debug('Failed to delete Gemini context cache entry', {
            cachedContentName: entry.name,
            reason: 'factory purge',
            error,
          });
        });
    }
  }

  static resetSharedContextCacheForTests(): void {
    GeminiContextCacheManager.contextCacheStore.clearAll();
  }

  static getMinimumCacheablePrefixTokensForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();

    if (
      lowerName.includes('gemini-3-flash') ||
      lowerName.includes('gemini-2.5-flash')
    ) {
      return GeminiContextCacheManager.FLASH_MIN_CACHEABLE_PREFIX_TOKENS;
    }

    if (
      lowerName.includes('gemini-3-pro') ||
      lowerName.includes('gemini-2.5-pro')
    ) {
      return GeminiContextCacheManager.PRO_MIN_CACHEABLE_PREFIX_TOKENS;
    }

    return GeminiContextCacheManager.DEFAULT_MIN_CACHEABLE_PREFIX_TOKENS;
  }

  static supportsToolsForModel(modelName: string): boolean {
    const lowerName = modelName.toLowerCase();
    return lowerName.includes('gemini-1.5') || lowerName.includes('gemini-2');
  }

  shouldAttemptContextCache(
    model: string,
    stablePrefix: string,
    toolsPayload: string,
    toolDeclarationCount: number,
  ): boolean {
    if (!GeminiContextCacheManager.supportsToolsForModel(model)) {
      return false;
    }

    const cacheableTokenEstimate = this.estimateCacheablePrefixTokens(
      stablePrefix,
      toolsPayload,
      toolDeclarationCount,
    );
    return (
      cacheableTokenEstimate >=
      GeminiContextCacheManager.getMinimumCacheablePrefixTokensForModel(model)
    );
  }

  estimateCacheablePrefixTokens(
    stablePrefix: string,
    toolsPayload: string,
    toolDeclarationCount: number,
  ): number {
    const encoder = new TextEncoder();
    const stablePrefixBytes = encoder.encode(stablePrefix).length;
    const toolsPayloadBytes = encoder.encode(toolsPayload).length;
    const textTokenEstimate = Math.ceil(stablePrefixBytes / 3.5);
    const structuredTokenEstimate = Math.ceil(toolsPayloadBytes / 2.5);
    const toolDeclarationOverhead = toolDeclarationCount * 32;

    return (
      textTokenEstimate + structuredTokenEstimate + toolDeclarationOverhead
    );
  }

  logPromptCacheMetadata(args: {
    model: string;
    stablePrefix: string;
    toolsPayload: string;
    toolDeclarationCount: number;
    cacheKey?: string;
    canUseCachedContent: boolean;
    requiresToolOverride: boolean;
    shouldUseCache: boolean;
    cachedContentName?: string;
  }): void {
    const cacheableTokenEstimate = this.estimateCacheablePrefixTokens(
      args.stablePrefix,
      args.toolsPayload,
      args.toolDeclarationCount,
    );
    const minCacheablePrefixTokens =
      GeminiContextCacheManager.getMinimumCacheablePrefixTokensForModel(
        args.model,
      );
    const encoder = new TextEncoder();
    const stablePrefixBytes = encoder.encode(args.stablePrefix).length;
    const toolsPayloadBytes = encoder.encode(args.toolsPayload).length;

    this.logger.info('Gemini prompt cache metadata', {
      model: args.model,
      cacheKey: args.cacheKey,
      stablePrefixHash: stableHashKeyPart(args.stablePrefix),
      toolsHash: stableHashKeyPart(args.toolsPayload),
      stablePrefixLength: args.stablePrefix.length,
      stablePrefixBytes,
      toolsPayloadLength: args.toolsPayload.length,
      toolsPayloadBytes,
      toolDeclarationCount: args.toolDeclarationCount,
      cacheableTokenEstimate,
      minCacheablePrefixTokens,
      canUseCachedContent: args.canUseCachedContent,
      requiresToolOverride: args.requiresToolOverride,
      shouldUseCache: args.shouldUseCache,
      cachedContentName: args.cachedContentName,
      cacheHit: Boolean(args.cachedContentName),
    });
  }

  createContextCacheKey(
    model: string,
    stablePrefix: string,
    toolsPayload: string,
  ): string {
    return [
      model,
      stableHashKeyPart(stablePrefix),
      stableHashKeyPart(toolsPayload),
    ].join(':');
  }

  async getUsableContextCacheEntry(
    cacheKey: string,
    reason: string,
  ): Promise<GeminiContextCacheEntry | null> {
    const entry = GeminiContextCacheManager.contextCacheStore.get(
      this.cacheNamespace,
      cacheKey,
    );
    if (!entry) {
      return null;
    }

    const age = Date.now() - entry.createdAt;
    if (age >= GeminiContextCacheManager.CONTEXT_CACHE_TTL_MS) {
      await this.removeContextCacheEntry(cacheKey, reason);
      return null;
    }

    entry.lastUsedAt = Date.now();
    return entry;
  }

  async createContextCacheEntry(
    cacheKey: string,
    model: string,
    stablePrefix: string,
    geminiTools?: Array<{ functionDeclarations: FunctionDeclaration[] }>,
  ): Promise<string | undefined> {
    try {
      this.logger.debug(
        'Creating Gemini context cache for stable prefix and tools',
        {
          model,
          cacheKey,
          stablePrefixLength: stablePrefix.length,
          toolDeclarationCount:
            geminiTools?.[0]?.functionDeclarations.length ?? 0,
        },
      );

      const cacheResponse = await this.genAI.caches.create({
        model,
        config: {
          systemInstruction: stablePrefix,
          tools: geminiTools,
          ttl: '3600s',
        },
      });
      const cacheName = cacheResponse.name;
      if (!cacheName) {
        throw new Error('Gemini cache creation returned no cache name');
      }

      GeminiContextCacheManager.contextCacheStore.set(
        this.cacheNamespace,
        cacheKey,
        {
          name: cacheName,
          createdAt: Date.now(),
          lastUsedAt: Date.now(),
        },
      );
      await this.evictContextCacheOverflow();

      this.logger.info(
        `Gemini context cache created successfully: ${cacheName}`,
      );
      return cacheName;
    } catch (error) {
      this.logger.warn(
        'Failed to create Gemini context cache, falling back to standard request. Note: cacheable prefix must exceed Gemini minimum size.',
        error,
      );
      GeminiContextCacheManager.contextCacheStore.delete(
        this.cacheNamespace,
        cacheKey,
      );
      return undefined;
    }
  }

  private async evictContextCacheOverflow(): Promise<void> {
    while (
      GeminiContextCacheManager.contextCacheStore.size(this.cacheNamespace) >
      GeminiContextCacheManager.MAX_CONTEXT_CACHE_ENTRIES
    ) {
      const oldestEntry = GeminiContextCacheManager.contextCacheStore
        .list(this.cacheNamespace)
        .reduce(
          (
            oldest,
            current,
          ): [
            string,
            { name: string; createdAt: number; lastUsedAt: number },
          ] =>
            current[1].lastUsedAt < oldest[1].lastUsedAt ? current : oldest,
        );

      await this.removeContextCacheEntry(
        oldestEntry[0],
        'LRU eviction after cache growth',
      );
    }
  }

  private async removeContextCacheEntry(
    cacheKey: string,
    reason: string,
  ): Promise<void> {
    const entry = GeminiContextCacheManager.contextCacheStore.delete(
      this.cacheNamespace,
      cacheKey,
    );
    if (!entry) {
      return;
    }

    try {
      await this.genAI.caches.delete({ name: entry.name });
      this.logger.debug('Deleted Gemini context cache entry', {
        cacheKey,
        cachedContentName: entry.name,
        reason,
      });
    } catch (error) {
      this.logger.debug('Failed to delete Gemini context cache entry', {
        cacheKey,
        cachedContentName: entry.name,
        reason,
        error,
      });
    }
  }
}
