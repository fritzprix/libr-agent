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

interface ToolCallAccumulator {
  id: string;
  name: string;
  partialJson: string;
  index: number;
  yielded: boolean; // 이미 yield했는지 추적
}

export class AnthropicService extends BaseAIService {
  private anthropic: Anthropic;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.anthropic = new Anthropic({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Anthropic;
  }

  async *streamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    } = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools } = this.prepareStreamChat(messages, options);

    try {
      const anthropicMessages = this.convertToAnthropicMessages(messages);

      const completion = await this.withRetry(() =>
        this.anthropic.messages.create({
          model:
            options.modelName ||
            config.defaultModel ||
            'claude-3-sonnet-20240229',
          max_tokens: config.maxTokens!,
          messages: anthropicMessages,
          stream: true,
          thinking: {
            budget_tokens: 1024,
            type: 'enabled',
          },
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
              yielded: false, // 초기값은 false
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
                accumulator.yielded = true; // 중복 yield 방지
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

  private convertToAnthropicMessages(
    messages: Message[],
  ): AnthropicMessageParam[] {
    const anthropicMessages: AnthropicMessageParam[] = [];

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
        const hasContent =
          m.content &&
          (typeof m.content === 'string'
            ? m.content.trim().length > 0
            : m.content.length > 0);
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
            signature: m.thinkingSignature,
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
          if (Array.isArray(processedContent)) {
            content.push(...processedContent);
          } else {
            content.push({ type: 'text' as const, text: processedContent });
          }
        }

        if (content.length > 0) {
          anthropicMessages.push({
            role: 'assistant',
            content,
          });
        }
      } else if (m.role === 'tool') {
        anthropicMessages.push({
          role: 'user',
          content: [
            {
              type: 'tool_result' as const,
              tool_use_id: m.tool_call_id!,
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

  // Implementation of abstract methods from BaseAIService
  protected createSystemMessage(systemPrompt: string): unknown {
    // Anthropic handles system messages separately as a parameter, not as a message
    void systemPrompt;
    return null;
  }

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
          signature: message.thinkingSignature,
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
        if (Array.isArray(processedContent)) {
          content.push(...processedContent);
        } else {
          content.push({ type: 'text' as const, text: processedContent });
        }
      }

      return {
        role: 'assistant',
        content,
      };
    } else if (message.role === 'tool') {
      return {
        role: 'user',
        content: [
          {
            type: 'tool_result' as const,
            tool_use_id: message.tool_call_id!,
            content: this.processMessageContent(message.content),
          },
        ],
      };
    } else {
      logger.warn(`Unsupported message role for Anthropic: ${message.role}`);
      return null;
    }
  }

  dispose(): void {
    // Anthropic SDK doesn't require explicit cleanup
  }
}
