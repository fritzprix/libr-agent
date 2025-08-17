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

  private generateDeterministicToolCallId(functionCall: FunctionCall): string {
    // 키 정렬된 안정적인 JSON 직렬화
    const stableStringify = (obj: unknown): string => {
      if (obj === null || typeof obj !== 'object') return JSON.stringify(obj);
      if (Array.isArray(obj)) return '[' + obj.map(stableStringify).join(',') + ']';
      const keys = Object.keys(obj as Record<string, unknown>).sort();
      return '{' + keys.map(k => `${JSON.stringify(k)}:${stableStringify((obj as Record<string, unknown>)[k])}`).join(',') + '}';
    };

    const argsStr = stableStringify(functionCall.args || {});
    const content = `${functionCall.name}:${argsStr}`;

    let hash = 0;
    for (let i = 0; i < content.length; i++) {
      const char = content.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash |= 0;
    }

    return `tool_${Math.abs(hash).toString(36)}`;
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

    // Log tool response statistics
    this.logToolResponseStats(messages);

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
            tool_calls: chunk.functionCalls.map((fc: FunctionCall) => {
              const toolCallId = this.generateDeterministicToolCallId(fc);

              logger.debug('Generated deterministic tool call ID', {
                functionName: fc.name,
                toolCallId,
                argsLength: JSON.stringify(fc.args || {}).length,
              });

              return {
                id: toolCallId,
                type: 'function',
                function: {
                  name: fc.name,
                  arguments: JSON.stringify(fc.args), // Convert args object to JSON string
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

    const logger = getLogger('GeminiValidation');
    
    // 1단계: tool → user 역할 변환 (Gemini 호환성을 위해)
    const convertedMessages = messages.map(m => {
      if (m.role === 'tool') {
        return { ...m, role: 'user' as const };
      }
      return m;
    });

    // 2단계: 첫 번째 user 메시지부터 시작하도록 조정
    const firstUserIndex = convertedMessages.findIndex(msg => msg.role === 'user');
    if (firstUserIndex === -1) {
      logger.warn('No user message found after role conversion');
      return [];
    }

    const validMessages = convertedMessages.slice(firstUserIndex);
    
    logger.info(`Role conversion and validation: ${messages.length} → ${validMessages.length} messages`, {
      originalRoles: messages.map(m => m.role),
      convertedRoles: validMessages.map(m => m.role)
    });

    return validMessages;
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
        // tool 메시지는 이미 validateGeminiMessageStack에서 user로 변환되었으므로
        // 이 분기는 실행되지 않아야 함
        const logger = getLogger('GeminiToolResponse');
        logger.warn('Unexpected tool message in convertToGeminiMessages - should have been converted to user');
        continue;
      }
    }

    return geminiMessages;
  }


  /**
   * Log tool response processing statistics
   */
  private logToolResponseStats(messages: Message[]): void {
    const logger = getLogger('GeminiToolResponseStats');
    
    const toolMessages = messages.filter(m => m.role === 'tool');
    if (toolMessages.length === 0) return;
    
    const stats = {
      totalToolMessages: toolMessages.length,
      jsonResponses: 0,
      textResponses: 0,
      errorResponses: 0,
      emptyResponses: 0
    };
    
    toolMessages.forEach(msg => {
      if (!msg.content) {
        stats.emptyResponses++;
        return;
      }
      
      try {
        JSON.parse(msg.content);
        stats.jsonResponses++;
      } catch {
        if (msg.content.includes('error:') || msg.content.includes('Error:')) {
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
