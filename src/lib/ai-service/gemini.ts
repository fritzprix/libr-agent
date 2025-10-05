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
import { tryParse, formatToolCall, generateToolCallId } from './utils';

const logger = getLogger('GeminiService');

/**
 * Defines the configuration specific to the Gemini service.
 * @internal
 */
interface GeminiServiceConfig {
  responseMimeType: string;
  tools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
  systemInstruction?: Array<{ text: string }>;
  maxOutputTokens?: number;
  temperature?: number;
}

/**
 * An AI service implementation for interacting with Google's Gemini models.
 */
export class GeminiService extends BaseAIService {
  private genAI: GoogleGenAI;

  /**
   * Initializes a new instance of the `GeminiService`.
   * @param apiKey The Google AI API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.genAI = new GoogleGenAI({
      apiKey: this.apiKey,
    });
  }

  /**
   * Generates a unique ID for a tool call.
   * @returns A unique tool call ID string.
   * @private
   */
  private generateToolCallId(): string {
    // keep for backward compatibility with subclasses expecting this method
    return generateToolCallId();
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Gemini`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Gemini;
  }

  /**
   * Initiates a streaming chat session with the Gemini API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @yields A JSON string for each chunk of the response, containing content and/or tool calls.
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
    const { config, tools } = this.prepareStreamChat(messages, options);

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
            (fc) => fc.name && typeof fc.name === 'string',
          );

          if (validFunctionCalls.length > 0) {
            yield JSON.stringify({
              tool_calls: validFunctionCalls.map((fc: FunctionCall) =>
                formatToolCall(
                  this.generateToolCallId(),
                  fc.name!,
                  fc.args ?? {},
                ),
              ),
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

  /**
   * Validates and sanitizes the message stack for Gemini.
   * Gemini requires that the conversation starts with a 'user' role. This function
   * converts any 'tool' messages to 'user' messages and ensures the stack
   * begins with the first 'user' message.
   * @param messages The array of messages to validate.
   * @returns A new array of validated and sanitized messages.
   * @private
   */
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

  /**
   * Converts an array of standard `Message` objects into the `Content` format
   * required by the Gemini API.
   * @param messages The array of messages to convert.
   * @returns An array of `Content` objects.
   * @private
   */
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
  /**
   * @inheritdoc
   * @description For Gemini, system instructions are handled as a separate parameter,
   * so this method returns null.
   * @protected
   */
  protected createSystemMessage(systemPrompt: string): unknown {
    // Gemini handles system instructions separately, not as messages
    void systemPrompt;
    return null;
  }

  /**
   * @inheritdoc
   * @description Converts a single `Message` into the format expected by the Gemini API.
   * @protected
   */
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

  /**
   * @inheritdoc
   * @description The Gemini SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    // Gemini SDK doesn't require explicit cleanup
  }
}
