import { FunctionDeclaration, GoogleGenAI } from '@google/genai';
import { getLogger } from '../../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '@/lib/mcp';
import { AIServiceProvider, AIServiceConfig } from '../types';
import { BaseAIService } from '../base-service';
import type { ModelInfo } from '../../llm-config-manager';
import { GeminiServiceConfig } from './types';
import { convertToGeminiMessages, convertSingleMessage } from './mapper';
import {
  mapReasoningEffortToBudget,
  checkThinkingSupport,
  prepareSafetySettings,
} from './config';
import { fetchGeminiModels, getDefaultModel } from './models';
import { processGeminiStream } from './stream';

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
      // Use withRetry to wrap the fetch call
      const models = await this.withRetry(async () => {
        return fetchGeminiModels(this.genAI);
      });

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

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
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
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

    try {
      const geminiMessages = convertToGeminiMessages(messages);
      if (geminiMessages.length === 0) {
        throw new Error(
          'No valid messages to send to Gemini (must start with user/tool role)',
        );
      }
      const geminiTools = tools
        ? [
            {
              functionDeclarations: tools as FunctionDeclaration[],
            },
          ]
        : undefined;

      const model =
        options.modelName || config.defaultModel || getDefaultModel();

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
        const modelSupportsThinking = await checkThinkingSupport(
          model,
          this.modelCache,
        );
        if (modelSupportsThinking) {
          const thinkingBudget = mapReasoningEffortToBudget(
            config.reasoningEffort,
          );
          geminiConfig.thinkingConfig = {
            thinkingBudget,
            includeThoughts: true,
          };
        }
      }

      // Configure Gemini safety settings.
      geminiConfig.safetySettings = prepareSafetySettings(config);

      // 🔍 Detailed Logging before API Call
      const sysPromptText = geminiConfig.systemInstruction?.[0]?.text || '';
      this.logger.debug('🚀 Calling Gemini API - System Prompt Verification', {
        model,
        systemPromptLength: sysPromptText.length,
        includesSkills: sysPromptText.includes('<available_skills>'),
        first500Chars: sysPromptText.substring(0, 500),
        last500Chars: sysPromptText.substring(sysPromptText.length - 500),
      });

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

      // Use the stream processing logic from stream.ts
      yield* processGeminiStream(result, this.getAbortSignal(), this.logger);
    } catch (error) {
      if (
        error instanceof Error &&
        (error.message.includes('malformed_function_call') ||
          error.message.includes('MALFORMED_FUNCTION_CALL'))
      ) {
        this.logger.warn(
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
        messages,
        options,
        config,
      });
    }
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
    return convertSingleMessage(message);
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
   * @inheritdoc
   * @description The Gemini SDK does not require explicit resource cleanup.
   */
  dispose(): void {
    // Gemini SDK doesn't require explicit cleanup
  }
}
