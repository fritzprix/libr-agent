import Anthropic from '@anthropic-ai/sdk';
import {
  MessageParam as AnthropicMessageParam,
  Tool as AnthropicTool,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import {
  AIServiceProvider,
  AIServiceConfig,
  type ContextInjectionResult,
  TokenUsage,
} from './types';
import { BaseAIService } from './base-service';
import { ModelInfo, llmConfigManager } from '../llm-config-manager';
import { supportsThinking, getContextWindow } from './model-capabilities';
import {
  applyAnthropicMessageDeltaUsage,
  applyAnthropicMessageStartUsage,
  buildAnthropicPromptCacheMetadata,
  buildAnthropicSystemBlocks,
  getAnthropicPromptTokens,
} from './anthropic/cache';
import { convertToAnthropicMessages } from './anthropic/message-converter';
import {
  ANTHROPIC_MODEL_CACHE_TTL,
  cacheAnthropicModels,
  getDefaultAnthropicModel,
  isAnthropicModelCacheValid,
  validateAnthropicFallbackModel,
} from './anthropic/models';
import {
  createEmptyAnthropicUsage,
  ToolCallAccumulator,
} from './anthropic/types';
import { createEphemeralSessionContextInjection } from './base-service-context';
import {
  createSerializableToolCallArgumentDelta,
  serializeToolCallArgumentDeltas,
} from './stream-events';
import { ensureSchemaTypeField } from './utils';

const logger = getLogger('AnthropicService');

const MAX_PARTIAL_TOOL_INPUT_LENGTH = 200_000;
const ANTHROPIC_SESSION_CONTEXT_METADATA_KEY =
  'anthropicSyntheticSessionContext';

/**
 * An AI service implementation for interacting with Anthropic's language models (e.g., Claude).
 * It handles the specifics of the Anthropic API, including message formatting,
 * tool use, and streaming.
 */
export class AnthropicService extends BaseAIService<
  AnthropicMessageParam,
  AnthropicTool
> {
  private anthropic: Anthropic;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = ANTHROPIC_MODEL_CACHE_TTL;

  /**
   * Initializes a new instance of the `AnthropicService`.
   * @param apiKey The Anthropic API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.anthropic = new Anthropic({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
    // Validate that fallback model exists in config
    validateAnthropicFallbackModel(this.logger);
  }

  /**
   * Gets the provider identifier.
   * @returns `AIServiceProvider.Anthropic`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Anthropic;
  }

  /**
   * @inheritdoc
   *
   * Marks the last tool with `cache_control: { type: 'ephemeral' }` so that
   * Anthropic caches the entire tool list as a second cache breakpoint.
   */
  convertTools(mcpTools: MCPTool[]): AnthropicTool[] {
    const tools = mcpTools.map((mcpTool) => {
      const input_schema = ensureSchemaTypeField(mcpTool.inputSchema);

      return {
        name: mcpTool.name,
        description: mcpTool.description,
        input_schema: input_schema as AnthropicTool['input_schema'],
      } as AnthropicTool;
    });

    if (tools.length > 0) {
      (
        tools[tools.length - 1] as AnthropicTool & {
          cache_control?: { type: 'ephemeral' };
        }
      ).cache_control = { type: 'ephemeral' };
    }
    return tools;
  }

  /**
   * Fetches available Claude models from Anthropic API.
   * Falls back to static config if SDK call fails.
   * Results are cached for 1 hour to minimize API calls.
   */
  async listModels(): Promise<ModelInfo[]> {
    const logger = getLogger('AnthropicService.listModels');

    // Return cached models if still valid
    if (this.modelCache && this.isCacheValid()) {
      logger.debug('Returning cached models');
      return this.modelCache;
    }

    try {
      // Use official SDK models.list() API
      const response = await this.anthropic.models.list();

      if (!response?.data || !Array.isArray(response.data)) {
        logger.warn('Invalid response structure from models API', { response });
        return this.fallbackToStaticModels();
      }

      const models: ModelInfo[] = [];

      for (const model of response.data) {
        // Merge SDK data with static config metadata
        const staticModel = llmConfigManager.getModel('anthropic', model.id);

        // Use dynamic context window detection
        const contextWindow = await getContextWindow(
          model.id,
          AIServiceProvider.Anthropic,
        );

        models.push({
          id: model.id,
          name: model.display_name || staticModel?.name || model.id,
          contextWindow,
          // Use static config as source of truth for capabilities
          supportReasoning: staticModel?.supportReasoning ?? false,
          supportTools: staticModel?.supportTools ?? true,
          supportStreaming: staticModel?.supportStreaming ?? true,
          cost: staticModel?.cost || {
            input: 0,
            output: 0,
          },
          description: staticModel?.description || `Claude model: ${model.id}`,
        });
      }

      const cacheState = cacheAnthropicModels(models);
      this.modelCache = cacheState.modelCache;
      this.cacheTimestamp = cacheState.cacheTimestamp;

      logger.info(`Loaded ${models.length} models from Anthropic API`);
      return models;
    } catch (error) {
      logger.warn(
        'Failed to fetch models from Anthropic API, falling back to static config',
        error,
      );
      return this.fallbackToStaticModels();
    }
  }

  /**
   * Fallback to static config models
   */
  private fallbackToStaticModels(): Promise<ModelInfo[]> {
    const logger = getLogger('AnthropicService.fallbackToStaticModels');
    logger.info('Using static config models');
    return super.listModels();
  }

  /**
   * Check if model cache is still valid (1 hour TTL)
   */
  private isCacheValid(): boolean {
    return isAnthropicModelCacheValid(this.cacheTimestamp, this.CACHE_TTL);
  }

  /**
   * Selects the best available model following priority order.
   * Priority: explicit option > config default > first available config model > safe fallback
   * @private
   */
  private getDefaultModel(): string {
    const logger = getLogger('AnthropicService.getDefaultModel');
    const model = getDefaultAnthropicModel(this.config);

    if (this.config?.defaultModel) {
      logger.debug(`Using config default model: ${model}`);
    } else if (llmConfigManager.getModelsForProvider('anthropic')) {
      logger.debug(`Using configured Anthropic model: ${model}`);
    } else {
      logger.warn(`No config models found, using fallback: ${model}`);
    }

    return model;
  }

  private logPromptCacheMetadata(args: {
    mode: 'stream' | 'non-stream';
    source: 'message_start' | 'message_delta' | 'sample';
    model: string;
    usage: {
      input_tokens: number | null;
      output_tokens: number | null;
      cache_creation_input_tokens?: number | null;
      cache_read_input_tokens?: number | null;
    };
    previousDetails?: TokenUsage['details'];
  }): void {
    this.logger.info('Anthropic prompt cache metadata', {
      mode: args.mode,
      source: args.source,
      model: args.model,
      ...buildAnthropicPromptCacheMetadata(args.usage, args.previousDetails),
    });
  }

  /**
   * Initiates a streaming chat session with the Anthropic API.
   * It handles message conversion, tool use, and processes the streaming response,
   * including partial JSON accumulation for tool calls and 'thinking' state updates.
   *
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat, including model name, system prompt, and tools.
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @yields A JSON string for each chunk of the response. The format can be `{ content: string }`
   *         for text, `{ thinking: object }` for thinking state, or `{ tool_calls: [...] }` for tool calls.
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
      const anthropicMessages = this.convertMessages(
        sanitizedMessages,
        options.systemPrompt,
      );

      // Check if model supports extended thinking via dynamic capability detection
      const model = options.modelName || this.getDefaultModel();
      let extendedThinking: boolean | undefined;
      if (config.enableReasoning) {
        const modelSupportsThinking = await supportsThinking(
          model,
          AIServiceProvider.Anthropic,
        );
        if (modelSupportsThinking) {
          extendedThinking = true;
        }
      }

      const systemBlocks = buildAnthropicSystemBlocks(
        options.systemPrompt,
        options.sessionContext,
      );

      const stream = this.anthropic.messages.stream(
        {
          model: model,
          max_tokens: config.maxTokens!,
          messages: anthropicMessages,
          ...(systemBlocks && { system: systemBlocks }),
          ...(extendedThinking && { extended_thinking: extendedThinking }),
          tools: tools,
          ...(options.forceToolUse &&
            options.availableTools?.length && { tool_choice: { type: 'any' } }),
        },
        { signal: this.getAbortSignal() },
      );

      // Tool call accumulator for partial JSON streaming
      const toolCallAccumulators = new Map<number, ToolCallAccumulator>();

      const createIndexedToolCall = (
        accumulator: ToolCallAccumulator,
        argumentsDelta: string,
      ) =>
        createSerializableToolCallArgumentDelta(
          accumulator.index,
          argumentsDelta,
          {
            id: accumulator.id,
            name: accumulator.name,
          },
        );

      // Track current usage metrics (updated from message_start and message_delta)
      let currentUsage: TokenUsage = createEmptyAnthropicUsage();

      if (this.getAbortSignal().aborted) {
        this.logger.debug('Stream aborted before iteration');
        return;
      }

      for await (const chunk of stream) {
        if (this.getAbortSignal().aborted) {
          this.logger.debug('Stream aborted during iteration');
          break;
        }

        // Handle message_start for input tokens and cache stats
        if (chunk.type === 'message_start') {
          if (chunk.message?.usage) {
            this.logPromptCacheMetadata({
              mode: 'stream',
              source: 'message_start',
              model,
              usage: chunk.message.usage,
              previousDetails: currentUsage.details,
            });
            currentUsage = applyAnthropicMessageStartUsage(
              currentUsage,
              chunk.message.usage,
            );
            yield JSON.stringify({ usage: currentUsage });
          }
        }

        if (chunk.type === 'message_delta') {
          if (chunk.usage) {
            this.logPromptCacheMetadata({
              mode: 'stream',
              source: 'message_delta',
              model,
              usage: chunk.usage,
              previousDetails: currentUsage.details,
            });
            currentUsage = applyAnthropicMessageDeltaUsage(
              currentUsage,
              chunk.usage,
            );
            yield JSON.stringify({ usage: currentUsage });
          }
        }

        // Extra logging for delta inspection: helpful to see exact shapes
        if (chunk && chunk.type === 'content_block_delta') {
          // kept only if really needed, otherwise remove or debug
          // removing for cleanup as requested
        }

        if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'text_delta'
        ) {
          yield JSON.stringify({ content: chunk.delta.text });
        } else if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'thinking_delta'
        ) {
          yield JSON.stringify({ thinking: chunk.delta.thinking });
        } else if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'signature_delta'
        ) {
          yield JSON.stringify({ thinkingSignature: chunk.delta.signature });
        } else if (chunk.type === 'content_block_start') {
          // Initialize accumulator for new tool call
          if (chunk.content_block.type === 'tool_use') {
            const initialInput =
              chunk.content_block.input &&
              typeof chunk.content_block.input === 'object' &&
              !Array.isArray(chunk.content_block.input)
                ? (chunk.content_block.input as Record<string, unknown>)
                : null;
            toolCallAccumulators.set(chunk.index, {
              id: chunk.content_block.id,
              name: chunk.content_block.name,
              partialJson: '',
              index: chunk.index,
              hasArgumentDelta: false,
              initialInput,
            });
            logger.debug('Started tool call accumulation', {
              index: chunk.index,
              id: chunk.content_block.id,
              name: chunk.content_block.name,
            });
            const accumulator = toolCallAccumulators.get(chunk.index);
            if (accumulator) {
              yield serializeToolCallArgumentDeltas([
                createIndexedToolCall(accumulator, ''),
              ]);
            }
          }
        } else if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'input_json_delta'
        ) {
          // Accumulate partial JSON
          const accumulator = toolCallAccumulators.get(chunk.index);
          if (accumulator) {
            // log the incoming partial fragment for inspection
            logger.info('Anthropic input_json_delta fragment', {
              index: chunk.index,
              fragment: chunk.delta.partial_json,
              currentLength: accumulator.partialJson.length,
            });
            accumulator.partialJson += chunk.delta.partial_json;
            if (
              accumulator.partialJson.length > MAX_PARTIAL_TOOL_INPUT_LENGTH
            ) {
              logger.error('Tool call input exceeded maximum buffered length', {
                index: chunk.index,
                toolId: accumulator.id,
                name: accumulator.name,
              });
              toolCallAccumulators.delete(chunk.index);
              continue;
            }
            logger.debug('Accumulated partial JSON', {
              index: chunk.index,
              partialJson: accumulator.partialJson,
            });
            if (chunk.delta.partial_json.length === 0) {
              logger.debug('No tool argument delta to emit yet', {
                index: chunk.index,
                id: accumulator.id,
              });
              continue;
            }
            accumulator.hasArgumentDelta = true;
            yield serializeToolCallArgumentDeltas([
              createIndexedToolCall(accumulator, chunk.delta.partial_json),
            ]);
          }
        } else if (chunk.type === 'content_block_stop') {
          logger.info('Anthropic content_block_stop', { index: chunk.index });
          const accumulator = toolCallAccumulators.get(chunk.index);
          if (
            accumulator &&
            !accumulator.hasArgumentDelta &&
            accumulator.initialInput
          ) {
            logger.info(
              'Tool call completed using initial input without deltas',
              {
                id: accumulator.id,
                name: accumulator.name,
              },
            );
            yield serializeToolCallArgumentDeltas([
              createIndexedToolCall(
                accumulator,
                JSON.stringify(accumulator.initialInput),
              ),
            ]);
          } else if (
            accumulator &&
            !accumulator.hasArgumentDelta &&
            accumulator.partialJson.trim()
          ) {
            yield serializeToolCallArgumentDeltas([
              createIndexedToolCall(accumulator, accumulator.partialJson),
            ]);
          }

          // Clean up accumulator regardless of yield status
          if (accumulator) {
            toolCallAccumulators.delete(chunk.index);
            logger.debug('Cleaned up tool call accumulator', {
              index: chunk.index,
              id: accumulator.id,
              hadArgumentDelta: accumulator.hasArgumentDelta,
            });
          }
        }
      }
    } catch (error) {
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  override prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): ContextInjectionResult {
    if (!sessionContext) {
      return { systemPrompt, sessionContext, messages };
    }

    return createEphemeralSessionContextInjection(
      systemPrompt,
      sessionContext,
      messages,
      {
        idPrefix: 'anthropic-session-context',
        metadata: {
          [ANTHROPIC_SESSION_CONTEXT_METADATA_KEY]: true,
        },
      },
    );
  }
  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Filter out tool messages without tool_call_id
    if (message.role === 'tool' && !message.tool_call_id) {
      logger.debug('Filtering out tool message without tool_call_id', {
        messageId: message.id,
      });
      return null;
    }

    return message;
  }

  /**
   * @inheritdoc
   */
  static supportsToolsForModel(modelName: string): boolean {
    const lowerName = modelName.toLowerCase();
    return lowerName.includes('claude-3') || lowerName.includes('claude-opus');
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('claude-3')) return 200000;
    return 100000;
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    return AnthropicService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return AnthropicService.estimateContextWindowForModel(modelName);
  }

  /**
   * Converts an array of standard `Message` objects into the format required
   * by the Anthropic API. It also performs a strict integrity check to ensure
   * that all tool calls have a corresponding tool result, throwing an error
   * if any inconsistencies are found.
   *
   * @param messages The array of messages to convert.
   * @param systemPrompt The system prompt.
   * @returns An array of `AnthropicMessageParam` objects.
   * @throws An error if an incomplete tool chain is detected.
   */
  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): AnthropicMessageParam[] {
    return convertToAnthropicMessages(messages, systemPrompt);
  }

  /**
   * Performs a non-streaming text generation request using the Anthropic API.
   * Used by the base-class `compact()` for context summarisation.
   * @param prompt The user prompt to send.
   * @param options Optional model name, sampling parameters, and service config.
   * @param options.modelName The name of the model.
   * @param options.samplingOptions The options used for text generation sampling.
   * @param options.config Optional configuration for the service.
   * @returns A resolved `SamplingResponse` with the generated text.
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
    const model =
      options?.modelName || config.defaultModel || this.getDefaultModel();
    const s = options?.samplingOptions;
    const abortSignal = this.getAbortSignal();

    const response = await this.withRetry(() =>
      this.anthropic.messages.create(
        {
          model,
          max_tokens: s?.maxTokens ?? config.maxTokens ?? 4096,
          temperature: s?.temperature ?? config.temperature,
          top_p: s?.topP,
          top_k: s?.topK,
          stop_sequences: s?.stopSequences,
          messages: [{ role: 'user', content: prompt }],
        },
        {
          signal: abortSignal,
        },
      ),
    );

    const textBlock = response.content.find((b) => b.type === 'text');
    const text = textBlock?.type === 'text' ? textBlock.text : '';

    this.logPromptCacheMetadata({
      mode: 'non-stream',
      source: 'sample',
      model,
      usage: response.usage,
    });

    const promptTokens = getAnthropicPromptTokens(response.usage);

    return {
      jsonrpc: '2.0',
      id: null,
      result: {
        content: [{ type: 'text', text }],
        sampling: {
          finishReason: response.stop_reason === 'end_turn' ? 'stop' : 'length',
          usage: {
            promptTokens,
            completionTokens: response.usage.output_tokens,
            totalTokens: promptTokens + response.usage.output_tokens,
            cachedPromptTokens:
              response.usage.cache_read_input_tokens ?? undefined,
          },
          model: response.model,
        },
      },
    };
  }

  /**
   * @inheritdoc
   * @description The Anthropic SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    // Anthropic SDK doesn't require explicit cleanup
  }
}
