import {
  FunctionDeclaration,
  FinishReason,
  FunctionCallingConfigMode,
  GoogleGenAI,
  Content,
  Type,
} from '@google/genai';
import { getLogger } from '../../logger';
import { Message } from '@/models/chat';
import { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import {
  AIServiceProvider,
  AIServiceConfig,
  type ContextInjectionResult,
} from '../types';
import { BaseAIService } from '../base-service';
import type { ModelInfo } from '../../llm-config-manager';
import { GeminiServiceConfig } from './types';
import {
  convertToGeminiMessages,
  convertMCPSchemaToGeminiParameters,
} from './mapper';
import {
  mapReasoningEffortToBudget,
  checkThinkingSupport,
  prepareSafetySettings,
} from './config';
import { fetchGeminiModels, getDefaultModel } from './models';
import { processGeminiStream } from './stream';
import {
  createEphemeralSessionContextInjection,
  formatSessionContextAsBackgroundReference,
  isCompactSummaryMessage,
} from '../base-service-context';
import type { MCPContent } from '@/lib/mcp';

function summarizeLibrAgentMessages(messages: Message[]): {
  count: number;
  roleCounts: Record<string, number>;
  compactSummaryCount: number;
  syntheticSessionContextCount: number;
  textChars: number;
  textBytes: number;
  idsPreview: string[];
} {
  const encoder = new TextEncoder();
  const roleCounts: Record<string, number> = {};
  let compactSummaryCount = 0;
  let syntheticSessionContextCount = 0;
  let textChars = 0;
  let textBytes = 0;

  for (const message of messages) {
    roleCounts[message.role] = (roleCounts[message.role] ?? 0) + 1;
    if (isCompactSummaryMessage(message)) {
      compactSummaryCount += 1;
    }
    if (message.id.startsWith('gemini-session-context-')) {
      syntheticSessionContextCount += 1;
    }
    const text = Array.isArray(message.content)
      ? message.content
          .filter(
            (part): part is MCPContent & { type: 'text'; text: string } => {
              return part.type === 'text' && typeof part.text === 'string';
            },
          )
          .map((part) => part.text)
          .join('\n')
      : '';
    textChars += text.length;
    textBytes += encoder.encode(text).length;
  }

  return {
    count: messages.length,
    roleCounts,
    compactSummaryCount,
    syntheticSessionContextCount,
    textChars,
    textBytes,
    idsPreview: messages.slice(0, 6).map((message) => message.id),
  };
}

function summarizeGeminiContents(contents: Content[]): {
  count: number;
  roleCounts: Record<string, number>;
  textPartCount: number;
  textChars: number;
  textBytes: number;
  inlineDataPartCount: number;
  functionCallPartCount: number;
  functionResponsePartCount: number;
} {
  const encoder = new TextEncoder();
  const roleCounts: Record<string, number> = {};
  let textPartCount = 0;
  let textChars = 0;
  let textBytes = 0;
  let inlineDataPartCount = 0;
  let functionCallPartCount = 0;
  let functionResponsePartCount = 0;

  for (const content of contents) {
    const role = content.role ?? 'unknown';
    roleCounts[role] = (roleCounts[role] ?? 0) + 1;
    for (const part of content.parts ?? []) {
      if ('text' in part && typeof part.text === 'string') {
        textPartCount += 1;
        textChars += part.text.length;
        textBytes += encoder.encode(part.text).length;
      }
      if ('inlineData' in part && part.inlineData) {
        inlineDataPartCount += 1;
      }
      if ('functionCall' in part && part.functionCall) {
        functionCallPartCount += 1;
      }
      if ('functionResponse' in part && part.functionResponse) {
        functionResponsePartCount += 1;
      }
    }
  }

  return {
    count: contents.length,
    roleCounts,
    textPartCount,
    textChars,
    textBytes,
    inlineDataPartCount,
    functionCallPartCount,
    functionResponsePartCount,
  };
}

function supportsGeminiToolsForModel(modelName: string): boolean {
  return /gemini-(1\.[5-9]|[2-9])/.test(modelName.toLowerCase());
}

/**
 * An AI service implementation for interacting with Google's Gemini models.
 */
export class GeminiService extends BaseAIService<Content, FunctionDeclaration> {
  private genAI: GoogleGenAI;
  private modelCache?: ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = 3600000;

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
   * @inheritdoc
   */
  convertTools(mcpTools: MCPTool[]): FunctionDeclaration[] {
    return mcpTools.map((mcpTool) => {
      // Convert the entire inputSchema to Gemini format
      const geminiParams = convertMCPSchemaToGeminiParameters(
        mcpTool.inputSchema,
      );

      return {
        name: mcpTool.name,
        description: mcpTool.description,
        parameters: {
          type: Type.OBJECT,
          description: mcpTool.inputSchema.description,
          properties: geminiParams.properties || {},
          required: mcpTool.inputSchema.required || [],
        },
      } satisfies FunctionDeclaration;
    });
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
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @yields A JSON string for each chunk of the response, containing content and/or tool calls.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      sessionContext?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
      disableToolUse?: boolean;
      signal?: AbortSignal;
    } = {},
  ): AsyncGenerator<string, void, void> {
    const abortSignal = options.signal;
    let currentOptions = options;
    let allowMalformedFunctionRetry = Boolean(options.availableTools?.length);

    while (true) {
      const { config, tools, sanitizedMessages } = this.prepareStreamChat(
        messages,
        currentOptions,
      );

      try {
        const normalizedContextInjection = this.prepareContextInjection(
          currentOptions.systemPrompt,
          currentOptions.sessionContext,
          sanitizedMessages,
        );
        const geminiMessages = this.convertMessages(
          normalizedContextInjection.messages,
          normalizedContextInjection.systemPrompt,
        );
        if (geminiMessages.length === 0) {
          throw new Error(
            'No valid messages to send to Gemini (must start with user/tool role)',
          );
        }
        const geminiTools =
          (tools?.length ?? 0) > 0
            ? [
                {
                  functionDeclarations: tools as FunctionDeclaration[],
                },
              ]
            : undefined;

        const model =
          currentOptions.modelName || config.defaultModel || getDefaultModel();
        const stablePrefix = normalizedContextInjection.systemPrompt ?? '';
        const encoder = new TextEncoder();
        const toolDeclarationCount =
          geminiTools?.[0]?.functionDeclarations.length ?? 0;

        const requestMessageSummary = summarizeLibrAgentMessages(
          normalizedContextInjection.messages,
        );
        const geminiContentSummary = summarizeGeminiContents(geminiMessages);

        this.logger.info('Gemini request assembly breakdown', {
          model,
          implicitCachingEligible: model.startsWith('gemini-2.5-'),
          disableToolUse: currentOptions.disableToolUse ?? false,
          forceToolUse: currentOptions.forceToolUse ?? false,
          librAgentMessages: requestMessageSummary,
          geminiContents: geminiContentSummary,
          stablePrefixLength: stablePrefix.length,
          stablePrefixBytes: encoder.encode(stablePrefix).length,
          toolDeclarationCount,
        });

        let thinkingConfig: GeminiServiceConfig['thinkingConfig'];
        if (config.enableReasoning) {
          const modelSupportsThinking = await checkThinkingSupport(
            model,
            this.modelCache,
          );
          if (modelSupportsThinking) {
            const thinkingBudget = mapReasoningEffortToBudget(
              config.reasoningEffort,
            );
            thinkingConfig = {
              thinkingBudget,
              includeThoughts: true,
            };
          }
        }

        const safetySettings = prepareSafetySettings(config);
        const geminiConfig: GeminiServiceConfig = {
          responseMimeType: 'text/plain',
          abortSignal,
        };

        if (geminiTools) {
          geminiConfig.tools = geminiTools;
        }
        if (stablePrefix) {
          geminiConfig.systemInstruction = [{ text: stablePrefix }];
        }

        if (geminiTools) {
          if (currentOptions.disableToolUse) {
            geminiConfig.toolConfig = {
              functionCallingConfig: {
                mode: FunctionCallingConfigMode.NONE,
              },
            };
          } else if (currentOptions.forceToolUse) {
            geminiConfig.toolConfig = {
              functionCallingConfig: {
                mode: FunctionCallingConfigMode.ANY,
              },
            };
          }
        }

        if (config.maxTokens) {
          geminiConfig.maxOutputTokens = config.maxTokens;
        }

        if (config.temperature !== undefined) {
          geminiConfig.temperature = config.temperature;
        }

        if (thinkingConfig) {
          geminiConfig.thinkingConfig = thinkingConfig;
        }

        geminiConfig.safetySettings = safetySettings;

        // 🔍 Detailed Logging before API Call
        const sysPromptText = geminiConfig.systemInstruction?.[0]?.text || '';
        this.logger.debug(
          '🚀 Calling Gemini API - System Prompt Verification',
          {
            model,
            systemPromptLength: sysPromptText.length,
            includesSkills: sysPromptText.includes('<available_skills>'),
            first500Chars: sysPromptText.substring(0, 500),
            last500Chars: sysPromptText.substring(sysPromptText.length - 500),
          },
        );

        const result = await this.withRetry(async () => {
          return this.genAI.models.generateContentStream({
            model: model,
            config: geminiConfig,
            contents: geminiMessages,
          });
        }, abortSignal);

        if (abortSignal?.aborted) {
          this.logger.debug('Stream aborted before iteration');
          return;
        }

        yield* processGeminiStream(result, abortSignal, this.logger);
        return;
      } catch (error) {
        const malformedFunctionCall =
          error instanceof Error &&
          (error.message.includes('malformed_function_call') ||
            error.message.includes('MALFORMED_FUNCTION_CALL'));

        if (malformedFunctionCall && allowMalformedFunctionRetry) {
          this.logger.warn(
            'MALFORMED_FUNCTION_CALL detected. Retrying request without tools.',
            { originalError: error },
          );
          allowMalformedFunctionRetry = false;
          currentOptions = {
            ...currentOptions,
            availableTools: [],
            forceToolUse: false,
          };
          continue;
        }

        this.handleStreamingError(error, {
          messages,
          options: currentOptions,
          config,
        });
      }
    }
  }

  override prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): ContextInjectionResult {
    if (!sessionContext) {
      return { systemPrompt, sessionContext: undefined, messages };
    }

    this.logger.debug(
      'Injecting Gemini session context as ephemeral tail message',
      {
        sessionContextLength: sessionContext.length,
      },
    );

    return createEphemeralSessionContextInjection(
      systemPrompt,
      sessionContext,
      messages,
      {
        idPrefix: 'gemini-session-context',
        contentText: formatSessionContextAsBackgroundReference(sessionContext),
      },
    );
  }

  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): Content[] {
    // Note: Gemini system prompt is handled via systemInstruction in config
    void systemPrompt;
    return convertToGeminiMessages(messages);
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Gemini handles system messages separately
    if (message.role === 'system') return null;

    // Remove thinkingSignature if it's the dummy signature we injected
    // (though usually we don't need to do this for the upstream API)
    if (message.thinkingSignature === 'skip_thought_signature_validator') {
      delete message.thinkingSignature;
    }

    return message;
  }

  /**
   * @inheritdoc
   */
  static supportsToolsForModel(modelName: string): boolean {
    return supportsGeminiToolsForModel(modelName);
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('gemini-1.5-pro')) return 2000000;
    if (lowerName.includes('gemini-1.5-flash')) return 1000000;
    if (lowerName.includes('gemini-2.0-flash')) return 1000000;
    return 1000000;
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    return GeminiService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return GeminiService.estimateContextWindowForModel(modelName);
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
   * Performs a non-streaming text generation request using the Gemini API.
   * @param prompt The prompt to send to the model.
   * @param options Optional parameters for the sampling request.
   * @param options.modelName The name of the model.
   * @param options.samplingOptions The options used for text generation sampling.
   * @param options.config Optional configuration for the service.
   * @returns A promise that resolves to a `SamplingResponse`.
   */
  async sampleText(
    prompt: string,
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
      signal?: AbortSignal;
    },
  ): Promise<SamplingResponse> {
    const rawConfig = this.mergeConfig(options) as AIServiceConfig &
      GeminiServiceConfig;
    const model =
      options?.modelName || rawConfig.defaultModel || getDefaultModel();
    const s = options?.samplingOptions;
    const abortSignal = options?.signal;

    const response = await this.withRetry(
      () =>
        this.genAI.models.generateContent({
          model,
          config: {
            abortSignal,
            maxOutputTokens: s?.maxTokens ?? rawConfig.maxTokens,
            temperature: s?.temperature ?? rawConfig.temperature,
            topP: s?.topP,
            topK: s?.topK,
            stopSequences: s?.stopSequences,
          },
          contents: [{ role: 'user', parts: [{ text: prompt }] }],
        }),
      abortSignal,
    );

    const candidate = response.candidates?.[0];
    const text = candidate?.content?.parts?.[0]?.text ?? '';
    const finishReason = candidate?.finishReason ?? FinishReason.STOP;

    return {
      jsonrpc: '2.0',
      id: null,
      result: {
        content: [{ type: 'text', text }],
        sampling: {
          finishReason: finishReason === FinishReason.STOP ? 'stop' : 'length',
          usage: response.usageMetadata
            ? {
                promptTokens: response.usageMetadata.promptTokenCount ?? 0,
                completionTokens:
                  response.usageMetadata.candidatesTokenCount ?? 0,
                totalTokens: response.usageMetadata.totalTokenCount ?? 0,
                cachedPromptTokens:
                  response.usageMetadata.cachedContentTokenCount,
              }
            : undefined,
          model,
        },
      },
    };
  }

  /**
   * @inheritdoc
   */
  dispose(): void {
    this.logger.debug('Gemini service disposed');
  }
}
