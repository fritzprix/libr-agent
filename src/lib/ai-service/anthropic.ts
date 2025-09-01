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

      for await (const chunk of completion) {
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
          chunk.delta.type === 'input_json_delta'
        ) {
          yield JSON.stringify({ tool_calls: chunk.delta.partial_json });
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
        if (m.tool_calls) {
          anthropicMessages.push({
            role: 'assistant',
            content: m.tool_calls.map((tc) => ({
              type: 'tool_use' as const,
              id: tc.id,
              name: tc.function.name,
              input: JSON.parse(tc.function.arguments),
            })),
          });
        } else if (m.tool_use) {
          anthropicMessages.push({
            role: 'assistant',
            content: [
              {
                type: 'tool_use' as const,
                id: m.tool_use.id,
                name: m.tool_use.name,
                input: m.tool_use.input,
              },
            ],
          });
        } else {
          anthropicMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content),
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
      if (message.tool_calls) {
        return {
          role: 'assistant',
          content: message.tool_calls.map((tc) => ({
            type: 'tool_use' as const,
            id: tc.id,
            name: tc.function.name,
            input: JSON.parse(tc.function.arguments),
          })),
        };
      } else if (message.tool_use) {
        return {
          role: 'assistant',
          content: [
            {
              type: 'tool_use' as const,
              id: message.tool_use.id,
              name: message.tool_use.name,
              input: message.tool_use.input,
            },
          ],
        };
      } else {
        return {
          role: 'assistant',
          content: this.processMessageContent(message.content),
        };
      }
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
