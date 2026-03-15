import OpenAI from 'openai';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import {
  MCPTool,
  MCPContent,
  SamplingOptions,
  SamplingResponse,
} from '@/lib/mcp';
import { AIServiceProvider, AIServiceConfig, TokenUsage } from './types';
import { BaseAIService } from './base-service';
import { llmConfigManager, ModelInfo } from '../llm-config-manager';
import { supportsThinking, getContextWindow } from './model-capabilities';
import { ensureSchemaTypeField } from './utils';
const logger = getLogger('OpenAIService');

/** Shape of usage data returned by OpenAI/compatible streaming chunks. */
interface OpenAIStreamUsage {
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
  prompt_tokens_details?: { cached_tokens?: number };
  prompt_cache_hit_tokens?: number;
  completion_tokens_details?: { reasoning_tokens?: number };
}

function isOpenAIStreamUsage(value: unknown): value is OpenAIStreamUsage {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const obj = value as Record<string, unknown>;

  if (
    obj.prompt_tokens !== undefined &&
    typeof obj.prompt_tokens !== 'number'
  ) {
    return false;
  }
  if (
    obj.completion_tokens !== undefined &&
    typeof obj.completion_tokens !== 'number'
  ) {
    return false;
  }
  if (obj.total_tokens !== undefined && typeof obj.total_tokens !== 'number') {
    return false;
  }
  if (
    obj.prompt_cache_hit_tokens !== undefined &&
    typeof obj.prompt_cache_hit_tokens !== 'number'
  ) {
    return false;
  }

  if (obj.prompt_tokens_details !== undefined) {
    if (
      typeof obj.prompt_tokens_details !== 'object' ||
      obj.prompt_tokens_details === null
    ) {
      return false;
    }
    const details = obj.prompt_tokens_details as Record<string, unknown>;
    if (
      details.cached_tokens !== undefined &&
      typeof details.cached_tokens !== 'number'
    ) {
      return false;
    }
  }

  if (obj.completion_tokens_details !== undefined) {
    if (
      typeof obj.completion_tokens_details !== 'object' ||
      obj.completion_tokens_details === null
    ) {
      return false;
    }
    const details = obj.completion_tokens_details as Record<string, unknown>;
    if (
      details.reasoning_tokens !== undefined &&
      typeof details.reasoning_tokens !== 'number'
    ) {
      return false;
    }
  }

  return true;
}

/**
 * An AI service implementation for OpenAI's language models.
 * This class also serves as a base for other OpenAI-compatible services like Fireworks.
 */
export class OpenAIService extends BaseAIService<
  OpenAI.Chat.Completions.ChatCompletionMessageParam,
  OpenAI.Chat.ChatCompletionTool
