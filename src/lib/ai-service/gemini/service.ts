import {
  FunctionDeclaration,
  FinishReason,
  GoogleGenAI,
  Content,
  Schema as GeminiSchema,
  Type,
} from '@google/genai';
import { JSONSchema } from '@/lib/mcp';
import { getLogger } from '../../logger';
import { Message } from '@/models/chat';
import { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import {
  AIServiceProvider,
  AIServiceConfig,
  type ContextInjectionResult,
} from '../types';
import {
  BaseAIService,
  stableHashKeyPart,
  stableStringify,
} from '../base-service';
import type { ModelInfo } from '../../llm-config-manager';
import { GeminiServiceConfig } from './types';
import { convertToGeminiMessages } from './mapper';
import {
  mapReasoningEffortToBudget,
  checkThinkingSupport,
  prepareSafetySettings,
} from './config';
import { fetchGeminiModels, getDefaultModel } from './models';
import { processGeminiStream } from './stream';

/**
 * An AI service implementation for interacting with Google's Gemini models.
 */
export class GeminiService extends BaseAIService<Content, FunctionDeclaration> {
  private static readonly MIN_CACHEABLE_PREFIX_TOKENS = 32768;
  private static readonly MAX_CONTEXT_CACHE_ENTRIES = 8;
  private static readonly SHORT_CONTEXT_CACHE_TTL_MS = 60 * 60 * 1000;
  private static readonly MEDIUM_CONTEXT_CACHE_TTL_MS = 3 * 60 * 60 * 1000;
  private static readonly LONG_CONTEXT_CACHE_TTL_MS = 6 * 60 * 60 * 1000;
  private static readonly CONTEXT_CACHE_REFRESH_THRESHOLD_MS = 10 * 60 * 1000;
  private static readonly MIN_HISTORY_CHECKPOINT_MESSAGES = 5;
  private static readonly HISTORY_CHECKPOINT_TAIL_USER_MESSAGES = 2;
  private genAI: GoogleGenAI;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = 3600000;
  private readonly cachedContextEntries = new Map<
    string,
    {
      name: string;
      createdAt: number;
      lastUsedAt: number;
      ttlMs: number;
      expiresAt: number;
      cacheableTokenCount: number;
    }
  >();
  private readonly cacheableTokenCounts = new Map<
    string,
    {
      tokenCount: number;
      source: 'count_tokens' | 'estimate_fallback';
    }
  >();
  private lastPromptSnapshot?: {
    model: string;
    systemPromptHash: string;
    toolsHash: string;
    cachedHistoryHash?: string;
    cachedHistoryMessageCount: number;
    messagesFingerprintHash: string;
    messageFingerprints: Array<{
      role: string;
      partCount: number;
      textLength: number;
      textHash: string;
      contentTag: 'regular' | 'session_context' | 'tool_result_media';
      functionCallCount: number;
      functionCallNames?: string[];
      functionResponseCount: number;
      functionResponseNames?: string[];
    }>;
    promptCacheKey?: string;
    cacheableTokenCount: number;
    tokenDecisionSource: 'count_tokens' | 'estimate_fallback';
    cacheStrategy: 'system_tools' | 'history_checkpoint' | 'none';
  };

  /**
   * Initializes a new instance of the `GeminiService`.
   * @param apiKey The Google AI API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.genAI = new GoogleGenAI({
      apiKey: this.apiKey,
    });
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Gemini`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Gemini;
  }

  /**
   * @inheritdoc
   */
  convertTools(mcpTools: MCPTool[]): FunctionDeclaration[] {
    return mcpTools.map((mcpTool) => {
      // Convert the entire inputSchema to Gemini format
      const geminiParams = this.convertMCPSchemaToGeminiParameters(
        mcpTool.inputSchema,
      );

      return {
        name: mcpTool.name,
        description: mcpTool.description,
        parameters: {
          type: Type.OBJECT,
          description: mcpTool.inputSchema.description,
          properties: geminiParams.properties || {},
          required: mcpTool.inputSchema.required || [],
        },
      } satisfies FunctionDeclaration;
    });
  }

  /**
   * Recursively converts an MCPTool's JSONSchema into the Google GenAI FunctionDeclaration format.
   * This properly handles nested objects and arrays, preserving all schema information.
   * @param schema The MCP JSONSchema to convert.
   * @returns The schema in the format required by Google GenAI SDK.
   * @private
   */
  private convertMCPSchemaToGeminiParameters(schema: JSONSchema): GeminiSchema {
    // Base case: String type with optional enum
    if (schema.type === 'string') {
      const result: GeminiSchema = { type: Type.STRING };
      if (schema.description) result.description = schema.description;
      if ('enum' in schema && Array.isArray(schema.enum)) {
        result.enum = schema.enum as string[];
      }
      return result;
    }

    // Base case: Number types
    if (schema.type === 'number' || schema.type === 'integer') {
      const result: GeminiSchema = { type: Type.NUMBER };
      if (schema.description) result.description = schema.description;
      return result;
    }

    // Base case: Boolean type
    if (schema.type === 'boolean') {
      const result: GeminiSchema = { type: Type.BOOLEAN };
      if (schema.description) result.description = schema.description;
      return result;
    }

    // Base case: Null type
    if (schema.type === 'null') {
      const result: GeminiSchema = { type: Type.STRING };
      if (schema.description) result.description = schema.description;
      return result;
    }

    // Recursive case: Arrays
    if (schema.type === 'array' && 'items' in schema && schema.items) {
      const arrayItems = Array.isArray(schema.items)
        ? schema.items[0]
        : schema.items;
      const result: GeminiSchema = {
        type: Type.ARRAY,
        items: arrayItems
          ? this.convertMCPSchemaToGeminiParameters(arrayItems)
          : { type: Type.STRING },
      };
      if (schema.description) result.description = schema.description;
      return result;
    }

    // Recursive case: Objects
    if (
      schema.type === 'object' &&
      'properties' in schema &&
      schema.properties
    ) {
      const geminiProperties: Record<string, GeminiSchema> = {};

      for (const [key, propSchema] of Object.entries(schema.properties)) {
        geminiProperties[key] =
          this.convertMCPSchemaToGeminiParameters(propSchema);
      }

      const result: GeminiSchema = {
        type: Type.OBJECT,
        properties: geminiProperties,
      };
      if (schema.description) result.description = schema.description;
      return result;
    }

    // Fallback for unknown or incomplete types
    return { type: Type.STRING };
  }

  /**
   * Fetches the list of available models from the Gemini API.
   * Uses pagination to retrieve all models and caches results for 1 hour.
   * Falls back to static config on API failure.
   */
  async listModels(): Promise<ModelInfo[]> {
    const logger = getLogger('GeminiService.listModels');

    // Return cached models if still valid
    if (this.modelCache && this.isCacheValid()) {
      logger.debug('Returning cached models');
      return this.modelCache;
    }

    try {
      // Use withRetry to wrap the fetch call
      const models = await this.withRetry(async () => {
        return fetchGeminiModels(this.genAI);
      });

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

      return models;
    } catch (error) {
      logger.warn(
        'Failed to fetch models from Gemini API, falling back to static config',
        error,
      );
      return this.fallbackToStaticModels();
    }
  }

  /**
   * Initiates a streaming chat session with the Gemini API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @yields A JSON string for each chunk of the response, containing content and/or tool calls.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      sessionContext?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
      disableToolUse?: boolean;
    } = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools, sanitizedMessages } = this.prepareStreamChat(
      messages,
      options,
    );

    try {
      const geminiMessages = this.convertMessages(
        sanitizedMessages,
        options.systemPrompt,
      );
      if (geminiMessages.length === 0) {
        throw new Error(
          'No valid messages to send to Gemini (must start with user/tool role)',
        );
      }
      const geminiTools = tools
        ? [
            {
              functionDeclarations: tools as FunctionDeclaration[],
            },
          ]
        : undefined;

      const model =
        options.modelName || config.defaultModel || getDefaultModel();

      // --- CONTEXT CACHING ABSTRACTION BEGIN ---
      const stablePrefix = options.systemPrompt ?? '';
      const dynamicContext = options.sessionContext ?? '';
      const toolsPayload = geminiTools ? stableStringify(geminiTools) : '';
      const historyCheckpoint = this.buildHistoryCheckpoint(geminiMessages);
      const cacheableContents = historyCheckpoint?.cacheContents;
      const requestContents = historyCheckpoint?.requestContents ?? geminiMessages;
      const cacheStrategy =
        historyCheckpoint !== null ? 'history_checkpoint' : 'system_tools';
      const requiresToolOverride =
        Boolean(geminiTools) &&
        (options.forceToolUse === true || options.disableToolUse === true);
      const canUseCachedContent = !requiresToolOverride;
      const cacheDecision = canUseCachedContent
        ? await this.evaluateContextCacheEligibility(
            model,
            stablePrefix,
            geminiTools,
            toolsPayload,
            cacheableContents,
          )
        : {
            shouldUseCache: false,
            cacheableTokenCount: 0,
            tokenDecisionSource: 'estimate_fallback' as const,
          };
      const shouldUseCache = cacheDecision.shouldUseCache;
      let cachedContentName: string | undefined;
      let cacheKey: string | undefined;

      if (shouldUseCache) {
        cacheKey = this.createContextCacheKey(
          model,
          stablePrefix,
          toolsPayload,
          historyCheckpoint?.cacheKeyPart,
        );
        const existingEntry = await this.getUsableContextCacheEntry(
          cacheKey,
          'preflight validation',
          this.getContextCacheTtlMs(cacheDecision.cacheableTokenCount),
        );

        if (existingEntry) {
          cachedContentName = existingEntry.name;
        } else {
          cachedContentName = await this.createContextCacheEntry(
            cacheKey,
            model,
            stablePrefix,
            geminiTools,
            cacheDecision.cacheableTokenCount,
            cacheableContents,
          );
        }
      }
      // --- CONTEXT CACHING ABSTRACTION END ---

      this.logPromptCacheMetadata({
        model,
        stablePrefix,
        toolsPayload,
        cacheKey,
        canUseCachedContent,
        requiresToolOverride,
        shouldUseCache,
        cacheableTokenCount: cacheDecision.cacheableTokenCount,
        tokenDecisionSource: cacheDecision.tokenDecisionSource,
        cacheStrategy:
          shouldUseCache && historyCheckpoint ? 'history_checkpoint' : cacheStrategy,
        cachedHistoryMessageCount: cacheableContents?.length ?? 0,
        cachedContentName,
      });

      const geminiConfig: GeminiServiceConfig = {
        responseMimeType: 'text/plain',
      };

      if (cachedContentName) {
        geminiConfig.cachedContent = cachedContentName;
        // The Google GenAI API does not allow overriding system_instruction or tools when cached_content is provided.
        // Therefore, we inject the dynamic context into the first user message.
        if (dynamicContext && requestContents.length > 0) {
          if (requestContents[0].parts) {
            requestContents[0].parts.unshift({
              text: `[System Context Update]\n${dynamicContext}\n\n`,
            });
          }
        }
      } else {
        if (geminiTools) {
          geminiConfig.tools = geminiTools;
          if (options.disableToolUse) {
            geminiConfig.functionCallingConfig = { mode: 'none' };
          } else if (options.forceToolUse) {
            geminiConfig.functionCallingConfig = { mode: 'any' };
          }
        }
        const combinedSystemPrompt = [
          options.systemPrompt,
          options.sessionContext,
        ]
          .filter(Boolean)
          .join('\n\n');
        if (combinedSystemPrompt) {
          geminiConfig.systemInstruction = [{ text: combinedSystemPrompt }];
        }
      }

      if (config.maxTokens) {
        geminiConfig.maxOutputTokens = config.maxTokens;
      }

      if (config.temperature !== undefined) {
        geminiConfig.temperature = config.temperature;
      }

      // Add thinkingConfig for models that support thinking
      if (config.enableReasoning) {
        const modelSupportsThinking = await checkThinkingSupport(
          model,
          this.modelCache,
        );
        if (modelSupportsThinking) {
          const thinkingBudget = mapReasoningEffortToBudget(
            config.reasoningEffort,
          );
          geminiConfig.thinkingConfig = {
            thinkingBudget,
            includeThoughts: true,
          };
        }
      }

      // Configure Gemini safety settings.
      geminiConfig.safetySettings = prepareSafetySettings(config);

      // 🔍 Detailed Logging before API Call
      const sysPromptText = geminiConfig.systemInstruction?.[0]?.text || '';
      this.logger.debug('🚀 Calling Gemini API - System Prompt Verification', {
        model,
        systemPromptLength: sysPromptText.length,
        includesSkills: sysPromptText.includes('<available_skills>'),
        first500Chars: sysPromptText.substring(0, 500),
        last500Chars: sysPromptText.substring(sysPromptText.length - 500),
      });
      this.logPromptDiagnostics({
        model,
        systemPrompt: options.systemPrompt,
        promptCacheKey: cacheKey,
        messages: requestContents,
        geminiTools,
        cachedHistoryContents: cacheableContents,
        cacheableTokenCount: cacheDecision.cacheableTokenCount,
        tokenDecisionSource: cacheDecision.tokenDecisionSource,
        cacheStrategy:
          shouldUseCache && historyCheckpoint ? 'history_checkpoint' : cacheStrategy,
      });

      const result = await this.withRetry(async () => {
        return this.genAI.models.generateContentStream({
          model: model,
          config: geminiConfig,
          contents: requestContents,
        });
      });

      if (this.getAbortSignal().aborted) {
        this.logger.debug('Stream aborted before iteration');
        return;
      }

      // Use the stream processing logic from stream.ts
      yield* processGeminiStream(result, this.getAbortSignal(), this.logger);
    } catch (error) {
      if (
        error instanceof Error &&
        (error.message.includes('malformed_function_call') ||
          error.message.includes('MALFORMED_FUNCTION_CALL'))
      ) {
        this.logger.warn(
          'MALFORMED_FUNCTION_CALL detected. Retrying request without tools.',
          { originalError: error },
        );
        if (options.availableTools && options.availableTools.length > 0) {
          const retryOptions = { ...options, availableTools: [] };
          yield* this.streamChat(messages, retryOptions);
          return;
        }
      }

      this.handleStreamingError(error, {
        messages,
        options,
        config,
      });
    }
  }

  override prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): ContextInjectionResult {
    if (!sessionContext) {
      return { systemPrompt, sessionContext: undefined, messages };
    }

    const syntheticSessionContextMessage =
      this.createSyntheticSessionContextMessage(sessionContext, messages, {
        idPrefix: 'gemini-session-context',
        contentText:
          this.formatSessionContextAsBackgroundReference(sessionContext),
      });

    this.logger.debug(
      'Injecting Gemini session context as ephemeral tail message',
      {
        sessionContextLength: sessionContext.length,
      },
    );

    return {
      systemPrompt,
      sessionContext: undefined,
      messages: [...messages, syntheticSessionContextMessage],
    };
  }

  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): Content[] {
    // Note: Gemini system prompt is handled via systemInstruction in config
    void systemPrompt;
    return convertToGeminiMessages(messages);
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Gemini handles system messages separately
    if (message.role === 'system') return null;

    // Remove thinkingSignature if it's the dummy signature we injected
    // (though usually we don't need to do this for the upstream API)
    if (message.thinkingSignature === 'skip_thought_signature_validator') {
      delete message.thinkingSignature;
    }

    return message;
  }

  /**
   * @inheritdoc
   */
  static supportsToolsForModel(modelName: string): boolean {
    const lowerName = modelName.toLowerCase();
    return lowerName.includes('gemini-1.5') || lowerName.includes('gemini-2');
  }

  static supportsContextCacheForModel(modelName: string): boolean {
    const lowerName = modelName.toLowerCase();
    return (
      lowerName.includes('gemini-1.5') ||
      lowerName.includes('gemini-2') ||
      lowerName.includes('gemini-3')
    );
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('gemini-1.5-pro')) return 2000000;
    if (lowerName.includes('gemini-1.5-flash')) return 1000000;
    if (lowerName.includes('gemini-2.0-flash')) return 1000000;
    return 1000000;
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    return GeminiService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return GeminiService.estimateContextWindowForModel(modelName);
  }

  /**
   * Check if model cache is still valid (1 hour TTL)
   * @private
   */
  private isCacheValid(): boolean {
    if (!this.cacheTimestamp) return false;
    const age = Date.now() - this.cacheTimestamp;
    return age < this.CACHE_TTL;
  }

  private async evaluateContextCacheEligibility(
    model: string,
    stablePrefix: string,
    geminiTools:
      | Array<{ functionDeclarations: FunctionDeclaration[] }>
      | undefined,
    toolsPayload: string,
    cacheableContents?: Content[],
  ): Promise<{
    shouldUseCache: boolean;
    cacheableTokenCount: number;
    tokenDecisionSource: 'count_tokens' | 'estimate_fallback';
  }> {
    if (!GeminiService.supportsContextCacheForModel(model)) {
      return {
        shouldUseCache: false,
        cacheableTokenCount: 0,
        tokenDecisionSource: 'estimate_fallback',
      };
    }

    if (!stablePrefix && !toolsPayload) {
      return {
        shouldUseCache: false,
        cacheableTokenCount: 0,
        tokenDecisionSource: 'estimate_fallback',
      };
    }

    const tokenCountKey = this.createContextCacheKey(
      model,
      stablePrefix,
      toolsPayload,
      cacheableContents
        ? stableHashKeyPart(stableStringify(this.createGeminiContentFingerprint(cacheableContents)))
        : undefined,
    );
    const cachedTokenCount = this.cacheableTokenCounts.get(tokenCountKey);
    if (cachedTokenCount) {
      return {
        shouldUseCache:
          cachedTokenCount.tokenCount >=
          GeminiService.MIN_CACHEABLE_PREFIX_TOKENS,
        cacheableTokenCount: cachedTokenCount.tokenCount,
        tokenDecisionSource: cachedTokenCount.source,
      };
    }

    try {
      const countResponse = await this.withRetry(() =>
        this.genAI.models.countTokens({
          model,
          contents: cacheableContents ?? [{ role: 'user', parts: [{ text: 'cache probe' }] }],
          config: {
            systemInstruction: stablePrefix,
            tools: geminiTools,
          },
        }),
      );
      const tokenCount = countResponse.totalTokens ?? 0;
      const tokenDecision = {
        tokenCount,
        source: 'count_tokens' as const,
      };
      this.cacheableTokenCounts.set(tokenCountKey, tokenDecision);

      return {
        shouldUseCache: tokenCount >= GeminiService.MIN_CACHEABLE_PREFIX_TOKENS,
        cacheableTokenCount: tokenCount,
        tokenDecisionSource: tokenDecision.source,
      };
    } catch (error) {
      const cacheableTokenEstimate = this.estimateCacheablePrefixTokens(
        stablePrefix,
        toolsPayload,
      );
      this.logger.warn(
        'Gemini countTokens failed during cache eligibility check; falling back to character estimate.',
        {
          model,
          stablePrefixLength: stablePrefix.length,
          toolsPayloadLength: toolsPayload.length,
          cacheableTokenEstimate,
          error,
        },
      );
      const tokenDecision = {
        tokenCount: cacheableTokenEstimate,
        source: 'estimate_fallback' as const,
      };
      this.cacheableTokenCounts.set(tokenCountKey, tokenDecision);

      return {
        shouldUseCache:
          cacheableTokenEstimate >= GeminiService.MIN_CACHEABLE_PREFIX_TOKENS,
        cacheableTokenCount: cacheableTokenEstimate,
        tokenDecisionSource: tokenDecision.source,
      };
    }
  }

  private estimateCacheablePrefixTokens(
    stablePrefix: string,
    toolsPayload: string,
  ): number {
    return Math.ceil((stablePrefix.length + toolsPayload.length) / 4);
  }

  private logPromptCacheMetadata(args: {
    model: string;
    stablePrefix: string;
    toolsPayload: string;
    cacheKey?: string;
    canUseCachedContent: boolean;
    requiresToolOverride: boolean;
    shouldUseCache: boolean;
    cacheableTokenCount: number;
    tokenDecisionSource: 'count_tokens' | 'estimate_fallback';
    cacheStrategy: 'system_tools' | 'history_checkpoint' | 'none';
    cachedHistoryMessageCount: number;
    cachedContentName?: string;
  }): void {
    const cacheableTokenEstimate = this.estimateCacheablePrefixTokens(
      args.stablePrefix,
      args.toolsPayload,
    );

    this.logger.info('Gemini prompt cache metadata', {
      model: args.model,
      cacheKey: args.cacheKey,
      stablePrefixHash: stableHashKeyPart(args.stablePrefix),
      toolsHash: stableHashKeyPart(args.toolsPayload),
      stablePrefixLength: args.stablePrefix.length,
      toolsPayloadLength: args.toolsPayload.length,
      cacheableTokenEstimate,
      cacheableTokenCount: args.cacheableTokenCount,
      tokenDecisionSource: args.tokenDecisionSource,
      cacheStrategy: args.cacheStrategy,
      cachedHistoryMessageCount: args.cachedHistoryMessageCount,
      targetTtlMs: this.getContextCacheTtlMs(args.cacheableTokenCount),
      canUseCachedContent: args.canUseCachedContent,
      requiresToolOverride: args.requiresToolOverride,
      shouldUseCache: args.shouldUseCache,
      cachedContentName: args.cachedContentName,
      cacheHit: Boolean(args.cachedContentName),
    });
  }

  private createContextCacheKey(
    model: string,
    stablePrefix: string,
    toolsPayload: string,
    historyKeyPart?: string,
  ): string {
    return [
      model,
      stableHashKeyPart(stablePrefix),
      stableHashKeyPart(toolsPayload),
      historyKeyPart ?? 'no_history',
    ].join(':');
  }

  private async getUsableContextCacheEntry(
    cacheKey: string,
    reason: string,
    desiredTtlMs: number,
  ): Promise<{ name: string; createdAt: number; lastUsedAt: number } | null> {
    const entry = this.cachedContextEntries.get(cacheKey);
    if (!entry) {
      return null;
    }

    const now = Date.now();
    if (now >= entry.expiresAt) {
      await this.removeContextCacheEntry(cacheKey, reason);
      return null;
    }

    entry.lastUsedAt = now;
    if (this.shouldRefreshContextCacheEntry(entry, desiredTtlMs, now)) {
      try {
        await this.genAI.caches.update({
          name: entry.name,
          config: {
            ttl: this.toGeminiDurationString(desiredTtlMs),
          },
        });
        entry.ttlMs = desiredTtlMs;
        entry.expiresAt = now + desiredTtlMs;
        this.logger.debug('Refreshed Gemini context cache TTL on reuse', {
          cacheKey,
          cachedContentName: entry.name,
          ttlMs: desiredTtlMs,
        });
      } catch (error) {
        this.logger.debug('Failed to refresh Gemini context cache TTL', {
          cacheKey,
          cachedContentName: entry.name,
          ttlMs: desiredTtlMs,
          error,
        });
      }
    }
    return entry;
  }

  private async createContextCacheEntry(
    cacheKey: string,
    model: string,
    stablePrefix: string,
    geminiTools?: Array<{ functionDeclarations: FunctionDeclaration[] }>,
    cacheableTokenCount = 0,
    cacheableContents?: Content[],
  ): Promise<string | undefined> {
    try {
      const ttlMs = this.getContextCacheTtlMs(cacheableTokenCount);
      this.logger.debug(
        'Creating Gemini context cache for stable prefix and tools',
        {
          model,
          cacheKey,
          stablePrefixLength: stablePrefix.length,
          cacheableTokenCount,
          ttlMs,
          cacheableContentCount: cacheableContents?.length ?? 0,
          toolDeclarationCount:
            geminiTools?.[0]?.functionDeclarations.length ?? 0,
        },
      );

      const cacheResponse = await this.genAI.caches.create({
        model,
        config: {
          systemInstruction: stablePrefix,
          tools: geminiTools,
          contents: cacheableContents,
          ttl: this.toGeminiDurationString(ttlMs),
        },
      });
      const cacheName = cacheResponse.name;
      if (!cacheName) {
        throw new Error('Gemini cache creation returned no cache name');
      }

      const now = Date.now();
      this.cachedContextEntries.set(cacheKey, {
        name: cacheName,
        createdAt: now,
        lastUsedAt: now,
        ttlMs,
        expiresAt: now + ttlMs,
        cacheableTokenCount,
      });
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
      this.cachedContextEntries.delete(cacheKey);
      return undefined;
    }
  }

  private async evictContextCacheOverflow(): Promise<void> {
    while (
      this.cachedContextEntries.size > GeminiService.MAX_CONTEXT_CACHE_ENTRIES
    ) {
      const oldestEntry = [...this.cachedContextEntries.entries()].reduce(
        (oldest, current) =>
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
    const entry = this.cachedContextEntries.get(cacheKey);
    if (!entry) {
      return;
    }

    this.cachedContextEntries.delete(cacheKey);

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

  /**
   * Fallback to static config models
   * @private
   */
  private fallbackToStaticModels(): Promise<ModelInfo[]> {
    const logger = getLogger('GeminiService.fallbackToStaticModels');
    logger.info('Using static config models');
    return super.listModels();
  }

  /**
   * Performs a non-streaming text generation request using the Gemini API.
   * @param prompt The prompt to send to the model.
   * @param options Optional parameters for the sampling request.
   * @param options.modelName The name of the model.
   * @param options.samplingOptions The options used for text generation sampling.
   * @param options.config Optional configuration for the service.
   * @returns A promise that resolves to a `SamplingResponse`.
   */
  async sampleText(
    prompt: string,
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
    },
  ): Promise<SamplingResponse> {
    const rawConfig = this.mergeConfig(options) as AIServiceConfig &
      GeminiServiceConfig;
    const model =
      options?.modelName || rawConfig.defaultModel || getDefaultModel();
    const s = options?.samplingOptions;

    const response = await this.withRetry(() =>
      this.genAI.models.generateContent({
        model,
        config: {
          maxOutputTokens: s?.maxTokens ?? rawConfig.maxTokens,
          temperature: s?.temperature ?? rawConfig.temperature,
          topP: s?.topP,
          topK: s?.topK,
          stopSequences: s?.stopSequences,
        },
        contents: [{ role: 'user', parts: [{ text: prompt }] }],
      }),
    );

    const candidate = response.candidates?.[0];
    const text = candidate?.content?.parts?.[0]?.text ?? '';
    const finishReason = candidate?.finishReason ?? FinishReason.STOP;

    return {
      jsonrpc: '2.0',
      id: null,
      result: {
        content: [{ type: 'text', text }],
        sampling: {
          finishReason: finishReason === FinishReason.STOP ? 'stop' : 'length',
          usage: response.usageMetadata
            ? {
                promptTokens: response.usageMetadata.promptTokenCount ?? 0,
                completionTokens:
                  response.usageMetadata.candidatesTokenCount ?? 0,
                totalTokens: response.usageMetadata.totalTokenCount ?? 0,
                cachedPromptTokens:
                  response.usageMetadata.cachedContentTokenCount,
              }
            : undefined,
          model,
        },
      },
    };
  }

  /**
   * @inheritdoc
   * @description The Gemini SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    const cacheKeys = [...this.cachedContextEntries.keys()];

    for (const cacheKey of cacheKeys) {
      void this.removeContextCacheEntry(cacheKey, 'service dispose');
    }
  }

  private getContextCacheTtlMs(cacheableTokenCount: number): number {
    if (cacheableTokenCount >= 262144) {
      return GeminiService.LONG_CONTEXT_CACHE_TTL_MS;
    }
    if (cacheableTokenCount >= 131072) {
      return GeminiService.MEDIUM_CONTEXT_CACHE_TTL_MS;
    }
    return GeminiService.SHORT_CONTEXT_CACHE_TTL_MS;
  }

  private toGeminiDurationString(ttlMs: number): string {
    return `${Math.max(60, Math.floor(ttlMs / 1000))}s`;
  }

  private shouldRefreshContextCacheEntry(
    entry: {
      name: string;
      createdAt: number;
      lastUsedAt: number;
      ttlMs: number;
      expiresAt: number;
      cacheableTokenCount: number;
    },
    desiredTtlMs: number,
    now: number,
  ): boolean {
    const remainingTtlMs = entry.expiresAt - now;
    return (
      desiredTtlMs > entry.ttlMs ||
      remainingTtlMs <= GeminiService.CONTEXT_CACHE_REFRESH_THRESHOLD_MS
    );
  }

  private logPromptDiagnostics(args: {
    model: string;
    systemPrompt?: string;
    promptCacheKey?: string;
    messages: Content[];
    geminiTools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
    cachedHistoryContents?: Content[];
    cacheableTokenCount: number;
    tokenDecisionSource: 'count_tokens' | 'estimate_fallback';
    cacheStrategy: 'system_tools' | 'history_checkpoint' | 'none';
  }): void {
    const messageFingerprints = args.messages.map((message) =>
      this.fingerprintGeminiMessage(message),
    );
    const messagesFingerprintHash = stableHashKeyPart(
      stableStringify(messageFingerprints),
    );
    const toolsPayload = stableStringify(args.geminiTools ?? []);
    const cachedHistoryFingerprint =
      args.cachedHistoryContents && args.cachedHistoryContents.length > 0
        ? this.createGeminiContentFingerprint(args.cachedHistoryContents)
        : undefined;
    const snapshot = {
      model: args.model,
      systemPromptHash: stableHashKeyPart(args.systemPrompt ?? ''),
      toolsHash: stableHashKeyPart(toolsPayload),
      cachedHistoryHash: cachedHistoryFingerprint
        ? stableHashKeyPart(stableStringify(cachedHistoryFingerprint))
        : undefined,
      cachedHistoryMessageCount: args.cachedHistoryContents?.length ?? 0,
      messagesFingerprintHash,
      messageFingerprints,
      promptCacheKey: args.promptCacheKey,
      cacheableTokenCount: args.cacheableTokenCount,
      tokenDecisionSource: args.tokenDecisionSource,
      cacheStrategy: args.cacheStrategy,
    };

    this.logger.debug('Gemini prompt diagnostics', {
      model: args.model,
      promptCacheKey: args.promptCacheKey,
      systemPromptLength: args.systemPrompt?.length ?? 0,
      systemPromptHash: snapshot.systemPromptHash,
      toolCount: args.geminiTools?.[0]?.functionDeclarations.length ?? 0,
      toolsHash: snapshot.toolsHash,
      cacheStrategy: args.cacheStrategy,
      cachedHistoryHash: snapshot.cachedHistoryHash,
      cachedHistoryMessageCount: snapshot.cachedHistoryMessageCount,
      messageCount: messageFingerprints.length,
      messagesFingerprintHash,
      cacheableTokenCount: args.cacheableTokenCount,
      tokenDecisionSource: args.tokenDecisionSource,
      messageFingerprints,
    });
    this.logPromptDrift(snapshot);
  }

  private logPromptDrift(
    snapshot: NonNullable<GeminiService['lastPromptSnapshot']>,
  ): void {
    const previous = this.lastPromptSnapshot;
    this.lastPromptSnapshot = snapshot;

    if (!previous) {
      return;
    }

    const minMessageCount = Math.min(
      previous.messageFingerprints.length,
      snapshot.messageFingerprints.length,
    );
    let firstDivergenceIndex = -1;
    for (let index = 0; index < minMessageCount; index += 1) {
      if (
        stableStringify(previous.messageFingerprints[index]) !==
        stableStringify(snapshot.messageFingerprints[index])
      ) {
        firstDivergenceIndex = index;
        break;
      }
    }

    if (
      firstDivergenceIndex === -1 &&
      previous.messageFingerprints.length !==
        snapshot.messageFingerprints.length
    ) {
      firstDivergenceIndex = minMessageCount;
    }

    const firstDivergenceComponent =
      previous.model !== snapshot.model
        ? 'model'
        : previous.systemPromptHash !== snapshot.systemPromptHash
          ? 'system_prompt'
          : previous.toolsHash !== snapshot.toolsHash
            ? 'tools'
            : previous.cachedHistoryHash !== snapshot.cachedHistoryHash
              ? 'cached_history'
            : firstDivergenceIndex >= 0
              ? 'messages'
              : 'none';

    this.logger.debug('Gemini prompt cache drift', {
      previousModel: previous.model,
      model: snapshot.model,
      previousPromptCacheKey: previous.promptCacheKey,
      promptCacheKey: snapshot.promptCacheKey,
      previousCacheStrategy: previous.cacheStrategy,
      cacheStrategy: snapshot.cacheStrategy,
      firstDivergenceComponent,
      firstDivergenceIndex:
        firstDivergenceComponent === 'messages'
          ? firstDivergenceIndex
          : undefined,
      commonPrefixMessages:
        firstDivergenceComponent === 'messages'
          ? firstDivergenceIndex
          : Math.min(
              previous.messageFingerprints.length,
              snapshot.messageFingerprints.length,
            ),
      previousMessageCount: previous.messageFingerprints.length,
      messageCount: snapshot.messageFingerprints.length,
      systemPromptChanged:
        previous.systemPromptHash !== snapshot.systemPromptHash,
      toolsChanged: previous.toolsHash !== snapshot.toolsHash,
      cachedHistoryChanged:
        previous.cachedHistoryHash !== snapshot.cachedHistoryHash,
      previousCachedHistoryHash: previous.cachedHistoryHash,
      cachedHistoryHash: snapshot.cachedHistoryHash,
      previousCachedHistoryMessageCount: previous.cachedHistoryMessageCount,
      cachedHistoryMessageCount: snapshot.cachedHistoryMessageCount,
      messagesChanged:
        previous.messagesFingerprintHash !== snapshot.messagesFingerprintHash,
      previousFingerprintHash: previous.messagesFingerprintHash,
      fingerprintHash: snapshot.messagesFingerprintHash,
      previousMessageAtDivergence:
        firstDivergenceIndex >= 0
          ? previous.messageFingerprints[firstDivergenceIndex]
          : undefined,
      currentMessageAtDivergence:
        firstDivergenceIndex >= 0
          ? snapshot.messageFingerprints[firstDivergenceIndex]
          : undefined,
    });
  }

  private fingerprintGeminiMessage(message: Content): {
    role: string;
    partCount: number;
    textLength: number;
    textHash: string;
    contentTag: 'regular' | 'session_context' | 'tool_result_media';
    functionCallCount: number;
    functionCallNames?: string[];
    functionResponseCount: number;
    functionResponseNames?: string[];
  } {
    const parts = Array.isArray(message.parts) ? message.parts : [];
    const textSegments: string[] = [];
    const functionCallNames: string[] = [];
    const functionResponseNames: string[] = [];

    for (const part of parts) {
      if (typeof part !== 'object' || part === null) {
        continue;
      }
      const candidate = part as Record<string, unknown>;
      if (typeof candidate.text === 'string') {
        textSegments.push(candidate.text);
      }

      if (
        typeof candidate.functionCall === 'object' &&
        candidate.functionCall !== null
      ) {
        const functionCall = candidate.functionCall as Record<string, unknown>;
        if (typeof functionCall.name === 'string') {
          functionCallNames.push(functionCall.name);
        }
      }

      if (
        typeof candidate.functionResponse === 'object' &&
        candidate.functionResponse !== null
      ) {
        const functionResponse = candidate.functionResponse as Record<
          string,
          unknown
        >;
        if (typeof functionResponse.name === 'string') {
          functionResponseNames.push(functionResponse.name);
        }
      }
    }

    const text = textSegments.join('\n');
    return {
      role: message.role ?? 'user',
      partCount: parts.length,
      textLength: text.length,
      textHash: stableHashKeyPart(text),
      contentTag: this.classifyGeminiContentTag(message.role ?? 'user', text),
      functionCallCount: functionCallNames.length,
      functionCallNames:
        functionCallNames.length > 0 ? functionCallNames : undefined,
      functionResponseCount: functionResponseNames.length,
      functionResponseNames:
        functionResponseNames.length > 0 ? functionResponseNames : undefined,
    };
  }

  private classifyGeminiContentTag(
    role: string,
    content: string,
  ): 'regular' | 'session_context' | 'tool_result_media' {
    if (
      role === 'user' &&
      content.startsWith('[Current session context — background reference only')
    ) {
      return 'session_context';
    }

    if (
      role === 'user' &&
      content.startsWith('Tool result media from tool_call_id=')
    ) {
      return 'tool_result_media';
    }

    return 'regular';
  }

  private buildHistoryCheckpoint(
    messages: Content[],
  ): {
    cacheContents: Content[];
    requestContents: Content[];
    cacheKeyPart: string;
  } | null {
    if (messages.length < GeminiService.MIN_HISTORY_CHECKPOINT_MESSAGES) {
      return null;
    }

    let tailUserMessages = 0;
    let splitIndex = -1;
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      if (messages[index]?.role === 'user') {
        tailUserMessages += 1;
        if (tailUserMessages === GeminiService.HISTORY_CHECKPOINT_TAIL_USER_MESSAGES) {
          splitIndex = index;
          break;
        }
      }
    }

    if (splitIndex <= 0 || splitIndex >= messages.length) {
      return null;
    }

    const cacheContents = messages.slice(0, splitIndex);
    const requestContents = messages.slice(splitIndex);
    if (cacheContents.length === 0 || requestContents.length === 0) {
      return null;
    }

    return {
      cacheContents,
      requestContents,
      cacheKeyPart: stableHashKeyPart(
        stableStringify(this.createGeminiContentFingerprint(cacheContents)),
      ),
    };
  }

  private createGeminiContentFingerprint(messages: Content[]): Array<{
    role: string;
    partCount: number;
    textHash: string;
    functionCallNames?: string[];
    functionResponseNames?: string[];
  }> {
    return messages.map((message) => {
      const fingerprint = this.fingerprintGeminiMessage(message);
      return {
        role: fingerprint.role,
        partCount: fingerprint.partCount,
        textHash: fingerprint.textHash,
        functionCallNames: fingerprint.functionCallNames,
        functionResponseNames: fingerprint.functionResponseNames,
      };
    });
  }
}
