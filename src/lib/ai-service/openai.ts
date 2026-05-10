import OpenAI from 'openai';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';

import { getLogger } from '../logger';
import type { Message } from '@/models/chat';
import {
  MCPTool,
  MCPContent,
  SamplingOptions,
  SamplingResponse,
} from '@/lib/mcp';
import {
  AIServiceProvider,
  AIServiceConfig,
  type ContextInjectionResult,
  TokenUsage,
} from './types';
import { BaseAIService } from './base-service';
import type { ModelInfo } from '../llm-config-manager';
import { supportsThinking } from './model-capabilities';
import { ensureSchemaTypeField, processMessageContent } from './utils';
import { OpenAIPromptDiagnosticsTracker } from './openai/diagnostics';
import { convertToOpenAIMessages } from './openai/message-converter';
import { fetchOpenAIModels } from './openai/models';
import {
  buildAutomaticPromptCacheKey,
  withPromptCaching,
} from './openai/prompt-cache';
import {
  createEphemeralSessionContextInjection,
  formatSessionContextAsBackgroundReference,
} from './base-service-context';
import {
  createSerializableToolCallArgumentDelta,
  serializeToolCallArgumentDeltas,
} from './stream-events';
import type {
  OpenAINonStreamingRequest,
  OpenAIResponseUsageDetails,
  OpenAIStreamUsage,
  OpenAIStreamingRequest,
} from './openai/types';
import { isOpenAIStreamUsage } from './openai/types';

const logger = getLogger('OpenAIService');

/**
 * An AI service implementation for OpenAI's language models.
 * This class also serves as a base for other OpenAI-compatible services like Fireworks.
 */
export class OpenAIService extends BaseAIService<
  OpenAI.Chat.Completions.ChatCompletionMessageParam,
  OpenAI.Chat.ChatCompletionTool
