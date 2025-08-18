import {
  FunctionDeclaration,
  GoogleGenAI,
  Content,
  FunctionCall,
} from '@google/genai';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';
import { convertMCPToolsToProviderTools } from './tool-converters';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('GeminiService');

interface GeminiServiceConfig {
  responseMimeType: string;
  tools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
  systemInstruction?: Array<{ text: string }>;
  maxOutputTokens?: number;
  temperature?: number;
}

function tryParse<T = unknown>(input?: string): T | undefined {
  if (!input) return undefined;
  try {
    return JSON.parse(input) as T;
  } catch {
    return undefined;
  }
}

export class GeminiService extends BaseAIService {
  private genAI: GoogleGenAI;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.genAI = new GoogleGenAI({
      apiKey: this.apiKey,
    });
  }

  private generateToolCallId(): string {
    return `tool_${createId()}`;
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Gemini;
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

    this.logToolResponseStats(messages);

    const validatedMessages = this.validateGeminiMessageStack(messages);

    const config = { ...this.defaultConfig, ...options.config };

    try {
      const geminiMessages = this.convertToGeminiMessages(validatedMessages);
      const tools = options.availableTools
        ? [
            {
              functionDeclarations: convertMCPToolsToProviderTools(
                options.availableTools,
                AIServiceProvider.Gemini,
              ) as FunctionDeclaration[],
            },
          ]
        : undefined;

      const model =
        options.modelName || config.defaultModel || 'gemini-1.5-pro';

      const geminiConfig: GeminiServiceConfig = {
        responseMimeType: 'text/plain',
      };

      if (tools) {
        geminiConfig.tools = tools;
      }

      if (options.systemPrompt) {
        geminiConfig.systemInstruction = [{ text: options.systemPrompt }];
      }

      if (config.maxTokens) {
        geminiConfig.maxOutputTokens = config.maxTokens;
      }

      if (config.temperature !== undefined) {
        geminiConfig.temperature = config.temperature;
      }

      logger.info('gemini final request: ', {
        model: model,
        config: geminiConfig,
        contents: geminiMessages,
      });

      const result = await this.withRetry(async () => {
        return this.genAI.models.generateContentStream({
          model: model,
          config: geminiConfig,
          contents: geminiMessages,
        });
      });

      for await (const chunk of result) {
        if (chunk.functionCalls && chunk.functionCalls.length > 0) {
          yield JSON.stringify({
            tool_calls: chunk.functionCalls.map((fc: FunctionCall) => {
              const toolCallId = this.generateToolCallId();

              logger.debug('Generated tool call ID', {
                functionName: fc.name,
                toolCallId,
              });

              return {
                id: toolCallId,
                type: 'function',
                function: {
                  name: fc.name,
                  arguments: JSON.stringify(fc.args),
                },
              };
            }),
          });
        } else if (chunk.text) {
          yield JSON.stringify({ content: chunk.text });
        }
      }
    } catch (error) {
      logger.error('Gemini API Error Details:', {
        error: error,
        message: error instanceof Error ? error.message : 'Unknown error',
        stack: error instanceof Error ? error.stack : undefined,
        requestData: {
          model: options.modelName || config.defaultModel || 'gemini-1.5-pro',
          originalMessagesCount: messages.length,
          validatedMessagesCount: validatedMessages.length,
          hasTools: !!options.availableTools?.length,
          systemPrompt: !!options.systemPrompt,
        },
      });
      throw new AIServiceError(
        `Gemini streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        AIServiceProvider.Gemini,
        undefined,
        error instanceof Error ? error : undefined,
      );
    }
  }

  private validateGeminiMessageStack(messages: Message[]): Message[] {
    if (messages.length === 0) {
      return messages;
    }

    const convertedMessages = messages.map((m) => {
      if (m.role === 'tool') {
        return { ...m, role: 'user' as const };
      }
      return m;
    });

    const firstUserIndex = convertedMessages.findIndex(
      (msg) => msg.role === 'user',
    );
    if (firstUserIndex === -1) {
      logger.warn('No user message found after role conversion');
      return [];
    }

    const validMessages = convertedMessages.slice(firstUserIndex);

    logger.info(
      `Role conversion and validation: ${messages.length} → ${validMessages.length} messages`,
      {
        originalRoles: messages.map((m) => m.role),
        convertedRoles: validMessages.map((m) => m.role),
      },
    );

    return validMessages;
  }

  private convertToGeminiMessages(messages: Message[]): Content[] {
    const geminiMessages: Content[] = [];

    for (const m of messages) {
      if (m.role === 'system') {
        continue;
      }

      if (m.role === 'user' && m.content) {
        geminiMessages.push({
          role: 'user',
          parts: [{ text: this.processMessageContent(m.content) }],
        });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          geminiMessages.push({
            role: 'model',
            parts: m.tool_calls.map((tc) => {
              const args =
                tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
              return {
                functionCall: {
                  name: tc.function.name,
                  args,
                },
              };
            }),
          });
        } else if (m.content) {
          geminiMessages.push({
            role: 'model',
            parts: [{ text: this.processMessageContent(m.content) }],
          });
        }
      } else if (m.role === 'tool') {
        logger.warn(
          'Unexpected tool message in convertToGeminiMessages - should have been converted to user',
        );
        continue;
      }
    }

    return geminiMessages;
  }

  private logToolResponseStats(messages: Message[]): void {
    const toolMessages = messages.filter((m) => m.role === 'tool');
    if (toolMessages.length === 0) return;

    const stats = {
      totalToolMessages: toolMessages.length,
      jsonResponses: 0,
      textResponses: 0,
      errorResponses: 0,
      emptyResponses: 0,
    };

    toolMessages.forEach((msg) => {
      if (!msg.content) {
        stats.emptyResponses++;
        return;
      }

      try {
        JSON.parse(this.processMessageContent(msg.content));
        stats.jsonResponses++;
      } catch {
        if (this.processMessageContent(msg.content).includes('error:') || this.processMessageContent(msg.content).includes('Error:')) {
          stats.errorResponses++;
        } else {
          stats.textResponses++;
        }
      }
    });

    logger.info('Tool response processing statistics', stats);
  }

  dispose(): void {
    // Gemini SDK doesn't require explicit cleanup
  }
}
