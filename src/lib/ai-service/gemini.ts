import {
  FunctionDeclaration,
  GoogleGenAI,
  Content,
  FunctionCall,
  createPartFromFunctionResponse,
  HarmCategory,
  HarmBlockThreshold,
} from '@google/genai';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { tryParse, formatToolCall, generateToolCallId } from './utils';
import type { ModelInfo } from '../llm-config-manager';
import { llmConfigManager } from '../llm-config-manager';

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
  functionCallingConfig?: { mode: 'auto' | 'any' | 'none' };
  thinkingConfig?: {
    thinkingBudget?: number; // -1 (dynamic) | 0 (disabled) | positive number (token count)
    includeThoughts?: boolean; // Include thinking process in response
  };
  safetySettings?: Array<{
    category: HarmCategory;
    threshold: HarmBlockThreshold;
  }>;
}

/**
 * An AI service implementation for interacting with Google's Gemini models.
 */
export class GeminiService extends BaseAIService {
  private genAI: GoogleGenAI;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = 3600000; // 1 hour in milliseconds

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
   * Maps reasoning effort level to Gemini thinkingBudget tokens.
   * @param level The reasoning effort level.
   * @returns The thinking budget in tokens.
   * @private
   */
  private mapReasoningEffortToBudget(
    level?: 'low' | 'medium' | 'high',
  ): number {
    switch (level) {
      case 'low':
        return 1024; // Fast, minimal reasoning
      case 'medium':
        return 8192; // Balanced reasoning (default)
      case 'high':
        return 24576; // Deep reasoning (higher cost)
      default:
        return -1; // Dynamic adjustment by the model
    }
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Gemini`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Gemini;
  }

  /**
   * Fetches the list of available models from the Gemini API.
   * Uses pagination to retrieve all models and caches results for 1 hour.
   * Falls back to static config on API failure.
   */
  async listModels(): Promise<ModelInfo[]> {
    const logger = getLogger('GeminiService.listModels');

    // Return cached models if still valid
    if (this.modelCache && this.isCacheValid()) {
      logger.debug('Returning cached models');
      return this.modelCache;
    }

    try {
      logger.info('Fetching models from Gemini API...');

      // Call Gemini API models.list() with pagination
      const pager = await this.withRetry(async () => {
        return this.genAI.models.list({
          config: {
            pageSize: 100, // Fetch up to 100 models per page
          },
        });
      });

      const models: ModelInfo[] = [];
      let totalFetched = 0;
      let filteredOut = 0;

      // Iterate through all pages using AsyncIterable
      for await (const geminiModel of pager) {
        totalFetched++;

        // Extract model name (remove 'models/' prefix if present)
        const modelId = geminiModel.name?.replace(/^models\//, '') || '';

        if (!modelId) {
          logger.warn('Skipping model with empty ID', { geminiModel });
          filteredOut++;
          continue;
        }

        // Filter: Only include models that support generateContent
        const supportsGeneration =
          geminiModel.supportedActions?.includes('generateContent') ?? true;

        if (!supportsGeneration) {
          logger.debug('Skipping non-generation model', {
            modelId,
            supportedActions: geminiModel.supportedActions,
          });
          filteredOut++;
          continue;
        }

        logger.debug('Processing model from API', {
          modelId,
          displayName: geminiModel.displayName,
          inputTokenLimit: geminiModel.inputTokenLimit,
          supportedActions: geminiModel.supportedActions,
        });

        // Merge with static config metadata
        const staticModel = llmConfigManager.getModel('gemini', modelId);

        // Use API-provided context window with fallback
        const contextWindow =
          geminiModel.inputTokenLimit || staticModel?.contextWindow || 1048576; // Default to 1M tokens

        const modelInfo: ModelInfo = {
          id: modelId,
          name: geminiModel.displayName || staticModel?.name || modelId,
          contextWindow,
          // Detect thinking mode support from API response or model name
          supportReasoning:
            staticModel?.supportReasoning ??
            /gemini-2\.[5-9]|gemini-[3-9]/.test(modelId),
          supportTools: staticModel?.supportTools ?? true,
          supportStreaming: staticModel?.supportStreaming ?? true,
          cost: staticModel?.cost || { input: 0, output: 0 },
          description:
            geminiModel.description ||
            staticModel?.description ||
            `Gemini model: ${modelId}`,
        };

        models.push(modelInfo);
      }

      // Add static config models that aren't in the API response
      const staticModels = llmConfigManager.getModelsForProvider('gemini');
      if (staticModels) {
        const apiModelIds = new Set(models.map((m) => m.id));
        const staticModelIds = Object.keys(staticModels);

        for (const staticId of staticModelIds) {
          if (!apiModelIds.has(staticId)) {
            const staticModel = staticModels[staticId];
            logger.debug('Adding static-only model', {
              modelId: staticId,
              name: staticModel.name,
            });

            models.push({
              id: staticId,
              name: staticModel.name,
              contextWindow: staticModel.contextWindow,
              supportReasoning: staticModel.supportReasoning,
              supportTools: staticModel.supportTools,
              supportStreaming: staticModel.supportStreaming,
              cost: staticModel.cost,
              description: staticModel.description,
            });
          }
        }
      }

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

      logger.info(
        `Loaded ${models.length} total models (API: ${models.length - (staticModels ? Object.keys(staticModels).length - totalFetched + filteredOut : 0)}, static-only: ${staticModels ? Object.keys(staticModels).length - (models.length - filteredOut) : 0})`,
      );
      return models;
    } catch (error) {
      logger.warn(
        'Failed to fetch models from Gemini API, falling back to static config',
        error,
      );
      return this.fallbackToStaticModels();
    }
  }

  /**
   * Initiates a streaming chat session with the Gemini API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @yields A JSON string for each chunk of the response, containing content and/or tool calls.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
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
        options.modelName || config.defaultModel || this.getDefaultModel();

      const geminiConfig: GeminiServiceConfig = {
        responseMimeType: 'text/plain',
      };

      if (geminiTools) {
        geminiConfig.tools = geminiTools;
        if (options.forceToolUse) {
          geminiConfig.functionCallingConfig = { mode: 'any' };
        }
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

      // Add thinkingConfig for models that support thinking
      if (config.enableReasoning) {
        const modelSupportsThinking = await this.checkThinkingSupport(model);
        if (modelSupportsThinking) {
          const thinkingBudget = this.mapReasoningEffortToBudget(
            config.reasoningEffort,
          );
          geminiConfig.thinkingConfig = {
            thinkingBudget,
            includeThoughts: true,
          };
        }
      }

      // Relax safety settings to avoid false positives (empty responses)
      geminiConfig.safetySettings = [
        {
          category: HarmCategory.HARM_CATEGORY_HARASSMENT,
          threshold: HarmBlockThreshold.BLOCK_NONE,
        },
        {
          category: HarmCategory.HARM_CATEGORY_HATE_SPEECH,
          threshold: HarmBlockThreshold.BLOCK_NONE,
        },
        {
          category: HarmCategory.HARM_CATEGORY_SEXUALLY_EXPLICIT,
          threshold: HarmBlockThreshold.BLOCK_NONE,
        },
        {
          category: HarmCategory.HARM_CATEGORY_DANGEROUS_CONTENT,
          threshold: HarmBlockThreshold.BLOCK_NONE,
        },
      ];

      const result = await this.withRetry(async () => {
        return this.genAI.models.generateContentStream({
          model: model,
          config: geminiConfig,
          contents: geminiMessages,
        });
      });

      if (this.getAbortSignal().aborted) {
        this.logger.debug('Stream aborted before iteration');
        return;
      }

      for await (const chunk of result) {
        if (this.getAbortSignal().aborted) {
          this.logger.debug('Stream aborted during iteration');
          break;
        }

        // Type definition for Gemini Experimental Thoughts
        // See: https://github.com/google/generative-ai-js/issues/186
        interface GeminiThoughtChunk {
          candidates?: Array<{
            content?: {
              parts?: Array<
                | {
                    thought?: boolean; // Sometimes boolean flag?
                    text?: string;
                  }
                | {
                    // Another possible schema seen in discussions
                    thought?: string;
                  }
              >;
            };
          }>;
          // Possible schema for direct parts
          parts?: Array<{
            thought?: string;
          }>;
        }

        const thoughtChunk = chunk as unknown as GeminiThoughtChunk;
        let thoughtContent = '';

        // Attempt to find thoughts in candidates
        if (thoughtChunk.candidates?.[0]?.content?.parts) {
          for (const part of thoughtChunk.candidates[0].content.parts) {
            // Schema 1: part has 'thought' property with string
            if ('thought' in part && typeof part.thought === 'string') {
              thoughtContent += part.thought;
            }
            // Schema 2: part has 'thoughtSignature'
            if (
              'thoughtSignature' in part &&
              typeof part.thoughtSignature === 'string'
            ) {
              // Yield the signature as a separate event or combined with thinking?
              // Based on Message model, we have `thinkingSignature` field.
              yield JSON.stringify({
                thinkingSignature: part.thoughtSignature,
              });
            }
          }
        }

        // Attempt to find thoughts in top-level parts (sometimes seen in simplified chunks)
        if (thoughtChunk.parts) {
          for (const part of thoughtChunk.parts) {
            if (typeof part.thought === 'string') {
              thoughtContent += part.thought;
            }
          }
        }

        if (thoughtContent) {
          yield JSON.stringify({ thinking: thoughtContent });
        }

        if (chunk.functionCalls && chunk.functionCalls.length > 0) {
          const validFunctionCalls = chunk.functionCalls.filter(
            (fc) => fc.name && typeof fc.name === 'string',
          );

          if (validFunctionCalls.length > 0) {
            yield JSON.stringify({
              tool_calls: validFunctionCalls.map((fc: FunctionCall) => {
                const callId =
                  fc.id && typeof fc.id === 'string' && fc.id.length
                    ? fc.id
                    : this.generateToolCallId();
                return formatToolCall(callId, fc.name!, fc.args ?? {});
              }),
            });
          }
        } else if (chunk.text) {
          yield JSON.stringify({ content: chunk.text });
        } else {
          const candidates = chunk.candidates || [];
          const candidate = candidates[0];
          const finishReason = candidate ? candidate.finishReason : undefined;

          if (finishReason === 'UNEXPECTED_TOOL_CALL') {
            logger.warn(
              'Gemini stream ended with UNEXPECTED_TOOL_CALL. The model attempted to call a tool that was not properly defined or permitted in this context.',
              { chunk, finishReason },
            );
          } else if (finishReason === 'STOP') {
            logger.debug('Gemini stream stopped normally with empty chunk', {
              chunk,
            });
          } else {
            logger.warn('Gemini chunk has no text or functionCalls', {
              chunk,
              finishReason: candidate.finishReason, // Log finish reason (e.g., SAFETY)
              safetyRatings: candidate.safetyRatings, // Log safety ratings
            });
          }
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

      if (m.role === 'user') {
        // If this user message actually represents a tool response (the
        // code earlier converts tool -> user), prefer to convert any
        // structured tool_calls into FunctionResponse parts so Gemini
        // receives the structured data rather than raw text.
        if (m.tool_calls && m.tool_calls.length > 0) {
          const parts = m.tool_calls.map((tc) => {
            const parsed =
              tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
            const id =
              tc.id && typeof tc.id === 'string'
                ? tc.id
                : this.generateToolCallId();
            return createPartFromFunctionResponse(id, tc.function.name, parsed);
          });
          geminiMessages.push({ role: 'user', parts });
        } else if (m.content) {
          geminiMessages.push({
            role: 'user',
            parts: [{ text: this.processMessageContent(m.content) }],
          });
        }
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
        // If for some reason a tool role remains, handle similarly by
        // converting tool_calls to FunctionResponse parts.
        if (m.tool_calls && m.tool_calls.length > 0) {
          const parts = m.tool_calls.map((tc) => {
            const parsed =
              tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
            const id =
              tc.id && typeof tc.id === 'string'
                ? tc.id
                : this.generateToolCallId();
            return createPartFromFunctionResponse(id, tc.function.name, parsed);
          });
          geminiMessages.push({ role: 'user', parts });
        } else if (m.content) {
          geminiMessages.push({
            role: 'user',
            parts: [{ text: this.processMessageContent(m.content) }],
          });
        }
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
      // Convert tool message into a FunctionResponse part if possible.
      if (message.tool_calls && message.tool_calls.length > 0) {
        const parts = message.tool_calls.map((tc) => {
          const parsed =
            tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
          const id =
            tc.id && typeof tc.id === 'string'
              ? tc.id
              : this.generateToolCallId();
          return createPartFromFunctionResponse(id, tc.function.name, parsed);
        });
        return {
          role: 'user',
          parts,
        };
      }
      if (message.content) {
        return {
          role: 'user',
          parts: [{ text: this.processMessageContent(message.content) }],
        };
      }
      return null;
    }
    return null;
  }

  /**
   * Check if model cache is still valid (1 hour TTL)
   * @private
   */
  private isCacheValid(): boolean {
    if (!this.cacheTimestamp) return false;
    const age = Date.now() - this.cacheTimestamp;
    return age < this.CACHE_TTL;
  }

  /**
   * Fallback to static config models
   * @private
   */
  private fallbackToStaticModels(): Promise<ModelInfo[]> {
    const logger = getLogger('GeminiService.fallbackToStaticModels');
    logger.info('Using static config models');
    return super.listModels();
  }

  /**
   * Get the default model from static config
   * @private
   */
  private getDefaultModel(): string {
    // Try to get from static config first
    const staticModels = llmConfigManager.getModelsForProvider('gemini');
    if (staticModels) {
      const modelIds = Object.keys(staticModels);
      // Prefer Gemini 2.5 Flash as default (fast & capable)
      const preferred = modelIds.find((id) => id.includes('gemini-2.5-flash'));
      if (preferred) return preferred;

      // Fallback to first available model
      if (modelIds.length > 0) return modelIds[0];
    }

    // Ultimate fallback
    return 'gemini-1.5-pro';
  }

  /**
   * Check if a model supports thinking mode
   * @private
   */
  private async checkThinkingSupport(modelId: string): Promise<boolean> {
    // Check cache first
    if (this.modelCache) {
      const cachedModel = this.modelCache.find((m) => m.id === modelId);
      if (cachedModel) {
        return cachedModel.supportReasoning;
      }
    }

    // Check static config
    const staticModel = llmConfigManager.getModel('gemini', modelId);
    if (staticModel?.supportReasoning !== undefined) {
      return staticModel.supportReasoning;
    }

    // Fallback to pattern matching
    return /gemini-2\.[5-9]|gemini-[3-9]/.test(modelId);
  }

  /**
   * @inheritdoc
   * @description The Gemini SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    // Gemini SDK doesn't require explicit cleanup
  }
}
