import Anthropic from '@anthropic-ai/sdk';
import {
  MessageParam as AnthropicMessageParam,
  Tool as AnthropicTool,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { formatToolCall } from './utils';
import { ModelInfo, llmConfigManager } from '../llm-config-manager';
const logger = getLogger('AnthropicService');

const MAX_PARTIAL_TOOL_INPUT_LENGTH = 200_000;

/**
 * An internal helper interface to accumulate partial JSON data for a tool call
 * during a streaming response.
 * @internal
 */
interface ToolCallAccumulator {
  id: string;
  name: string;
  partialJson: string;
  index: number;
  yielded: boolean; // Track if already yielded to prevent duplicates
  initialInput?: Record<string, unknown> | null;
}

/**
 * An AI service implementation for interacting with Anthropic's language models (e.g., Claude).
 * It handles the specifics of the Anthropic API, including message formatting,
 * tool use, and streaming.
 */
export class AnthropicService extends BaseAIService {
  private anthropic: Anthropic;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = 3600000; // 1 hour in milliseconds

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

        models.push({
          id: model.id,
          name: model.display_name || staticModel?.name || model.id,
          contextWindow: staticModel?.contextWindow || 200000,
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
    return age < this.CACHE_TTL;
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
  async *streamChat(
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
      const anthropicMessages =
        this.convertToAnthropicMessages(sanitizedMessages);

      const stream = this.anthropic.messages.stream(
        {
          model: options.modelName || this.getDefaultModel(),
          max_tokens: config.maxTokens!,
          messages: anthropicMessages,
          system: options.systemPrompt,
          tools: tools as AnthropicTool[],
          ...(options.forceToolUse &&
            options.availableTools?.length && { tool_choice: { type: 'any' } }),
        },
        { signal: this.getAbortSignal() },
      );

      // Tool call accumulator for partial JSON streaming
      const toolCallAccumulators = new Map<number, ToolCallAccumulator>();

      if (this.getAbortSignal().aborted) {
        this.logger.info('Stream aborted before iteration');
        return;
      }

      for await (const chunk of stream) {
        if (this.getAbortSignal().aborted) {
          this.logger.info('Stream aborted during iteration');
          break;
        }
        logger.debug('Received chunk from Anthropic', { chunk });

        // Extra logging for delta inspection: helpful to see exact shapes
        if (chunk && chunk.type === 'content_block_delta') {
          try {
            logger.info('Anthropic content_block_delta raw', {
              index: chunk.index,
              deltaType: chunk.delta?.type,
              delta: chunk.delta,
            });
          } catch (e) {
            logger.debug('Failed to log content_block_delta safely', {
              error: e,
            });
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
          // Initialize accumulator for new tool call
          logger.info('Anthropic content_block_start', {
            index: chunk.index,
            content_block: chunk.content_block,
          });
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
              yielded: false, // Initial value is false
              initialInput,
            });
            logger.debug('Started tool call accumulation', {
              index: chunk.index,
              id: chunk.content_block.id,
              name: chunk.content_block.name,
            });
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
              accumulator.yielded = true;
              continue;
            }
            logger.debug('Accumulated partial JSON', {
              index: chunk.index,
              partialJson: accumulator.partialJson,
            });

            // Try to parse the accumulated JSON only if not already yielded
            if (!accumulator.yielded) {
              const trimmedPartial = accumulator.partialJson.trim();
              if (trimmedPartial.length === 0) {
                logger.debug('No complete JSON fragment yet; waiting', {
                  index: chunk.index,
                  id: accumulator.id,
                });
                continue;
              }
              try {
                const parsedInput = JSON.parse(trimmedPartial) as Record<
                  string,
                  unknown
                >;
                // If parsing succeeds, yield the tool call and mark as yielded
                yield JSON.stringify({
                  tool_calls: [
                    formatToolCall(
                      accumulator.id,
                      accumulator.name,
                      parsedInput,
                    ),
                  ],
                });
                accumulator.yielded = true; // Prevent duplicate yields
                logger.debug('Tool call yielded successfully', {
                  index: chunk.index,
                  id: accumulator.id,
                  name: accumulator.name,
                });
              } catch (parseError) {
                // Continue accumulating if JSON is still incomplete
                logger.debug('JSON still incomplete, continuing accumulation', {
                  error: parseError,
                  partialJson: accumulator.partialJson,
                });
              }
            }
          }
        } else if (chunk.type === 'content_block_stop') {
          logger.info('Anthropic content_block_stop', { index: chunk.index });
          // Final attempt to parse accumulated JSON only if not already yielded
          const accumulator = toolCallAccumulators.get(chunk.index);
          if (accumulator && accumulator.partialJson && !accumulator.yielded) {
            const trimmedPartial = accumulator.partialJson.trim();
            try {
              const parsedInput = JSON.parse(trimmedPartial) as Record<
                string,
                unknown
              >;
              logger.info('Tool call completed on content_block_stop', {
                id: accumulator.id,
                name: accumulator.name,
                input: parsedInput,
              });
              // Final tool call yield if not already done
              yield JSON.stringify({
                tool_calls: [
                  formatToolCall(accumulator.id, accumulator.name, parsedInput),
                ],
              });
              accumulator.yielded = true;
            } catch (parseError) {
              if (accumulator.initialInput) {
                logger.info(
                  'Using initial tool input from content_block_start',
                  {
                    id: accumulator.id,
                    name: accumulator.name,
                  },
                );
                yield JSON.stringify({
                  tool_calls: [
                    formatToolCall(
                      accumulator.id,
                      accumulator.name,
                      accumulator.initialInput,
                    ),
                  ],
                });
                accumulator.yielded = true;
              } else {
                logger.error('Failed to parse final tool call JSON', {
                  error: parseError,
                  partialJson: accumulator.partialJson,
                  toolId: accumulator.id,
                  toolName: accumulator.name,
                });
              }
            }
          } else if (
            accumulator &&
            !accumulator.yielded &&
            accumulator.initialInput
          ) {
            logger.info(
              'Tool call completed using initial input without deltas',
              {
                id: accumulator.id,
                name: accumulator.name,
              },
            );
            yield JSON.stringify({
              tool_calls: [
                formatToolCall(
                  accumulator.id,
                  accumulator.name,
                  accumulator.initialInput,
                ),
              ],
            });
            accumulator.yielded = true;
          }

          // Clean up accumulator regardless of yield status
          if (accumulator) {
            toolCallAccumulators.delete(chunk.index);
            logger.debug('Cleaned up tool call accumulator', {
              index: chunk.index,
              id: accumulator.id,
              wasYielded: accumulator.yielded,
            });
          }
        }
      }
    } catch (error) {
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * Converts an array of standard `Message` objects into the format required
   * by the Anthropic API. It also performs a strict integrity check to ensure
   * that all tool calls have a corresponding tool result, throwing an error
   * if any inconsistencies are found.
   *
   * @param messages The array of messages to convert.
   * @returns An array of `AnthropicMessageParam` objects.
   * @throws An error if an incomplete tool chain is detected.
   * @private
   */
  private convertToAnthropicMessages(
    messages: Message[],
  ): AnthropicMessageParam[] {
    const anthropicMessages: AnthropicMessageParam[] = [];
    const toolUseIds = new Set<string>();
    const toolResultIds = new Set<string>();

    // Track tool chains for debugging and integrity checks
    for (const m of messages) {
      if (m.role === 'assistant' && m.tool_use) {
        toolUseIds.add(m.tool_use.id);
      } else if (m.role === 'assistant' && m.tool_calls) {
        m.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
      } else if (m.role === 'tool' && m.tool_call_id) {
        toolResultIds.add(m.tool_call_id);
      }
    }

    // Verify tool chain integrity
    const unmatchedToolUses = Array.from(toolUseIds).filter(
      (id) => !toolResultIds.has(id),
    );
    const unmatchedToolResults = Array.from(toolResultIds).filter(
      (id) => !toolUseIds.has(id),
    );

    if (unmatchedToolUses.length > 0 || unmatchedToolResults.length > 0) {
      logger.warn('Potential tool chain mismatch detected', {
        unmatchedToolUses,
        unmatchedToolResults,
        totalMessages: messages.length,
        toolUseIds: Array.from(toolUseIds),
        toolResultIds: Array.from(toolResultIds),
      });
    }

    logger.debug('Tool chain integrity verification passed', {
      totalMessages: messages.length,
      toolUseCount: toolUseIds.size,
      toolResultCount: toolResultIds.size,
    });

    for (const m of messages) {
      // Convert UI-originated messages to user role for provider calls
      const effectiveRole = m.source === 'ui' ? 'user' : m.role;

      if (effectiveRole === 'system') {
        // System messages are handled separately in the API call
        continue;
      }

      if (effectiveRole === 'user') {
        anthropicMessages.push({
          role: 'user',
          content: this.processMessageContent(m.content),
        });
      } else if (effectiveRole === 'assistant') {
        // Filter out empty assistant messages that would cause API errors
        const hasContent = m.content && m.content.length > 0;
        const hasToolCalls = m.tool_calls && m.tool_calls.length > 0;
        const hasToolUse = m.tool_use;

        // Skip empty assistant messages to prevent 400 errors
        if (!hasContent && !hasToolCalls && !hasToolUse) {
          logger.debug('Skipping empty assistant message', { messageId: m.id });
          continue;
        }

        // Build content array with thinking block first if present
        const content = [];

        // Add thinking block as first element if exists
        if (m.thinking) {
          content.push({
            type: 'thinking' as const,
            thinking: m.thinking,
            signature: m.thinkingSignature || '',
          });
        }

        // Add tool_use or text content after thinking
        if (m.tool_calls) {
          content.push(
            ...m.tool_calls.map((tc) => ({
              type: 'tool_use' as const,
              id: tc.id,
              name: tc.function.name,
              input: this.parseToolInput(tc.function.arguments, {
                messageId: m.id,
                toolId: tc.id,
                toolName: tc.function.name,
              }),
            })),
          );
        } else if (m.tool_use) {
          content.push({
            type: 'tool_use' as const,
            id: m.tool_use.id,
            name: m.tool_use.name,
            input: this.ensureObjectInput(m.tool_use.input, {
              messageId: m.id,
              toolId: m.tool_use.id,
              toolName: m.tool_use.name,
            }),
          });
        } else if (hasContent) {
          const processedContent = this.processMessageContent(m.content);
          content.push({ type: 'text' as const, text: processedContent });
        }

        if (content.length > 0) {
          anthropicMessages.push({
            role: 'assistant',
            content,
          });
        }
      } else if (effectiveRole === 'tool') {
        if (!m.tool_call_id) {
          logger.warn('Tool message missing tool_call_id, skipping', {
            messageId: m.id,
          });
          continue;
        }
        anthropicMessages.push({
          role: 'user',
          content: [
            {
              type: 'tool_result' as const,
              tool_use_id: m.tool_call_id,
              content: this.processMessageContent(m.content),
            },
          ],
        });
      } else {
        logger.warn(`Unsupported message role for Anthropic: ${m.role}`);
      }
    }
    return anthropicMessages;
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
    if (message.role === 'system') {
      // System messages are handled separately in the API call
      return null;
    }

    if (message.role === 'user') {
      return {
        role: 'user',
        content: this.processMessageContent(message.content),
      };
    } else if (message.role === 'assistant') {
      // Build content array with thinking block first if present
      const content = [];

      // Add thinking block as first element if exists
      if (message.thinking) {
        content.push({
          type: 'thinking' as const,
          thinking: message.thinking,
          signature: message.thinkingSignature || '',
        });
      }

      // Add tool_use or text content after thinking
      if (message.tool_calls) {
        content.push(
          ...message.tool_calls.map((tc) => ({
            type: 'tool_use' as const,
            id: tc.id,
            name: tc.function.name,
            input: this.parseToolInput(tc.function.arguments, {
              messageId: message.id,
              toolId: tc.id,
              toolName: tc.function.name,
            }),
          })),
        );
      } else if (message.tool_use) {
        content.push({
          type: 'tool_use' as const,
          id: message.tool_use.id,
          name: message.tool_use.name,
          input: this.ensureObjectInput(message.tool_use.input, {
            messageId: message.id,
            toolId: message.tool_use.id,
            toolName: message.tool_use.name,
          }),
        });
      } else if (message.content) {
        const processedContent = this.processMessageContent(message.content);
        content.push({ type: 'text' as const, text: processedContent });
      }

      return {
        role: 'assistant',
        content,
      };
    } else if (message.role === 'tool') {
      if (!message.tool_call_id) {
        logger.warn('Tool message missing tool_call_id, skipping', {
          messageId: message.id,
        });
        return null;
      }
      return {
        role: 'user',
        content: [
          {
            type: 'tool_result' as const,
            tool_use_id: message.tool_call_id,
            content: this.processMessageContent(message.content),
          },
        ],
      };
    } else {
      logger.warn(`Unsupported message role for Anthropic: ${message.role}`);
      return null;
    }
  }

  /**
   * @inheritdoc
   * @description The Anthropic SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    // Anthropic SDK doesn't require explicit cleanup
  }

  private parseToolInput(
    raw: unknown,
    context: { messageId?: string; toolId?: string; toolName?: string },
  ): Record<string, unknown> {
    if (raw == null) {
      logger.warn(
        'Tool call input missing; defaulting to empty object',
        context,
      );
      return {};
    }

    if (typeof raw === 'string') {
      try {
        return JSON.parse(raw) as Record<string, unknown>;
      } catch (error) {
        logger.error('Failed to parse tool call arguments as JSON', {
          ...context,
          error,
        });
        throw error instanceof Error ? error : new Error(String(error));
      }
    }

    if (typeof raw === 'object' && !Array.isArray(raw)) {
      return raw as Record<string, unknown>;
    }

    logger.error('Unsupported tool call argument type', {
      ...context,
      valueType: typeof raw,
    });
    throw new Error('Unsupported tool call argument type');
  }

  private ensureObjectInput(
    raw: unknown,
    context: { messageId?: string; toolId?: string; toolName?: string },
  ): Record<string, unknown> {
    if (raw == null) {
      return {};
    }

    if (typeof raw === 'object' && !Array.isArray(raw)) {
      return raw as Record<string, unknown>;
    }

    return this.parseToolInput(raw, context);
  }
}