> {
  protected openai: OpenAI;
  private readonly promptDiagnostics: OpenAIPromptDiagnosticsTracker;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = 3600000; // 1 hour in milliseconds

  /**
   * Initializes a new instance of the `OpenAIService`.
   * @param apiKey The OpenAI API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      baseURL: config?.baseUrl || undefined,
      dangerouslyAllowBrowser: true,
    });
    this.promptDiagnostics = new OpenAIPromptDiagnosticsTracker(this.logger);
  }

  static supportsToolsForModel(modelName: string): boolean {
    const lowerName = modelName.toLowerCase();
    const isReasoningModel = /^o(?:1|3|4)(?:$|[-.])/.test(lowerName);
    return (
      lowerName.includes('gpt-4') ||
      lowerName.includes('gpt-3.5-turbo') ||
      isReasoningModel
    );
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('gpt-4.1')) return 1000000;
    if (lowerName.includes('gpt-4o')) return 128000;
    if (/^o(?:3|4)(?:$|[-.])/.test(lowerName)) return 200000;
    return 8192;
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.OpenAI`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.OpenAI;
  }

  /**
   * @inheritdoc
   */
  convertTools(mcpTools: MCPTool[]): OpenAIChatCompletionTool[] {
    return mcpTools.map((mcpTool) => {
      const parameters = ensureSchemaTypeField(mcpTool.inputSchema);

      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters: parameters as Record<string, unknown>,
        },
      };
    });
  }

  /**
   * OpenAI-optimised context injection strategy.
   *
   * Keeps the stable system prompt untouched so OpenAI's automatic prefix
   * caching can maximise cache hits across turns. The volatile `sessionContext`
   * (planning state, memory, current time, etc.) is injected as an ephemeral
   * user message appended at the tail of the conversation, framed so the model
   * treats it as background context rather than a question to answer.
   *
   * If `sessionContext` is absent, falls back to the base (concat) behaviour.
   */
  override prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): ContextInjectionResult {
    if (!sessionContext) {
      return { systemPrompt, sessionContext: undefined, messages };
    }

    logger.debug('Injecting session context as ephemeral tail message', {
      sessionContextLength: sessionContext.length,
    });

    return createEphemeralSessionContextInjection(
      systemPrompt,
      sessionContext,
      messages,
      {
        idPrefix: 'openai-session-context',
        contentText: formatSessionContextAsBackgroundReference(sessionContext),
        sessionIdFallback: '',
        threadIdFallback: '',
        createdAt: new Date(),
      },
    );
  }

  /**
   * Fetches the list of available models from the OpenAI service.
   * Maps provider-specific model metadata into the project's `ModelInfo` shape.
   * On error, returns an empty array and logs the failure.
   */
  async listModels(): Promise<ModelInfo[]> {
    const logger = getLogger('OpenAIService.listModels');

    // Return cached models if still valid
    if (this.modelCache && this.isCacheValid()) {
      logger.debug('Returning cached models');
      return this.modelCache;
    }

    try {
      const models = await fetchOpenAIModels({
        openai: this.openai,
        provider: AIServiceProvider.OpenAI,
        withRetry: (fn) => this.withRetry(fn),
        logger,
      });

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

      return models;
    } catch (error) {
      logger.warn(
        'Failed to fetch models from OpenAI API, falling back to static config',
        error,
      );
      return this.fallbackToStaticModels();
    }
  }

  /**
   * Initiates a streaming chat session with the OpenAI API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @yields A JSON string for each chunk of the response.
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
      signal?: AbortSignal;
    } = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools, sanitizedMessages } = this.prepareStreamChat(
      messages,
      options,
    );

    const provider = this.getProvider();

    try {
      // Use the sanitized messages prepared for the provider to ensure
      // provider-specific fixes (tool call conversions, thinking-field removals, etc.)
      const openaiMessages = this.convertMessages(
        sanitizedMessages,
        options.systemPrompt,
      );

      let modelName = options.modelName || config.defaultModel || 'gpt-4-turbo';

      // Handle Fireworks prefix
      const fireworksPrefix = 'accounts/fireworks/models/';
      if (
        provider === AIServiceProvider.Fireworks &&
        !modelName.startsWith(fireworksPrefix)
      ) {
        modelName = `${fireworksPrefix}${modelName}`;
      }

      // Prepare reasoning_effort for reasoning models
      // Check model capability dynamically instead of hardcoded patterns
      let reasoningEffort: 'low' | 'medium' | 'high' | undefined;
      if (config.enableReasoning && config.reasoningEffort) {
        const modelSupportsThinking = await supportsThinking(
          modelName,
          provider,
        );
        if (modelSupportsThinking) {
          reasoningEffort = config.reasoningEffort;
        }
      }

      const automaticPromptCacheKey = this.buildAutomaticPromptCacheKey({
        model: modelName,
        systemPrompt: options.systemPrompt,
        messages: sanitizedMessages,
        tools,
        config,
      });

      const request = withPromptCaching<OpenAIStreamingRequest>(
        {
          model: modelName,
          messages: openaiMessages,
          max_completion_tokens: config.maxTokens,
          stream: true,
          stream_options: { include_usage: true },
          ...(reasoningEffort && { reasoning_effort: reasoningEffort }),
          tools,
          tool_choice: !options.availableTools?.length
            ? undefined
            : options.disableToolUse
              ? 'none'
              : options.forceToolUse
                ? 'required'
                : 'auto',
        },
        this.getProvider(),
        config,
        automaticPromptCacheKey,
      );

      const requestId = this.promptDiagnostics.createRequestId();
      this.promptDiagnostics.logPromptDiagnostics({
        mode: 'stream',
        model: modelName,
        systemPrompt: options.systemPrompt,
        request,
        messages: openaiMessages,
        tools,
      });
      this.promptDiagnostics.logFetchDiagnostics({
        mode: 'stream',
        requestId,
        model: modelName,
        request,
      });

      const abortSignal = options.signal;
      const completion = await this.withRetry(
        () =>
          this.openai.chat.completions.create(request, {
            signal: abortSignal,
            headers: {
              'x-libragent-request-id': requestId,
            },
          }),
        abortSignal,
      );

      if (abortSignal?.aborted) {
        this.logger.debug('Stream aborted before iteration');
        return;
      }

      yield* this.streamChatWithTTFT(
        (async function* (
          service: OpenAIService,
        ): AsyncGenerator<string, void, void> {
          for await (const chunk of completion) {
            if (abortSignal?.aborted) {
              service.logger.info('Stream aborted during iteration');
              break;
            }

            const rawUsage = chunk.usage;
            if (rawUsage && isOpenAIStreamUsage(rawUsage)) {
              const u = rawUsage as OpenAIStreamUsage;
              service.promptDiagnostics.logPromptCacheMetadata({
                mode: 'stream',
                model: modelName,
                request,
                usage: u,
              });
              const cachedPromptTokens =
                u.prompt_tokens_details?.cached_tokens ??
                u.prompt_cache_hit_tokens;

              const usage: TokenUsage = {
                promptTokens: u.prompt_tokens || 0,
                completionTokens: u.completion_tokens || 0,
                totalTokens: u.total_tokens || 0,
                cachedPromptTokens,
                details: {
                  reasoningTokens:
                    u.completion_tokens_details?.reasoning_tokens,
                },
              };
              yield JSON.stringify({ usage });
            }

            const delta = chunk.choices[0]
              ?.delta as OpenAI.Chat.Completions.ChatCompletionChunk.Choice.Delta & {
              reasoning_content?: string;
            };
            if (delta?.reasoning_content) {
              yield JSON.stringify({
                thinking: delta.reasoning_content || '',
              });
            }

            if (delta?.tool_calls) {
              const toolCalls = delta.tool_calls
                .map((toolCall) => {
                  if (typeof toolCall.index !== 'number') {
                    return null;
                  }

                  return createSerializableToolCallArgumentDelta(
                    toolCall.index,
                    toolCall.function?.arguments || '',
                    {
                      id: toolCall.id,
                      name: toolCall.function?.name,
                    },
                  );
                })
                .filter(
                  (
                    toolCall,
                  ): toolCall is ReturnType<
                    typeof createSerializableToolCallArgumentDelta
                  > => toolCall !== null,
                );

              if (toolCalls.length > 0) {
                yield serializeToolCallArgumentDeltas(toolCalls);
              }
            } else if (delta?.content) {
              yield JSON.stringify({
                content: delta.content || '',
              });
            }
          }
        })(this),
      );
    } catch (error) {
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Remove thinking-related fields that OpenAI family doesn't support
    if (message.thinking) {
      logger.debug('Removing thinking field for OpenAI family', {
        messageId: message.id,
      });
      delete message.thinking;
    }
    if (message.thinkingSignature) {
      delete message.thinkingSignature;
    }

    // Convert tool_use to tool_calls for OpenAI family
    if (message.tool_use && !message.tool_calls) {
      message.tool_calls = [
        {
          id: message.tool_use.id,
          type: 'function',
          function: {
            name: message.tool_use.name,
            arguments: JSON.stringify(message.tool_use.input),
          },
        },
      ];
      logger.debug('Converted tool_use to tool_calls for OpenAI family', {
        messageId: message.id,
        toolName: message.tool_use.name,
      });
      delete message.tool_use;
    }

    return message;
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    return OpenAIService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return OpenAIService.estimateContextWindowForModel(modelName);
  }

  /**
   * Converts an array of standard `Message` objects into the format required by the OpenAI API.
   * UI-generated messages (source: 'ui') are treated as user messages to ensure
   * the AI model interprets UI interactions as user intent rather than system responses.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of `OpenAI.Chat.Completions.ChatCompletionMessageParam` objects.
   * @private
   */
  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): OpenAI.Chat.Completions.ChatCompletionMessageParam[] {
    return convertToOpenAIMessages({
      messages,
      systemPrompt,
      logger: this.logger,
      processMessageContent: (content) => this.processMessageContent(content),
      processMultiModalContent: (content) =>
        this.processMultiModalContent(content),
      extractMediaContent: (content) => this.extractMediaContent(content),
    });
  }

  protected processMessageContent(content: MCPContent[]): string {
    return processMessageContent(content);
  }

  /**
   * @inheritdoc
   * @description The OpenAI SDK does not require explicit resource cleanup.
   */

  /**
   * Check if model cache is still valid (1 hour TTL)
   * @private
   */
  private isCacheValid(): boolean {
    if (!this.cacheTimestamp) return false;
    const age = Date.now() - this.cacheTimestamp;
    return age < this.CACHE_TTL;
  }

  /**
   * Fallback to static config models
   * @private
   */
  private fallbackToStaticModels(): Promise<ModelInfo[]> {
    const logger = getLogger('OpenAIService.fallbackToStaticModels');
    logger.info('Using static config models');
    return super.listModels();
  }

  /**
   * Performs a non-streaming text generation request using the OpenAI API.
   * Subclasses that use OpenAI-compatible endpoints (Fireworks, OpenRouter) inherit this.
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
      signal?: AbortSignal;
    },
  ): Promise<SamplingResponse> {
    const config = this.mergeConfig(options);
    const model = options?.modelName || config.defaultModel || '';
    const s = options?.samplingOptions;

    const request = withPromptCaching<OpenAINonStreamingRequest>(
      {
        model,
        stream: false,
        messages: [{ role: 'user', content: prompt }],
        max_tokens: s?.maxTokens ?? config.maxTokens,
        temperature: s?.temperature ?? config.temperature,
        top_p: s?.topP,
        presence_penalty: s?.presencePenalty,
        frequency_penalty: s?.frequencyPenalty,
        stop: s?.stopSequences,
      },
      this.getProvider(),
      config,
    );

    const requestId = this.promptDiagnostics.createRequestId();
    this.promptDiagnostics.logPromptDiagnostics({
      mode: 'non-stream',
      model,
      systemPrompt: undefined,
      request,
      messages: request.messages,
      tools: request.tools,
    });
    this.promptDiagnostics.logFetchDiagnostics({
      mode: 'non-stream',
      requestId,
      model,
      request,
    });

    const abortSignal = options?.signal;
    const response = await this.withRetry(
      () =>
        this.openai.chat.completions.create(request, {
          signal: abortSignal,
          headers: {
            'x-libragent-request-id': requestId,
          },
        }),
      abortSignal,
    );

    if (response.usage) {
      this.promptDiagnostics.logPromptCacheMetadata({
        mode: 'non-stream',
        model,
        request,
        usage: response.usage as OpenAIResponseUsageDetails & {
          prompt_tokens?: number;
          completion_tokens?: number;
          total_tokens?: number;
        },
      });
    }

    const choice = response.choices[0];
    const text = choice.message.content ?? '';

    return {
      jsonrpc: '2.0',
      id: null,
      result: {
        content: [{ type: 'text', text }],
        sampling: {
          finishReason: choice.finish_reason === 'stop' ? 'stop' : 'length',
          usage: response.usage
            ? {
                promptTokens: response.usage.prompt_tokens,
                completionTokens: response.usage.completion_tokens,
                totalTokens: response.usage.total_tokens,
                cachedPromptTokens:
                  (
                    response.usage as unknown as {
                      prompt_tokens_details?: { cached_tokens?: number };
                    }
                  ).prompt_tokens_details?.cached_tokens ??
                  (
                    response.usage as unknown as {
                      prompt_cache_hit_tokens?: number;
                    }
                  ).prompt_cache_hit_tokens,
              }
            : undefined,
          model: response.model,
        },
      },
    };
  }

  dispose(): void {
    // OpenAI SDK doesn't require explicit cleanup
  }

  private buildAutomaticPromptCacheKey(args: {
    model: string;
    systemPrompt?: string;
    messages?: Message[];
    tools?: OpenAIChatCompletionTool[];
    config?: AIServiceConfig;
  }): string | undefined {
    return buildAutomaticPromptCacheKey(args);
  }
}
