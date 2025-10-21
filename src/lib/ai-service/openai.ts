import OpenAI from 'openai';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { llmConfigManager } from '../llm-config-manager';
const logger = getLogger('OpenAIService');

/**
 * An AI service implementation for OpenAI's language models.
 * This class also serves as a base for other OpenAI-compatible services like Fireworks.
 */
export class OpenAIService extends BaseAIService {
  protected openai: OpenAI;
  private modelCache?: import('../llm-config-manager').ModelInfo[];
  private cacheTimestamp?: number;
  private readonly CACHE_TTL = 3600000; // 1 hour in milliseconds

  /**
   * Initializes a new instance of the `OpenAIService`.
   * @param apiKey The OpenAI API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.OpenAI`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.OpenAI;
  }

  /**
   * Fetches the list of available models from the OpenAI service.
   * Maps provider-specific model metadata into the project's `ModelInfo` shape.
   * On error, returns an empty array and logs the failure.
   */
  async listModels(): Promise<import('../llm-config-manager').ModelInfo[]> {
    const logger = getLogger('OpenAIService.listModels');

    // Return cached models if still valid
    if (this.modelCache && this.isCacheValid()) {
      logger.debug('Returning cached models');
      return this.modelCache;
    }

    try {
      logger.info('Fetching models from OpenAI...');

      const response = await this.withRetry(async () => {
        // The OpenAI JS SDK exposes `models.list()` which returns a paginated result
        // with a `data` array of model metadata. Use that if available.
        // Use `any` locally to avoid depending on SDK-specific types here.
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const res: any = await (this.openai as any).models.list();
        return res;
      });

      // Normalize response shape — treat as unknown and narrow below
      const modelsRaw: Array<unknown> = Array.isArray(response?.data)
        ? (response.data as Array<unknown>)
        : [];

      const models = modelsRaw
        .map((entry) => {
          if (entry == null || typeof entry !== 'object') return null;
          const e = entry as Record<string, unknown>;

          const id =
            (typeof e.id === 'string' && e.id) ||
            (typeof e.model === 'string' && e.model) ||
            (typeof e.name === 'string' && e.name) ||
            String(e);

          // Merge with static config metadata
          const staticModel = llmConfigManager.getModel('openai', id);

          // Heuristic for context window (fallback if static config doesn't have it)
          let contextWindow = staticModel?.contextWindow ?? 4096;
          if (!staticModel?.contextWindow) {
            const lc = id.toLowerCase();
            if (lc.includes('gpt-4o') || lc.includes('gpt-4o-mini'))
              contextWindow = 65536;
            else if (lc.includes('gpt-4')) contextWindow = 32768;
            else if (lc.includes('gpt-3.5')) contextWindow = 4096;
          }

          const name = staticModel?.name || id;
          const supportStreaming = staticModel?.supportStreaming ?? true;
          const supportReasoning = staticModel?.supportReasoning ?? (id.toLowerCase().includes('gpt-4') || id.toLowerCase().includes('gpt-3.5'));
          const supportTools = staticModel?.supportTools ?? false;

          const description =
            staticModel?.description ||
            (typeof e.description === 'string' && e.description) ||
            (Array.isArray(e.permission)
              ? e.permission.join(',')
              : undefined) ||
            id;

          const modelInfo: import('../llm-config-manager').ModelInfo = {
            id,
            name,
            contextWindow,
            supportReasoning,
            supportTools,
            supportStreaming,
            cost: staticModel?.cost || { input: 0, output: 0 },
            description,
          };

          return modelInfo;
        })
        .filter(
          (v): v is import('../llm-config-manager').ModelInfo => v !== null,
        );

      // Cache the results
      this.modelCache = models;
      this.cacheTimestamp = Date.now();

      logger.info(`Loaded ${models.length} models from OpenAI API`);
      return models;
    } catch (error) {
      logger.warn(
        'Failed to fetch models from OpenAI API, falling back to static config',
        error,
      );
      return this.fallbackToStaticModels();
    }
  }

