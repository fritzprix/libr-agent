import { createId } from '@paralleldrive/cuid2';
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

const logger = getLogger('AIService');

export class GeminiService extends BaseAIService {
  private genAI: GoogleGenAI;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.genAI = new GoogleGenAI({
      apiKey: this.apiKey,
    });
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

    // Validate and fix message stack for Gemini's sequencing rules
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
      logger.info('gemini call : ', { model, config });

      // Fixed API call structure based on your example
      interface GeminiServiceConfig {
        responseMimeType: string;
        tools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
        systemInstruction?: Array<{ text: string }>;
        maxOutputTokens?: number;
        temperature?: number;
      }

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
            tool_calls: chunk.functionCalls.map((fc: FunctionCall) => ({
              id: createId(), // Generate a new ID for each tool call
              type: 'function',
              function: {
                name: fc.name,
                arguments: JSON.stringify(fc.args), // Convert args object to JSON string
              },
            })),
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

  /**
   * Simple validation for Gemini's message sequencing rules.
   * Removes orphaned function calls at the end of the message window.
   */
  private validateGeminiMessageStack(messages: Message[]): Message[] {
    if (messages.length === 0) {
      return messages;
    }

    // Check if the last message is an orphaned function call
    const lastMessage = messages[messages.length - 1];
    if (lastMessage.role === 'assistant' && lastMessage.tool_calls && lastMessage.tool_calls.length > 0) {
      // Check if this function call has corresponding tool responses
      let hasCorrespondingResponses = false;
      for (let i = messages.length - 1; i >= 0; i--) {
        const msg = messages[i];
        if (msg.role === 'tool' && lastMessage.tool_calls.some(tc => tc.id === msg.tool_call_id)) {
          hasCorrespondingResponses = true;
          break;
        }
        // Stop searching if we hit another assistant message
        if (msg.role === 'assistant' && msg !== lastMessage) {
          break;
        }
      }

      if (!hasCorrespondingResponses) {
        logger.warn('Removing orphaned function call at end of message window', {
          toolCallsCount: lastMessage.tool_calls.length,
          messageContent: lastMessage.content?.substring(0, 100)
        });
        return messages.slice(0, -1);
      }
    }

    return messages;
  }

  private convertToGeminiMessages(messages: Message[]): Content[] {
    const geminiMessages: Content[] = [];

    for (const m of messages) {
      if (m.role === 'system') {
        continue; // Skip system messages, handled by systemInstruction
      }

      if (m.role === 'user' && m.content) {
        geminiMessages.push({
          role: 'user',
          parts: [{ text: m.content }],
        });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          geminiMessages.push({
            role: 'model',
            parts: m.tool_calls.map((tc) => ({
              functionCall: {
                name: tc.function.name,
                args: JSON.parse(tc.function.arguments),
              },
            })),
          });
        } else if (m.content) {
          geminiMessages.push({
            role: 'model',
            parts: [{ text: m.content }],
          });
        }
      } else if (m.role === 'tool') {
        // Find the corresponding assistant message to get the function name
        let functionName: string | undefined;
        for (let j = messages.indexOf(m) - 1; j >= 0; j--) {
          const prevMessage = messages[j];
          if (prevMessage.role === 'assistant' && prevMessage.tool_calls) {
            const toolCall = prevMessage.tool_calls.find(
              (tc) => tc.id === m.tool_call_id,
            );
            if (toolCall) {
              functionName = toolCall.function.name;
              break;
            }
          }
        }

        if (functionName) {
          let response: Record<string, unknown> | undefined;
          try {
            response = JSON.parse(m.content);
          } catch {
            // If not valid JSON, wrap as object
            response = { value: m.content };
            logger.warn(
              `Tool message content is not valid JSON, wrapping as object: ${m.content}`,
            );
          }
          geminiMessages.push({
            role: 'function', // Gemini expects 'function' role for tool results
            parts: [
              {
                functionResponse: {
                  name: functionName,
                  response,
                },
              },
            ],
          });
        } else {
          logger.warn(
            `Could not find function name for tool message with tool_call_id: ${m.tool_call_id}`,
          );
          // Optionally, handle this error more robustly or skip the message
        }
      }
    }

    return geminiMessages;
  }

  dispose(): void {
    // Gemini SDK doesn't require explicit cleanup
  }
}
