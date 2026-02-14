import Anthropic from '@anthropic-ai/sdk';
import {
  Tool as AnthropicTool,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import { getLogger } from '../../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '@/lib/mcp';
import { AIServiceProvider, AIServiceConfig, TokenUsage } from '../types';
import { BaseAIService } from '../base-service';
import { ModelInfo, llmConfigManager } from '../../llm-config-manager';
import { supportsThinking, getContextWindow } from '../model-capabilities';
import { AnthropicUsageWithCache } from './types';
import { CACHE_TTL } from './constants';
import {
  convertToAnthropicMessages,
  convertSingleAnthropicMessage,
} from './message-converter';
import { ToolCallStreamAccumulator } from './stream-handler';

/**
 * An AI service implementation for interacting with Anthropic's language models (e.g., Claude).
 * It handles the specifics of the Anthropic API, including message formatting,
 * tool use, and streaming.
 */
export class AnthropicService extends BaseAIService {
  private anthropic: Anthropic;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;

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
    this.validateFallbackModel();
  }

  /**
   * Gets the provider identifier.
   * @returns `AIServiceProvider.Anthropic`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Anthropic;
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

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

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
    if (!this.cacheTimestamp) return false;
    const age = Date.now() - this.cacheTimestamp;
    return age < CACHE_TTL;
  }

  /**
   * Selects the best available model following priority order.
   * Priority: explicit option > config default > first available config model > safe fallback
   * @private
   */
  private getDefaultModel(): string {
    const logger = getLogger('AnthropicService.getDefaultModel');

    // Priority 1: Check config default
    if (this.config?.defaultModel) {
      logger.debug(`Using config default model: ${this.config.defaultModel}`);
      return this.config.defaultModel;
    }

    // Priority 2: First model from config
    const configModels = llmConfigManager.getModelsForProvider('anthropic');
    if (configModels && Object.keys(configModels).length > 0) {
      const firstModel = Object.keys(configModels)[0];
      logger.debug(`Using first config model: ${firstModel}`);
      return firstModel;
    }

    // Priority 3: Safe fallback (verified to exist in current config)
    const fallback = 'claude-3-5-sonnet-20241022';
    logger.warn(`No config models found, using fallback: ${fallback}`);
    return fallback;
  }

  /**
   * Validates that the fallback model exists in config
   * @private
   */
  private validateFallbackModel(): void {
    const fallback = 'claude-3-5-sonnet-20241022';
    const model = llmConfigManager.getModel('anthropic', fallback);
    if (!model) {
      this.logger.error(
        `Fallback model ${fallback} not found in config. Update getDefaultModel() to use a valid fallback.`,
      );
    } else {
      this.logger.debug(`Fallback model ${fallback} validated successfully`);
    }
  }

  /**
   * Initiates a streaming chat session with the Anthropic API.
   * It handles message conversion, tool use, and processes the streaming response,
   * including partial JSON accumulation for tool calls and 'thinking' state updates.
   *
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat, including model name, system prompt, and tools.
   * @yields A JSON string for each chunk of the response. The format can be `{ content: string }`
   *         for text, `{ thinking: object }` for thinking state, or `{ tool_calls: [...] }` for tool calls.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
    } = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools, sanitizedMessages } = this.prepareStreamChat(
      messages,
      options,
    );

    try {
      const anthropicMessages = convertToAnthropicMessages(sanitizedMessages);

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

      const stream = this.anthropic.messages.stream(
        {
          model: model,
          max_tokens: config.maxTokens!,
          messages: anthropicMessages,
          system: options.systemPrompt,
          ...(extendedThinking && { extended_thinking: extendedThinking }),
          tools: tools as AnthropicTool[],
          ...(options.forceToolUse &&
            options.availableTools?.length && { tool_choice: { type: 'any' } }),
        },
        { signal: this.getAbortSignal() },
      );

      const toolAccumulator = new ToolCallStreamAccumulator();

      // Track current usage metrics (updated from message_start and message_delta)
      let currentUsage: TokenUsage = {
        promptTokens: 0,
        completionTokens: 0,
        totalTokens: 0,
      };

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
            const u = chunk.message.usage as AnthropicUsageWithCache;
            currentUsage.promptTokens = u.input_tokens || 0;
            currentUsage.totalTokens =
              currentUsage.promptTokens + currentUsage.completionTokens;

            // Extract prompt caching stats if available
            if (
              u.cache_creation_input_tokens !== undefined ||
              u.cache_read_input_tokens !== undefined
            ) {
              currentUsage.details = {
                ...currentUsage.details,
                cacheCreationInputTokens: u.cache_creation_input_tokens,
                cacheReadInputTokens: u.cache_read_input_tokens,
              };
            }
            yield JSON.stringify({ usage: currentUsage });
          }
        }

        if (chunk.type === 'message_delta') {
          if (chunk.usage) {
            // Update completion tokens
            currentUsage.completionTokens = chunk.usage.output_tokens || 0;

            // Update input tokens if provided (usually in message_start, but just in case)
            if (chunk.usage.input_tokens) {
              currentUsage.promptTokens = chunk.usage.input_tokens;
            }

            currentUsage.totalTokens =
              currentUsage.promptTokens + currentUsage.completionTokens;

            yield JSON.stringify({ usage: currentUsage });
          }
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
          toolAccumulator.handleContentBlockStart(chunk);
        } else if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'input_json_delta'
        ) {
          const result = toolAccumulator.handleInputJsonDelta(chunk);
          if (result) {
            yield JSON.stringify(result);
          }
        } else if (chunk.type === 'content_block_stop') {
          const result = toolAccumulator.handleContentBlockStop(chunk);
          if (result) {
            yield JSON.stringify(result);
          }
        }
      }
    } catch (error) {
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * @inheritdoc
   * @description For Anthropic, system messages are handled as a separate parameter
   * in the API call, so this method returns null.
   * @protected
   */
  protected createSystemMessage(systemPrompt: string): unknown {
    // Anthropic handles system messages separately as a parameter, not as a message
    void systemPrompt;
    return null;
  }

  /**
   * @inheritdoc
   * @description Converts a single `Message` into the format expected by the Anthropic API.
   * @protected
   */
  protected convertSingleMessage(message: Message): unknown {
    return convertSingleAnthropicMessage(message);
  }

  /**
   * @inheritdoc
   * @description The Anthropic SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    // Anthropic SDK doesn't require explicit cleanup
  }
}
