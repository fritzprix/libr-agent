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
const logger = getLogger('AnthropicService');

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
}

/**
 * An AI service implementation for interacting with Anthropic's language models (e.g., Claude).
 * It handles the specifics of the Anthropic API, including message formatting,
 * tool use, and streaming.
 */
export class AnthropicService extends BaseAIService {
  private anthropic: Anthropic;

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
  }

  /**
   * Gets the provider identifier.
   * @returns `AIServiceProvider.Anthropic`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Anthropic;
  }

  /**
   * Determines whether to enable the 'thinking' feature based on the model name.
   * Extended thinking is available for Claude 3.5 Sonnet and later models.
   * @param modelName The name of the model.
   * @param config The service configuration.
   * @returns True if the thinking feature should be enabled, false otherwise.
   * @private
   */
  private shouldEnableThinking(
    modelName?: string,
    config?: AIServiceConfig,
  ): boolean {
    // Only enable thinking for models that support it
    // Extended thinking is available for Claude 3.5 Sonnet and later models
    const model =
      modelName || config?.defaultModel || 'claude-3-sonnet-20240229';
    return model.includes('claude-3-5') || model.includes('claude-3-opus');
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
    } = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools, sanitizedMessages } = this.prepareStreamChat(
      messages,
      options,
    );

    try {
      const anthropicMessages =
        this.convertToAnthropicMessages(sanitizedMessages);

      const shouldEnableThinking = this.shouldEnableThinking(
        options.modelName,
        config,
      );
      const completion = await this.withRetry(() =>
        this.anthropic.messages.create({
          model:
            options.modelName ||
            config.defaultModel ||
            'claude-3-sonnet-20240229',
          max_tokens: config.maxTokens!,
          messages: anthropicMessages,
          stream: true,
          ...(shouldEnableThinking && {
            thinking: {
              budget_tokens: 1024,
              type: 'enabled',
            },
          }),
          system: options.systemPrompt,
          tools: tools as AnthropicTool[],
        }),
      );

      // Tool call accumulator for partial JSON streaming
      const toolCallAccumulators = new Map<number, ToolCallAccumulator>();

      for await (const chunk of completion) {
        logger.debug('Received chunk from Anthropic', { chunk });

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
            toolCallAccumulators.set(chunk.index, {
              id: chunk.content_block.id,
              name: chunk.content_block.name,
              partialJson: '',
              index: chunk.index,
              yielded: false, // Initial value is false
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
            accumulator.partialJson += chunk.delta.partial_json;
            logger.debug('Accumulated partial JSON', {
              index: chunk.index,
              partialJson: accumulator.partialJson,
            });

            // Try to parse the accumulated JSON only if not already yielded
            if (!accumulator.yielded) {
              try {
                const parsedInput = JSON.parse(accumulator.partialJson);
                // If parsing succeeds, yield the tool call and mark as yielded
                yield JSON.stringify({
                  tool_calls: [
                    {
                      id: accumulator.id,
                      function: {
                        name: accumulator.name,
                        arguments: JSON.stringify(parsedInput),
                      },
                    },
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
          // Final attempt to parse accumulated JSON only if not already yielded
          const accumulator = toolCallAccumulators.get(chunk.index);
          if (accumulator && accumulator.partialJson && !accumulator.yielded) {
            try {
              const parsedInput = JSON.parse(accumulator.partialJson);
              logger.info('Tool call completed on content_block_stop', {
                id: accumulator.id,
                name: accumulator.name,
                input: parsedInput,
              });
              // Final tool call yield if not already done
              yield JSON.stringify({
                tool_calls: [
                  {
                    id: accumulator.id,
                    function: {
                      name: accumulator.name,
                      arguments: JSON.stringify(parsedInput),
                    },
                  },
                ],
              });
              accumulator.yielded = true;
            } catch (parseError) {
              logger.error('Failed to parse final tool call JSON', {
                error: parseError,
                partialJson: accumulator.partialJson,
                toolId: accumulator.id,
                toolName: accumulator.name,
              });
            }
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
      logger.error(
        'Tool chain integrity violation detected before Anthropic API call',
        {
          unmatchedToolUses,
          unmatchedToolResults,
          totalMessages: messages.length,
          toolUseIds: Array.from(toolUseIds),
          toolResultIds: Array.from(toolResultIds),
        },
      );
      // Throw an error at this point to detect the problem before the API call
      throw new Error(
        `Incomplete tool chain: ${unmatchedToolUses.length} unmatched tool_use, ${unmatchedToolResults.length} unmatched tool_result`,
      );
    }

    logger.debug('Tool chain integrity verification passed', {
      totalMessages: messages.length,
      toolUseCount: toolUseIds.size,
      toolResultCount: toolResultIds.size,
    });

    for (const m of messages) {
      if (m.role === 'system') {
        // System messages are handled separately in the API call
        continue;
      }

      if (m.role === 'user') {
        anthropicMessages.push({
          role: 'user',
          content: this.processMessageContent(m.content),
        });
      } else if (m.role === 'assistant') {
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
              input: JSON.parse(tc.function.arguments),
            })),
          );
        } else if (m.tool_use) {
          content.push({
            type: 'tool_use' as const,
            id: m.tool_use.id,
            name: m.tool_use.name,
            input: m.tool_use.input,
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
      } else if (m.role === 'tool') {
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
            input: JSON.parse(tc.function.arguments),
          })),
        );
      } else if (message.tool_use) {
        content.push({
          type: 'tool_use' as const,
          id: message.tool_use.id,
          name: message.tool_use.name,
          input: message.tool_use.input,
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
}
