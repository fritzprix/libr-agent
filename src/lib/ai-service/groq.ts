import Groq from 'groq-sdk';
import { ChatCompletionTool as GroqChatCompletionTool } from 'groq-sdk/resources/chat/completions.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { llmConfigManager } from '../llm-config-manager';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
const logger = getLogger('GroqService');

export class GroqService extends BaseAIService {
  private groq: Groq;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.groq = new Groq({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Groq;
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
    logger.info('tools : ', { availableTools: options.availableTools });

    try {
      const groqMessages = this.convertToGroqMessages(
        messages,
        options.systemPrompt,
      );

      const model = llmConfigManager.getModel(
        'groq',
        options.modelName || config.defaultModel || 'llama-3.1-8b-instant',
      );

      const chatCompletion = await this.withRetry(() =>
        this.groq.chat.completions.create({
          messages: groqMessages,
          model:
            options.modelName || config.defaultModel || 'llama-3.1-8b-instant',
          temperature: config.temperature,
          max_tokens: config.maxTokens,
          reasoning_format: model?.supportReasoning ? 'parsed' : undefined,
          stream: true,
          tools: tools as GroqChatCompletionTool[],
          tool_choice: options.availableTools ? 'auto' : undefined,
        }),
      );

      for await (const chunk of chatCompletion) {
        if (chunk.choices[0]?.delta?.reasoning) {
          yield JSON.stringify({ thinking: chunk.choices[0].delta.reasoning });
        } else if (chunk.choices[0]?.delta?.tool_calls) {
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

  private convertToGroqMessages(
    messages: Message[],
    systemPrompt?: string,
  ): Groq.Chat.Completions.ChatCompletionMessageParam[] {
    const groqMessages: Groq.Chat.Completions.ChatCompletionMessageParam[] = [];

    if (systemPrompt) {
      groqMessages.push({ role: 'system', content: systemPrompt });
    }

    for (const m of messages) {
      if (m.role === 'user') {
        groqMessages.push({
          role: 'user',
          content: this.processMessageContent(m.content),
        });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          groqMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content) || null,
            tool_calls: m.tool_calls,
          });
        } else if (m.thinking) {
          groqMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content),
          });
        } else {
          groqMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content),
          });
        }
      } else if (m.role === 'tool') {
        if (m.tool_call_id) {
          groqMessages.push({
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
    return groqMessages;
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
      } else if (message.thinking) {
        return {
          role: 'assistant',
          content: this.processMessageContent(message.content),
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
    // Groq SDK doesn't require explicit cleanup
  }
}
