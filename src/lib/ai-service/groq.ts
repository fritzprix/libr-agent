import Groq from 'groq-sdk';
import { ChatCompletionTool as GroqChatCompletionTool } from 'groq-sdk/resources/chat/completions.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { llmConfigManager } from '../llm-config-manager';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';
import { convertMCPToolsToProviderTools } from './tool-converters';

const logger = getLogger('AIService');

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
    this.validateMessages(messages);
    logger.info('tools : ', { availableTools: options.availableTools });

    const config = { ...this.defaultConfig, ...options.config };

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
          tools: options.availableTools
            ? (convertMCPToolsToProviderTools(
                options.availableTools,
                AIServiceProvider.Groq,
              ) as GroqChatCompletionTool[])
            : undefined,
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
      throw new AIServiceError(
        `Groq streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        AIServiceProvider.Groq,
        undefined,
        error instanceof Error ? error : undefined,
      );
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
        groqMessages.push({ role: 'user', content: m.content });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          groqMessages.push({
            role: 'assistant',
            content: m.content || null,
            tool_calls: m.tool_calls,
          });
        } else if (m.thinking) {
          groqMessages.push({
            role: 'assistant',
            content: m.content,
          });
        } else {
          groqMessages.push({ role: 'assistant', content: m.content });
        }
      } else if (m.role === 'tool') {
        if (m.tool_call_id) {
          groqMessages.push({
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
    return groqMessages;
  }

  dispose(): void {
    // Groq SDK doesn't require explicit cleanup
  }
}