  /**
   * Initiates a streaming chat session with the OpenAI API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @yields A JSON string for each chunk of the response.
   */
  async *streamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
    } = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools, sanitizedMessages } = this.prepareStreamChat(
      messages,
      options,
    );

    const provider = this.getProvider();
    logger.debug('Preparing OpenAI streaming chat', {
      messages: sanitizedMessages,
    });

    try {
      // Use the sanitized messages prepared for the provider to ensure
      // provider-specific fixes (tool call conversions, thinking-field removals, etc.)
      const openaiMessages = this.convertToOpenAIMessages(
        sanitizedMessages,
        options.systemPrompt,
      );

      logger.debug('OpenAI request payload prepared', {
        messages: openaiMessages,
      });

      let modelName = options.modelName || config.defaultModel || 'gpt-4-turbo';

      // Handle Fireworks prefix
      const fireworksPrefix = 'accounts/fireworks/models/';
      if (
        provider === AIServiceProvider.Fireworks &&
        !modelName.startsWith(fireworksPrefix)
      ) {
        modelName = `${fireworksPrefix}${modelName}`;
      }

      logger.info(`${provider} call : `, {
        model: modelName,
        messages: openaiMessages,
      });

      const completion = await this.withRetry(() =>
        this.openai.chat.completions.create(
          {
            model: modelName,
            messages: openaiMessages,
            max_completion_tokens: config.maxTokens,
            stream: true,
            tools: tools as OpenAIChatCompletionTool[],
            tool_choice: !options.availableTools?.length
              ? undefined
              : options.forceToolUse
                ? 'required'
                : 'auto',
          },
          { signal: this.getAbortSignal() },
        ),
      );

      if (this.getAbortSignal().aborted) {
        this.logger.info('Stream aborted before iteration');
        return;
      }

      for await (const chunk of completion) {
        if (this.getAbortSignal().aborted) {
          this.logger.info('Stream aborted during iteration');
          break;
        }

        if (chunk.choices[0]?.delta?.tool_calls) {
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
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * Converts an array of standard `Message` objects into the format required by the OpenAI API.
   * UI-generated messages (source: 'ui') are treated as user messages to ensure
   * the AI model interprets UI interactions as user intent rather than system responses.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of `OpenAI.Chat.Completions.ChatCompletionMessageParam` objects.
   * @private
   */
  private convertToOpenAIMessages(
    messages: Message[],
    systemPrompt?: string,
  ): OpenAI.Chat.Completions.ChatCompletionMessageParam[] {
    const openaiMessages: OpenAI.Chat.Completions.ChatCompletionMessageParam[] =
      [];

    if (systemPrompt) {
      openaiMessages.push({ role: 'system', content: systemPrompt });
    }

    for (const m of messages) {
      // UI-generated messages are treated as user messages
      // This ensures that messages created by UI interactions (button clicks, tool executions, etc.)
      // are interpreted by the AI model as user intent
      const effectiveRole = m.source === 'ui' ? 'user' : m.role;

      if (effectiveRole === 'user') {
        openaiMessages.push({
          role: 'user',
          content: this.processMessageContent(m.content),
        });
      } else if (effectiveRole === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          openaiMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content) || null,
            tool_calls: m.tool_calls,
          });
        } else {
          openaiMessages.push({
            role: 'assistant',
            content: this.processMessageContent(m.content),
          });
        }
      } else if (effectiveRole === 'tool') {
        if (m.tool_call_id) {
          openaiMessages.push({
            role: 'tool',
            tool_call_id: m.tool_call_id,
            content: this.processMessageContent(m.content),
          });
        } else {
          logger.warn(
            `Tool message missing tool_call_id: ${JSON.stringify(m)}`,
          );
        }
      }
    }
    return openaiMessages;
  }

  /**
   * @inheritdoc
   * @description Creates an OpenAI-compatible system message object.
   * @protected
   */
  protected createSystemMessage(systemPrompt: string): unknown {
    return { role: 'system', content: systemPrompt };
  }

  /**
   * @inheritdoc
   * @description Converts a single `Message` into the format expected by the OpenAI API.
   * UI-generated messages (source: 'ui') are treated as user messages to ensure
   * the AI model interprets UI interactions as user intent.
   * @protected
   */
  protected convertSingleMessage(message: Message): unknown {
    // UI-generated messages are treated as user messages
    // This ensures that messages created by UI interactions (button clicks, tool executions, etc.)
    // are interpreted by the AI model as user intent
    const effectiveRole = message.source === 'ui' ? 'user' : message.role;

    if (effectiveRole === 'user') {
      return {
        role: 'user',
        content: this.processMessageContent(message.content),
      };
    } else if (effectiveRole === 'assistant') {
      if (message.tool_calls && message.tool_calls.length > 0) {
        return {
          role: 'assistant',
          content: this.processMessageContent(message.content) || null,
          tool_calls: message.tool_calls,
        };
      } else {
        return {
          role: 'assistant',
          content: this.processMessageContent(message.content),
        };
      }
    } else if (effectiveRole === 'tool') {
      if (message.tool_call_id) {
        return {
          role: 'tool',
          tool_call_id: message.tool_call_id,
          content: this.processMessageContent(message.content),
        };
      } else {
        logger.warn(
          `Tool message missing tool_call_id: ${JSON.stringify(message)}`,
        );
        return null;
      }
    } else if (effectiveRole === 'system') {
      return {
        role: 'system',
        content: this.processMessageContent(message.content),
      };
    }
    return null;
  }

  /**
   * @inheritdoc
   * @description The OpenAI SDK does not require explicit resource cleanup.
   */

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
  private fallbackToStaticModels(): Promise<import('../llm-config-manager').ModelInfo[]> {
    const logger = getLogger('OpenAIService.fallbackToStaticModels');
    logger.info('Using static config models');
    return super.listModels();
  }

  dispose(): void {
    // OpenAI SDK doesn't require explicit cleanup
  }
}
