import OpenAI from 'openai';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';
import { convertMCPToolsToProviderTools } from './tool-converters';

const logger = getLogger('AIService');

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
    this.validateMessages(messages);

    const config = { ...this.defaultConfig, ...options.config };
    const provider = this.getProvider();

    if (options.availableTools) {
      logger.info('tool calls: ', {
        tools: convertMCPToolsToProviderTools(
          options?.availableTools,
          provider,
        ),
      });
    }

    try {
      const openaiMessages = this.convertToOpenAIMessages(
        messages,
        options.systemPrompt,
      );

      let modelName = options.modelName || config.defaultModel || 'gpt-4-turbo';

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
          tools: options.availableTools
            ? (convertMCPToolsToProviderTools(
                options.availableTools,
                provider,
              ) as OpenAIChatCompletionTool[])
            : undefined,
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
      const serviceProvider = this.getProvider();
      let modelName = options.modelName || config.defaultModel || 'gpt-4-turbo';
      const fireworksPrefix = 'accounts/fireworks/models/';
      if (
        serviceProvider === AIServiceProvider.Fireworks &&
        !modelName.startsWith(fireworksPrefix)
      ) {
        modelName = `${fireworksPrefix}${modelName}`;
      }
      logger.error(`${serviceProvider} streaming failed`, {
        error: error,
        message: error instanceof Error ? error.message : 'Unknown error',
        stack: error instanceof Error ? error.stack : undefined,
        requestData: {
          model: modelName,
          messagesCount: messages.length,
          hasTools: !!options.availableTools?.length,
          systemPrompt: !!options.systemPrompt,
        },
      });
      throw new AIServiceError(
        `${serviceProvider} streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        serviceProvider,
        undefined,
        error instanceof Error ? error : undefined,
      );
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
        openaiMessages.push({ role: 'user', content: m.content });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          openaiMessages.push({
            role: 'assistant',
            content: m.content || null,
            tool_calls: m.tool_calls,
          });
        } else {
          openaiMessages.push({ role: 'assistant', content: m.content });
        }
      } else if (m.role === 'tool') {
        if (m.tool_call_id) {
          openaiMessages.push({
            role: 'tool',
            tool_call_id: m.tool_call_id,
            content: m.content,
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

  dispose(): void {
    // OpenAI SDK doesn't require explicit cleanup
  }
}
