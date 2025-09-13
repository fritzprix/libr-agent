import {
  FunctionDeclaration,
  GoogleGenAI,
  Content,
  FunctionCall,
} from '@google/genai';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
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
    const { config, tools } = this.prepareStreamChat(messages, options);

    this.logToolResponseStats(messages);

    const validatedMessages = this.validateGeminiMessageStack(messages);

    try {
      const geminiMessages = this.convertToGeminiMessages(validatedMessages);
      const geminiTools = tools
        ? [
            {
              functionDeclarations: tools as FunctionDeclaration[],
            },
          ]
        : undefined;

      const model =
        options.modelName || config.defaultModel || 'gemini-1.5-pro';

      const geminiConfig: GeminiServiceConfig = {
        responseMimeType: 'text/plain',
      };

      if (geminiTools) {
        geminiConfig.tools = geminiTools;
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
        logger.info('chunk : ', { chunk });
        if (chunk.functionCalls && chunk.functionCalls.length > 0) {
          const validFunctionCalls = chunk.functionCalls.filter(
            (fc) => fc.name && typeof fc.name === 'string' && fc.args,
          );

          if (validFunctionCalls.length > 0) {
            yield JSON.stringify({
              tool_calls: validFunctionCalls.map((fc: FunctionCall) => {
                let argumentsStr: string;
                try {
                  argumentsStr = JSON.stringify(fc.args || {});
                } catch (error) {
                  logger.warn('Failed to serialize function arguments', {
                    functionName: fc.name,
                    args: fc.args,
                    error,
                  });
                  argumentsStr = "'";
                }

                const toolCallId = this.generateToolCallId();
                logger.debug('Generated tool call ID', {
                  functionName: fc.name,
                  toolCallId,
                });

                return {
                  id: toolCallId,
                  type: 'function',
                  function: { name: fc.name, arguments: argumentsStr },
                };
              }),
            });
          }
        } else if (chunk.text) {
          yield JSON.stringify({ content: chunk.text });
        }
      }
    } catch (error) {
      if (
        error instanceof Error &&
        (error.message.includes('malformed_function_call') ||
          error.message.includes('MALFORMED_FUNCTION_CALL'))
      ) {
        logger.warn(
          'MALFORMED_FUNCTION_CALL detected. Retrying request without tools.',
          { originalError: error },
        );
        if (options.availableTools && options.availableTools.length > 0) {
          const retryOptions = { ...options, availableTools: [] };
          yield* this.streamChat(messages, retryOptions);
          return;
        }
      }

      this.handleStreamingError(error, {
        messages: validatedMessages,
        options,
        config,
      });
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
        if (
          this.processMessageContent(msg.content).includes('error:') ||
          this.processMessageContent(msg.content).includes('Error:')
        ) {
          stats.errorResponses++;
        } else {
          stats.textResponses++;
        }
      }
    });

    logger.info('Tool response processing statistics', stats);
  }

  // Implementation of abstract methods from BaseAIService
  protected createSystemMessage(systemPrompt: string): unknown {
    // Gemini handles system instructions separately, not as messages
    void systemPrompt;
    return null;
  }

  protected convertSingleMessage(message: Message): unknown {
    if (message.role === 'system') {
      // System messages are handled separately in the API call
      return null;
    }

    if (message.role === 'user' && message.content) {
      return {
        role: 'user',
        parts: [{ text: this.processMessageContent(message.content) }],
      };
    } else if (message.role === 'assistant') {
      if (message.tool_calls && message.tool_calls.length > 0) {
        return {
          role: 'model',
          parts: message.tool_calls.map((tc) => {
            const args =
              tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
            return {
              functionCall: {
                name: tc.function.name,
                args,
              },
            };
          }),
        };
      } else if (message.content) {
        return {
          role: 'model',
          parts: [{ text: this.processMessageContent(message.content) }],
        };
      }
    } else if (message.role === 'tool') {
      // Tool messages should be converted to user messages for Gemini
      logger.warn(
        'Tool message in convertSingleMessage - should have been converted to user',
      );
      return null;
    }
    return null;
  }

  dispose(): void {
    // Gemini SDK doesn't require explicit cleanup
  }
}