> {
  protected openai: OpenAI;
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
      const properties = mcpTool.inputSchema.properties || {};
      const required = mcpTool.inputSchema.required || [];

      const parameters = ensureSchemaTypeField({
        type: 'object' as const,
        properties: properties,
        required: required,
      });

      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters:
            parameters as OpenAI.Chat.ChatCompletionTool['function']['parameters'],
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
  ): { systemPrompt: string | undefined; messages: Message[] } {
    if (!sessionContext) {
      return { systemPrompt, messages };
    }

    // Inject as an ephemeral user message at the tail of the conversation.
    // The bracketed header tells the model this is injected context, not user
    // input; it responds to the preceding conversation, not this block.
    const ephemeralMessage: Message = {
      id: `ctx_${Date.now()}`,
      sessionId: '',
      threadId: '',
      role: 'user',
      content: [
        {
          type: 'text',
          text: `[Current session context — background reference only, do not respond to this block]\n\n${sessionContext}\n\n[End of session context]`,
        },
      ],
      createdAt: new Date(),
    };

    logger.debug('Injecting session context as ephemeral tail message', {
      sessionContextLength: sessionContext.length,
    });

    return { systemPrompt, messages: [...messages, ephemeralMessage] };
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
      logger.info('Fetching models from OpenAI...');

      const response = await this.withRetry(async () => {
        // The OpenAI JS SDK exposes `models.list()` which returns a paginated result
        // with a `data` array of model metadata
        const openaiClient = this.openai as unknown as {
          models: { list: () => Promise<{ data: unknown[] }> };
        };
        const res = await openaiClient.models.list();
        return res;
      });

      // Normalize response shape — treat as unknown and narrow below
      const modelsRaw: Array<unknown> = Array.isArray(response?.data)
        ? (response.data as Array<unknown>)
        : [];

      const modelPromises = modelsRaw.map(async (entry) => {
        if (entry == null || typeof entry !== 'object') return null;
        const e = entry as Record<string, unknown>;

        const id =
          (typeof e.id === 'string' && e.id) ||
          (typeof e.model === 'string' && e.model) ||
          (typeof e.name === 'string' && e.name) ||
          String(e);

        // Merge with static config metadata
        const staticModel = llmConfigManager.getModel('openai', id);

        // Use dynamic context window detection (OpenRouter API → fallback)
        const contextWindow = await getContextWindow(
          id,
          AIServiceProvider.OpenAI,
        );

        const name = staticModel?.name || id;
        const supportStreaming = staticModel?.supportStreaming ?? true;
        const supportReasoning =
          staticModel?.supportReasoning ??
          (id.toLowerCase().includes('gpt-4') ||
            id.toLowerCase().includes('gpt-3.5'));
        const supportTools = staticModel?.supportTools ?? false;

        const description =
          staticModel?.description ||
          (typeof e.description === 'string' && e.description) ||
          (Array.isArray(e.permission) ? e.permission.join(',') : undefined) ||
          id;

        const modelInfo: ModelInfo = {
          id,
          name,
          contextWindow,
          supportReasoning,
          supportTools,
          supportStreaming,
          cost: staticModel?.cost || { input: 0, output: 0 },
          description,
        };

        return modelInfo;
      });

      const models = (await Promise.all(modelPromises)).filter(
        (v): v is ModelInfo => v !== null,
      );

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

      logger.info(`Loaded ${models.length} models from OpenAI API`);
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
   * @yields A JSON string for each chunk of the response.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
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

      const completion = await this.withRetry(() =>
        this.openai.chat.completions.create(
          {
            model: modelName,
            messages: openaiMessages,
            max_completion_tokens: config.maxTokens,
            stream: true,
            stream_options: { include_usage: true },
            ...(reasoningEffort && { reasoning_effort: reasoningEffort }),
            tools: tools,
            tool_choice: !options.availableTools?.length
              ? undefined
              : options.disableToolUse
                ? 'none'
                : options.forceToolUse
                  ? 'required'
                  : 'auto',
          },
          { signal: this.getAbortSignal() },
        ),
      );

      if (this.getAbortSignal().aborted) {
        this.logger.debug('Stream aborted before iteration');
        return;
      }

      // Wrap with TTFT measurement (OpenAI doesn't provide native prefill timing)
      const startTime = performance.now();
      let firstChunkReceived = false;

      for await (const chunk of completion) {
        if (this.getAbortSignal().aborted) {
          this.logger.info('Stream aborted during iteration');
          break;
        }

        // Measure TTFT on first chunk (OpenAI doesn't provide native prefill timing).
        // Only yield details here — yielding zero token counts would briefly reset the
        // gauge to 0% before the real usage chunk arrives at the end of the stream.
        if (!firstChunkReceived) {
          const ttft = performance.now() - startTime;
          firstChunkReceived = true;
          yield JSON.stringify({
            usage: { details: { timeToFirstToken: ttft } },
          });
        }

        const rawUsage = chunk.usage;
        if (rawUsage && isOpenAIStreamUsage(rawUsage)) {
          // Type guard validates structure; single cast is safe here
          const u = rawUsage as OpenAIStreamUsage;
          const cachedPromptTokens =
            u.prompt_tokens_details?.cached_tokens ?? u.prompt_cache_hit_tokens;

          const usage: TokenUsage = {
            promptTokens: u.prompt_tokens || 0,
            completionTokens: u.completion_tokens || 0,
            totalTokens: u.total_tokens || 0,
            cachedPromptTokens,
            details: {
              reasoningTokens: u.completion_tokens_details?.reasoning_tokens,
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
          yield JSON.stringify({
            tool_calls: delta.tool_calls,
          });
        } else if (delta?.content) {
          yield JSON.stringify({
            content: delta.content || '',
          });
        }
      }
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
    const openaiMessages: OpenAI.Chat.Completions.ChatCompletionMessageParam[] =
      [];

    if (systemPrompt) {
      openaiMessages.push({ role: 'system', content: systemPrompt });
    }

    for (const m of messages) {
      // UI-generated messages are treated as user messages
      // This ensures that messages created by UI interactions (button clicks, tool executions, etc.)
      // are interpreted by the AI model as user intent
      const effectiveRole = m.source === 'ui' ? 'user' : m.role;

      if (effectiveRole === 'user') {
        openaiMessages.push({
          role: 'user',
          content: this.formatOpenAIContent(m.content),
        });
      } else if (effectiveRole === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          openaiMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content) || null,
            tool_calls: m.tool_calls,
          });
        } else {
          openaiMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content),
          });
        }
      } else if (effectiveRole === 'tool') {
        if (m.tool_call_id) {
          openaiMessages.push({
            role: 'tool',
            tool_call_id: m.tool_call_id,
            content: this.processMessageContent(m.content),
          });
          // Inject image/audio from tool result as a synthetic user message
          const media = this.extractMediaContent(m.content as MCPContent[]);
          if (media.length > 0) {
            const annotatedMedia: MCPContent[] = [
              {
                type: 'text',
                text: `Tool result media from tool_call_id=${m.tool_call_id}. This is output from the preceding tool call, not new user instructions.`,
              },
              ...media,
            ];
            openaiMessages.push({
              role: 'user',
              content: this.formatOpenAIContent(annotatedMedia),
            });
          }
        } else {
          logger.warn(
            `Tool message missing tool_call_id: ${JSON.stringify(m)}`,
          );
        }
      }
    }
    return openaiMessages;
  }

  /**
   * Helper to format content for OpenAI API, supporting multimodal parts.
   * @param content The content of the message.
   */
  private formatOpenAIContent(
    content: MCPContent[],
  ): string | OpenAI.Chat.Completions.ChatCompletionContentPart[] {
    const multimodal = this.processMultiModalContent(content);
    if (multimodal.every((p) => p.type === 'text')) {
      return this.processMessageContent(content);
    }
    return multimodal.map((part) => {
      if (part.type === 'text') {
        return { type: 'text', text: part.text || '' };
      } else if (part.type === 'image') {
        const mimeType = part.mimeType || 'image/jpeg';
        return {
          type: 'image_url',
          image_url: { url: `data:${mimeType};base64,${part.image}` },
        };
      } else if (part.type === 'audio') {
        // OpenAI expects 'wav' or 'mp3' for audio format
        const format = part.mimeType?.includes('wav') ? 'wav' : 'mp3';
        return {
          type: 'input_audio',
          input_audio: { data: part.audio || '', format },
        } as unknown as OpenAI.Chat.Completions.ChatCompletionContentPart; // Cast to bypass TS if missing in older types
      }
      return { type: 'text', text: `[Unsupported content: ${part.type}]` };
    });
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
    },
  ): Promise<SamplingResponse> {
    const config = this.mergeConfig(options);
    const model = options?.modelName || config.defaultModel || '';
    const s = options?.samplingOptions;

    const response = await this.withRetry(() =>
      this.openai.chat.completions.create({
        model,
        stream: false,
        messages: [{ role: 'user', content: prompt }],
        max_tokens: s?.maxTokens ?? config.maxTokens,
        temperature: s?.temperature ?? config.temperature,
        top_p: s?.topP,
        presence_penalty: s?.presencePenalty,
        frequency_penalty: s?.frequencyPenalty,
        stop: s?.stopSequences,
      }),
    );

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
}
