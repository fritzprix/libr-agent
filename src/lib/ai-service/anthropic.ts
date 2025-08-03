import Anthropic from '@anthropic-ai/sdk';
import {
  MessageParam as AnthropicMessageParam,
  Tool as AnthropicTool,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';
import { convertMCPToolsToProviderTools } from './tool-converters';
import { MessageValidator } from './validators';

const logger = getLogger('AIService');

export class AnthropicService extends BaseAIService {
  private anthropic: Anthropic;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.anthropic = new Anthropic({ apiKey: this.apiKey });
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
    this.validateMessages(messages);

    const config = { ...this.defaultConfig, ...options.config };

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
          tools: options.availableTools
            ? (convertMCPToolsToProviderTools(
                options.availableTools,
                AIServiceProvider.Anthropic,
              ) as AnthropicTool[])
            : undefined,
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
      throw new AIServiceError(
        `Anthropic streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        AIServiceProvider.Anthropic,
        undefined,
        error instanceof Error ? error : undefined,
      );
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
        anthropicMessages.push({ role: 'user', content: m.content });
      } else if (m.role === 'assistant') {
        if (m.tool_calls) {
          anthropicMessages.push({
            role: 'assistant',
            content: m.tool_calls.map((tc) => ({
              type: 'tool_use' as const,
              id: tc.id,
              name: tc.function.name,
              input: MessageValidator.sanitizeToolArguments(
                tc.function.arguments,
              ),
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
          anthropicMessages.push({ role: 'assistant', content: m.content });
        }
      } else if (m.role === 'tool') {
        anthropicMessages.push({
          role: 'user',
          content: [
            {
              type: 'tool_result' as const,
              tool_use_id: m.tool_call_id!,
              content: m.content,
            },
          ],
        });
      } else {
        logger.warn(`Unsupported message role for Anthropic: ${m.role}`);
      }
    }
    return anthropicMessages;
  }

  dispose(): void {
    // Anthropic SDK doesn't require explicit cleanup
  }
}
