import OpenAI from 'openai';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
const logger = getLogger('OpenAIService');

export class OpenAIService extends BaseAIService {
  protected openai: OpenAI;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.OpenAI;
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
    const provider = this.getProvider();

    if (options.availableTools) {
      logger.info('tool calls: ', { tools });
    }

    try {
      const openaiMessages = this.convertToOpenAIMessages(
        messages,
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

      logger.info(`${provider} call : `, {
        model: modelName,
        messages: openaiMessages,
      });

      const completion = await this.withRetry(() =>
        this.openai.chat.completions.create({
          model: modelName,
          messages: openaiMessages,
          max_completion_tokens: config.maxTokens,
          stream: true,
          tools: tools as OpenAIChatCompletionTool[],
          tool_choice: options.availableTools ? 'auto' : undefined,
        }),
      );

      for await (const chunk of completion) {
        if (chunk.choices[0]?.delta?.tool_calls) {
          yield JSON.stringify({
            tool_calls: chunk.choices[0].delta.tool_calls,
          });
        } else if (chunk.choices[0]?.delta?.content) {
          yield JSON.stringify({
            content: chunk.choices[0]?.delta?.content || '',
          });
        }
      }
    } catch (error) {
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  private convertToOpenAIMessages(
    messages: Message[],
    systemPrompt?: string,
  ): OpenAI.Chat.Completions.ChatCompletionMessageParam[] {
    const openaiMessages: OpenAI.Chat.Completions.ChatCompletionMessageParam[] =
      [];

    if (systemPrompt) {
      openaiMessages.push({ role: 'system', content: systemPrompt });
    }

    for (const m of messages) {
      if (m.role === 'user') {
        openaiMessages.push({
          role: 'user',
          content: this.processMessageContent(m.content),
        });
      } else if (m.role === 'assistant') {
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
      } else if (m.role === 'tool') {
        if (m.tool_call_id) {
          openaiMessages.push({
            role: 'tool',
            tool_call_id: m.tool_call_id,
            content: this.processMessageContent(m.content),
          });
        } else {
          logger.warn(
            `Tool message missing tool_call_id: ${JSON.stringify(m)}`,
          );
        }
      }
    }
    return openaiMessages;
  }

  // Implementation of abstract methods from BaseAIService
  protected createSystemMessage(systemPrompt: string): unknown {
    return { role: 'system', content: systemPrompt };
  }

  protected convertSingleMessage(message: Message): unknown {
    if (message.role === 'user') {
      return {
        role: 'user',
        content: this.processMessageContent(message.content),
      };
    } else if (message.role === 'assistant') {
      if (message.tool_calls && message.tool_calls.length > 0) {
        return {
          role: 'assistant',
          content: this.processMessageContent(message.content) || null,
          tool_calls: message.tool_calls,
        };
      } else {
        return {
          role: 'assistant',
          content: this.processMessageContent(message.content),
        };
      }
    } else if (message.role === 'tool') {
      if (message.tool_call_id) {
        return {
          role: 'tool',
          tool_call_id: message.tool_call_id,
          content: this.processMessageContent(message.content),
        };
      } else {
        logger.warn(
          `Tool message missing tool_call_id: ${JSON.stringify(message)}`,
        );
        return null;
      }
    } else if (message.role === 'system') {
      return {
        role: 'system',
        content: this.processMessageContent(message.content),
      };
    }
    return null;
  }

  dispose(): void {
    // OpenAI SDK doesn't require explicit cleanup
  }
}
