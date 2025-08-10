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
   * Validates Gemini's strict message sequencing rules.
   * Function calls (assistant with tool_calls) must come immediately after a user turn or function response turn.
   * Removes any assistant messages with tool_calls that violate this rule.
   */
  private validateGeminiMessageStack(messages: Message[]): Message[] {
    if (messages.length === 0) {
      return messages;
    }

    const validatedMessages: Message[] = [];
    let removedCount = 0;

    for (let i = 0; i < messages.length; i++) {
      const currentMessage = messages[i];

      // Check if current message is an assistant message with tool_calls
      if (
        currentMessage.role === 'assistant' &&
        currentMessage.tool_calls &&
        currentMessage.tool_calls.length > 0
      ) {
        // Function call must come immediately after user or tool message
        const previousMessage = validatedMessages[validatedMessages.length - 1];

        if (
          !previousMessage ||
          (previousMessage.role !== 'user' && previousMessage.role !== 'tool')
        ) {
          logger.warn(
            'Removing assistant function call that violates Gemini sequencing rules',
            {
              index: i,
              toolCallsCount: currentMessage.tool_calls.length,
              previousRole: previousMessage?.role || 'none',
              messageContent: currentMessage.content?.substring(0, 100),
            },
          );
          removedCount++;
          continue; // Skip this message
        }
      }

      // Add valid message to the result
      validatedMessages.push(currentMessage);
    }

    if (removedCount > 0) {
      logger.info(
        `Removed ${removedCount} assistant messages that violated Gemini sequencing rules`,
        {
          originalCount: messages.length,
          validatedCount: validatedMessages.length,
        },
      );
    }

    return validatedMessages;
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
