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
  private static readonly CONTEXT_CACHE_TTL_MS = 55 * 60 * 1000;
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
    }
  >();

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
      const requiresToolOverride =
        Boolean(geminiTools) &&
        (options.forceToolUse === true || options.disableToolUse === true);
      const canUseCachedContent = !requiresToolOverride;
      const shouldUseCache =
        canUseCachedContent &&
        this.shouldAttemptContextCache(model, stablePrefix, toolsPayload);
      let cachedContentName: string | undefined;
      let cacheKey: string | undefined;

      if (shouldUseCache) {
        cacheKey = this.createContextCacheKey(
          model,
          stablePrefix,
          toolsPayload,
        );
        const existingEntry = await this.getUsableContextCacheEntry(
          cacheKey,
          'preflight validation',
        );

        if (existingEntry) {
          cachedContentName = existingEntry.name;
        } else {
          cachedContentName = await this.createContextCacheEntry(
            cacheKey,
            model,
            stablePrefix,
            geminiTools,
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
        cachedContentName,
      });

      const geminiConfig: GeminiServiceConfig = {
        responseMimeType: 'text/plain',
      };

      if (cachedContentName) {
        geminiConfig.cachedContent = cachedContentName;
        // The Google GenAI API does not allow overriding system_instruction or tools when cached_content is provided.
        // Therefore, we inject the dynamic context into the first user message.
        if (dynamicContext && geminiMessages.length > 0) {
          if (geminiMessages[0].parts) {
            geminiMessages[0].parts.unshift({
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

      const result = await this.withRetry(async () => {
        return this.genAI.models.generateContentStream({
          model: model,
          config: geminiConfig,
          contents: geminiMessages,
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

  private shouldAttemptContextCache(
    model: string,
    stablePrefix: string,
    toolsPayload: string,
  ): boolean {
    if (!GeminiService.supportsToolsForModel(model)) {
      return false;
    }

    const cacheableTokenEstimate = this.estimateCacheablePrefixTokens(
      stablePrefix,
      toolsPayload,
    );
    return cacheableTokenEstimate >= GeminiService.MIN_CACHEABLE_PREFIX_TOKENS;
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
  ): string {
    return [
      model,
      stableHashKeyPart(stablePrefix),
      stableHashKeyPart(toolsPayload),
    ].join(':');
  }

  private async getUsableContextCacheEntry(
    cacheKey: string,
    reason: string,
  ): Promise<{ name: string; createdAt: number; lastUsedAt: number } | null> {
    const entry = this.cachedContextEntries.get(cacheKey);
    if (!entry) {
      return null;
    }

    const age = Date.now() - entry.createdAt;
    if (age >= GeminiService.CONTEXT_CACHE_TTL_MS) {
      await this.removeContextCacheEntry(cacheKey, reason);
      return null;
    }

    entry.lastUsedAt = Date.now();
    return entry;
  }

  private async createContextCacheEntry(
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

      this.cachedContextEntries.set(cacheKey, {
        name: cacheName,
        createdAt: Date.now(),
        lastUsedAt: Date.now(),
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
        (
          oldest,
          current,
        ): [string, { name: string; createdAt: number; lastUsedAt: number }] =>
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
}
