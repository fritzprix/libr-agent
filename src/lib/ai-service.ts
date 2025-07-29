import { createId } from '@paralleldrive/cuid2';
import Groq from 'groq-sdk';
import OpenAI from 'openai';
import Anthropic from '@anthropic-ai/sdk';
import {
  FunctionDeclaration,
  GoogleGenAI,
  Content,
  Type,
  FunctionCall,
} from '@google/genai';
import { ChatCompletionTool as GroqChatCompletionTool } from 'groq-sdk/resources/chat/completions.mjs';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import {
  MessageParam as AnthropicMessageParam,
  Tool as AnthropicTool,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import { MCPTool, JSONSchema } from './tauri-mcp-client';
import { getLogger } from './logger';
import { Message } from '@/models/chat';

const logger = getLogger('AIService');

// --- Configuration and Types ---

export interface AIServiceConfig {
  timeout?: number;
  maxRetries?: number;
  retryDelay?: number;
  defaultModel?: string;
  maxTokens?: number;
  temperature?: number;
}

export enum AIServiceProvider {
  Groq = 'groq',
  OpenAI = 'openai',
  Anthropic = 'anthropic',
  Gemini = 'gemini',
  Empty = 'empty',
}

export class AIServiceError extends Error {
  constructor(
    message: string,
    public provider: AIServiceProvider,
    public statusCode?: number,
    public originalError?: Error,
  ) {
    super(message);
    this.name = 'AIServiceError';
  }
}

// --- Validation and Security ---

class MessageValidator {
  static validateMessage(message: Message): void {
    if (!message.id || typeof message.id !== 'string') {
      throw new Error('Message must have a valid id');
    }
    if (
      (!message.content &&
        (message.role === 'user' || message.role === 'system')) ||
      typeof message.content !== 'string'
    ) {
      logger.error(`Invalid message content: `, { message });
      throw new Error('Message must have valid content');
    }
    if (!['user', 'assistant', 'system', 'tool'].includes(message.role)) {
      throw new Error('Message must have a valid role');
    }

    // Sanitize content length
    if (message.content.length > 100000) {
      throw new Error('Message content too long');
    }
  }

  static validateTool(tool: MCPTool): void {
    if (!tool.name || typeof tool.name !== 'string') {
      throw new Error('Tool must have a valid name');
    }
    if (!tool.description || typeof tool.description !== 'string') {
      throw new Error('Tool must have a valid description');
    }
    if (!tool.inputSchema || typeof tool.inputSchema !== 'object') {
      throw new Error('Tool must have a valid inputSchema');
    }
    if (tool.inputSchema.type !== 'object') {
      throw new Error('Tool inputSchema must be of type "object"');
    }
  }

  static sanitizeToolArguments(args: string): Record<string, unknown> {
    try {
      const parsed = JSON.parse(args);
      if (typeof parsed !== 'object' || parsed === null) {
        throw new Error('Tool arguments must be an object');
      }
      return parsed;
    } catch (error: unknown) {
      throw new Error(
        `Invalid tool arguments: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }
}

// --- Tool Conversion with Enhanced Type Safety ---

type ProviderToolType =
  | GroqChatCompletionTool
  | OpenAIChatCompletionTool
  | AnthropicTool
  | FunctionDeclaration;
type ProviderToolsType = ProviderToolType[];

// Helper function to convert JSON schema types to Gemini types
interface JsonSchemaProperty {
  type: string;
  description?: string;
  items?: JsonSchemaProperty;
}

function convertJSONSchemaToJsonSchemaProperty(
  schema: JSONSchema,
): JsonSchemaProperty {
  // Extract the base type from our structured JSONSchema
  const getTypeString = (schema: JSONSchema): string => {
    if (schema.type === 'string') return 'string';
    if (schema.type === 'number') return 'number';
    if (schema.type === 'integer') return 'integer';
    if (schema.type === 'boolean') return 'boolean';
    if (schema.type === 'array') return 'array';
    if (schema.type === 'object') return 'object';
    if (schema.type === 'null') return 'null';
    return 'string'; // fallback
  };

  const result: JsonSchemaProperty = {
    type: getTypeString(schema),
    description: schema.description,
  };

  // Handle array items recursively
  if (schema.type === 'array' && 'items' in schema && schema.items) {
    if (Array.isArray(schema.items)) {
      // If items is an array, use the first item
      result.items =
        schema.items.length > 0
          ? convertJSONSchemaToJsonSchemaProperty(schema.items[0])
          : { type: 'string' };
    } else {
      result.items = convertJSONSchemaToJsonSchemaProperty(schema.items);
    }
  }

  return result;
}

// Helper function to convert JSON schema types to Gemini types
function convertPropertiesToGeminiTypes(
  properties: Record<string, JsonSchemaProperty>,
): Record<
  string,
  { type: Type; description?: string; items?: { type: Type } }
> {
  if (!properties || typeof properties !== 'object') {
    return {};
  }

  const convertedProperties: Record<
    string,
    { type: Type; description?: string; items?: { type: Type } }
  > = {};

  for (const [key, value] of Object.entries(properties)) {
    const propType = value.type;

    switch (propType) {
      case 'string':
        convertedProperties[key] = { type: Type.STRING };
        break;
      case 'number':
      case 'integer':
        convertedProperties[key] = { type: Type.NUMBER };
        break;
      case 'boolean':
        convertedProperties[key] = { type: Type.BOOLEAN };
        break;
      case 'array':
        convertedProperties[key] = {
          type: Type.ARRAY,
          items: value.items
            ? convertSinglePropertyToGeminiType(value.items)
            : { type: Type.STRING },
        };
        break;
      case 'object':
        convertedProperties[key] = { type: Type.OBJECT };
        break;
      default:
        convertedProperties[key] = { type: Type.STRING };
        break;
    }

    if (value.description && typeof value.description === 'string') {
      convertedProperties[key].description = value.description;
    }
  }

  return convertedProperties;
}

// Helper function to convert a single property type
function convertSinglePropertyToGeminiType(prop: JsonSchemaProperty): {
  type: Type;
  items?: { type: Type };
} {
  switch (prop.type) {
    case 'string':
      return { type: Type.STRING };
    case 'number':
    case 'integer':
      return { type: Type.NUMBER };
    case 'boolean':
      return { type: Type.BOOLEAN };
    case 'array':
      return { type: Type.ARRAY };
    case 'object':
      return { type: Type.OBJECT };
    default:
      return { type: Type.STRING };
  }
}

// Updated tool conversion for Gemini - use parameters with Type enums
function convertMCPToolToProviderFormat(
  mcpTool: MCPTool,
  provider: AIServiceProvider,
): ProviderToolType {
  MessageValidator.validateTool(mcpTool);

  // Extract properties and required fields from the structured schema
  const properties = mcpTool.inputSchema.properties || {};
  const required = mcpTool.inputSchema.required || [];

  const commonParameters = {
    type: 'object' as const,
    properties: properties,
    required: required,
  };

  switch (provider) {
    case AIServiceProvider.OpenAI:
      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters: commonParameters,
        },
      } satisfies OpenAIChatCompletionTool;
    case AIServiceProvider.Groq:
      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters: commonParameters,
        },
      };
    case AIServiceProvider.Anthropic:
      return {
        name: mcpTool.name,
        description: mcpTool.description,
        input_schema: commonParameters,
      };
    case AIServiceProvider.Gemini:
      // Use parameters with Type enums for Google GenAI SDK
      return {
        name: mcpTool.name,
        description: mcpTool.description,
        parameters: {
          type: Type.OBJECT,
          properties: convertPropertiesToGeminiTypes(
            Object.fromEntries(
              Object.entries(mcpTool.inputSchema.properties || {}).map(
                ([key, value]) => [
                  key,
                  convertJSONSchemaToJsonSchemaProperty(value),
                ],
              ),
            ),
          ),
          required: mcpTool.inputSchema.required || [],
        },
      };
    case AIServiceProvider.Empty:
      throw new AIServiceError(
        `Tool conversion not supported for Empty AIServiceProvider`,
        AIServiceProvider.Empty,
      );
  }
}

export function convertMCPToolsToProviderTools(
  mcpTools: MCPTool[],
  provider: AIServiceProvider,
): ProviderToolsType {
  if (provider === AIServiceProvider.Gemini) {
    return mcpTools.map(
      (tool) =>
        convertMCPToolToProviderFormat(tool, provider) as FunctionDeclaration,
    );
  }
  return mcpTools.map((tool) => convertMCPToolToProviderFormat(tool, provider));
}

// --- Enhanced Service Interface ---

export interface IAIService {
  streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;

  dispose(): void;
}

// --- Base Service Class with Common Functionality ---

abstract class BaseAIService implements IAIService {
  protected defaultConfig: AIServiceConfig = {
    timeout: 30000,
    maxRetries: 3,
    retryDelay: 1000,
    maxTokens: 4096,
    temperature: 0.7,
  };

  constructor(
    protected apiKey: string,
    protected config?: AIServiceConfig,
  ) {
    this.validateApiKey(apiKey);
    this.defaultConfig = { ...this.defaultConfig, ...config };
  }

  protected validateApiKey(apiKey: string): void {
    if (!apiKey || typeof apiKey !== 'string' || apiKey.trim().length === 0) {
      throw new AIServiceError('Invalid API key provided', this.getProvider());
    }
  }

  protected validateMessages(messages: Message[]): void {
    if (!Array.isArray(messages) || messages.length === 0) {
      throw new AIServiceError(
        'Messages array cannot be empty',
        this.getProvider(),
      );
    }
    messages.forEach(MessageValidator.validateMessage);
  }

  protected async withRetry<T>(
    operation: () => Promise<T>,
    maxRetries: number = this.defaultConfig.maxRetries!,
  ): Promise<T> {
    let lastError: Error;

    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        return await this.withTimeout(operation(), this.defaultConfig.timeout!);
      } catch (error) {
        lastError = error as Error;

        if (attempt === maxRetries) {
          throw new AIServiceError(
            `Operation failed after ${maxRetries + 1} attempts: ${
              lastError.message
            }`,
            this.getProvider(),
            undefined,
            lastError,
          );
        }

        // Exponential backoff
        const delay = this.defaultConfig.retryDelay! * Math.pow(2, attempt);
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }

    throw lastError!;
  }

  protected async withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
  ): Promise<T> {
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error('Operation timed out')), timeoutMs);
    });

    return Promise.race([promise, timeoutPromise]);
  }

  abstract streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;

  abstract getProvider(): AIServiceProvider;
  abstract dispose(): void;
}

export class EmptyAIService extends BaseAIService {
  constructor() {
    super('empty_api_key'); // Dummy API key
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Empty;
  }

  async *streamChat(): AsyncGenerator<string, void, void> {
    yield '';
    throw new AIServiceError(
      `EmptyAIService does not support streaming chat`,
      AIServiceProvider.Empty,
    );
    // Yield nothing, this is an empty service
  }

  dispose(): void {
    // No-op
  }
}

// --- Enhanced Service Implementations ---

export class GroqService extends BaseAIService {
  private groq: Groq;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.groq = new Groq({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Groq;
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
    logger.info('tools : ', { availableTools: options.availableTools });

    const config = { ...this.defaultConfig, ...options.config };

    try {
      const groqMessages = this.convertToGroqMessages(
        messages,
        options.systemPrompt,
      );

      const chatCompletion = await this.withRetry(() =>
        this.groq.chat.completions.create({
          messages: groqMessages,
          model:
            options.modelName || config.defaultModel || 'llama-3.1-8b-instant',
          temperature: config.temperature,
          max_tokens: config.maxTokens,
          reasoning_format: 'parsed',
          stream: true,
          tools: options.availableTools
            ? (convertMCPToolsToProviderTools(
                options.availableTools,
                AIServiceProvider.Groq,
              ) as GroqChatCompletionTool[])
            : undefined,
          tool_choice: options.availableTools ? 'auto' : undefined,
        }),
      );

      for await (const chunk of chatCompletion) {
        logger.info('inside chunk : ', { chunk: JSON.stringify(chunk) });
        if (chunk.choices[0]?.delta?.reasoning) {
          yield JSON.stringify({ thinking: chunk.choices[0].delta.reasoning });
        } else if (chunk.choices[0]?.delta?.tool_calls) {
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
      throw new AIServiceError(
        `Groq streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        AIServiceProvider.Groq,
        undefined,
        error instanceof Error ? error : undefined,
      );
    }
  }

  private convertToGroqMessages(
    messages: Message[],
    systemPrompt?: string,
  ): Groq.Chat.Completions.ChatCompletionMessageParam[] {
    const groqMessages: Groq.Chat.Completions.ChatCompletionMessageParam[] = [];

    if (systemPrompt) {
      groqMessages.push({ role: 'system', content: systemPrompt });
    }

    for (const m of messages) {
      if (m.role === 'user') {
        groqMessages.push({ role: 'user', content: m.content });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          groqMessages.push({
            role: 'assistant',
            content: m.content || null,
            tool_calls: m.tool_calls,
          });
        } else if (m.thinking) {
          groqMessages.push({
            role: 'assistant',
            content: m.content,
          });
        } else {
          groqMessages.push({ role: 'assistant', content: m.content });
        }
      } else if (m.role === 'tool') {
        if (m.tool_call_id) {
          groqMessages.push({
            role: 'tool',
            tool_call_id: m.tool_call_id,
            content: m.content,
          });
        } else {
          logger.warn(
            `Tool message missing tool_call_id: ${JSON.stringify(m)}`,
          );
        }
      }
    }
    return groqMessages;
  }

  dispose(): void {
    // Groq SDK doesn't require explicit cleanup
  }
}

export class OpenAIService extends BaseAIService {
  private openai: OpenAI;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      dangerouslyAllowBrowser: true,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.OpenAI;
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

    const config = { ...this.defaultConfig, ...options.config };
    if (options.availableTools) {
      logger.info(' tool calls: ', {
        tools: convertMCPToolsToProviderTools(
          options?.availableTools,
          AIServiceProvider.OpenAI,
        ),
      });
    }

    try {
      const openaiMessages = this.convertToOpenAIMessages(
        messages,
        options.systemPrompt,
      );
      logger.info('openai call : ', { openaiMessages });

      const completion = await this.withRetry(() =>
        this.openai.chat.completions.create({
          model: options.modelName || config.defaultModel || 'gpt-4-turbo',
          messages: openaiMessages,
          max_completion_tokens: config.maxTokens,
          stream: true,
          tools: options.availableTools
            ? (convertMCPToolsToProviderTools(
                options.availableTools,
                AIServiceProvider.OpenAI,
              ) as OpenAIChatCompletionTool[])
            : undefined,
          tool_choice: options.availableTools ? 'auto' : undefined,
        }),
      );

      for await (const chunk of completion) {
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
      throw new AIServiceError(
        `OpenAI streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        AIServiceProvider.OpenAI,
        undefined,
        error instanceof Error ? error : undefined,
      );
    }
  }

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
      if (m.role === 'user') {
        openaiMessages.push({ role: 'user', content: m.content });
      } else if (m.role === 'assistant') {
        if (m.tool_calls && m.tool_calls.length > 0) {
          openaiMessages.push({
            role: 'assistant',
            content: m.content || null,
            tool_calls: m.tool_calls,
          });
        } else {
          openaiMessages.push({ role: 'assistant', content: m.content });
        }
      } else if (m.role === 'tool') {
        if (m.tool_call_id) {
          openaiMessages.push({
            role: 'tool',
            tool_call_id: m.tool_call_id,
            content: m.content,
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

  dispose(): void {
    // OpenAI SDK doesn't require explicit cleanup
  }
}

export class AnthropicService extends BaseAIService {
  private anthropic: Anthropic;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.anthropic = new Anthropic({ apiKey: this.apiKey });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Anthropic;
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

    const config = { ...this.defaultConfig, ...options.config };

    try {
      const anthropicMessages = this.convertToAnthropicMessages(messages);

      const completion = await this.withRetry(() =>
        this.anthropic.messages.create({
          model:
            options.modelName ||
            config.defaultModel ||
            'claude-3-sonnet-20240229',
          max_tokens: config.maxTokens!,
          messages: anthropicMessages,
          stream: true,
          thinking: {
            budget_tokens: 1024,
            type: 'enabled',
          },
          system: options.systemPrompt,
          tools: options.availableTools
            ? (convertMCPToolsToProviderTools(
                options.availableTools,
                AIServiceProvider.Anthropic,
              ) as AnthropicTool[])
            : undefined,
        }),
      );

      for await (const chunk of completion) {
        if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'text_delta'
        ) {
          yield JSON.stringify({ content: chunk.delta.text });
        } else if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'thinking_delta'
        ) {
          yield JSON.stringify({ thinking: chunk.delta.thinking });
        } else if (
          chunk.type === 'content_block_delta' &&
          chunk.delta.type === 'input_json_delta'
        ) {
          yield JSON.stringify({ tool_calls: chunk.delta.partial_json });
        }
      }
    } catch (error) {
      throw new AIServiceError(
        `Anthropic streaming failed: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`,
        AIServiceProvider.Anthropic,
        undefined,
        error instanceof Error ? error : undefined,
      );
    }
  }

  private convertToAnthropicMessages(
    messages: Message[],
  ): AnthropicMessageParam[] {
    const anthropicMessages: AnthropicMessageParam[] = [];

    for (const m of messages) {
      if (m.role === 'system') {
        // System messages are handled separately in the API call
        continue;
      }

      if (m.role === 'user') {
        anthropicMessages.push({ role: 'user', content: m.content });
      } else if (m.role === 'assistant') {
        if (m.tool_calls) {
          anthropicMessages.push({
            role: 'assistant',
            content: m.tool_calls.map((tc) => ({
              type: 'tool_use' as const,
              id: tc.id,
              name: tc.function.name,
              input: MessageValidator.sanitizeToolArguments(
                tc.function.arguments,
              ),
            })),
          });
        } else if (m.tool_use) {
          anthropicMessages.push({
            role: 'assistant',
            content: [
              {
                type: 'tool_use' as const,
                id: m.tool_use.id,
                name: m.tool_use.name,
                input: m.tool_use.input,
              },
            ],
          });
        } else {
          anthropicMessages.push({ role: 'assistant', content: m.content });
        }
      } else if (m.role === 'tool') {
        anthropicMessages.push({
          role: 'user',
          content: [
            {
              type: 'tool_result' as const,
              tool_use_id: m.tool_call_id!,
              content: m.content,
            },
          ],
        });
      } else {
        logger.warn(`Unsupported message role for Anthropic: ${m.role}`);
      }
    }
    return anthropicMessages;
  }

  dispose(): void {
    // Anthropic SDK doesn't require explicit cleanup
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

    const config = { ...this.defaultConfig, ...options.config };

    try {
      const geminiMessages = this.convertToGeminiMessages(messages);
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
          messagesCount: messages.length,
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
                args: MessageValidator.sanitizeToolArguments(
                  tc.function.arguments,
                ),
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
          geminiMessages.push({
            role: 'function', // Gemini expects 'function' role for tool results
            parts: [
              {
                functionResponse: {
                  name: functionName,
                  response: JSON.parse(m.content), // Assuming tool content is JSON string
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

// --- Enhanced Service Factory ---

interface ServiceInstance {
  service: IAIService;
  apiKey: string;
  created: number;
}

export class AIServiceFactory {
  private static instances: Map<string, ServiceInstance> = new Map();
  private static readonly INSTANCE_TTL = 1000 * 60 * 60; // 1 hour

  static getService(
    provider: AIServiceProvider,
    apiKey: string,
    config?: AIServiceConfig,
  ): IAIService {
    const instanceKey = `${provider}:${apiKey}`;
    const now = Date.now();

    // Clean up expired instances
    this.cleanupExpiredInstances(now);

    const existing = this.instances.get(instanceKey);
    if (existing && now - existing.created < this.INSTANCE_TTL) {
      return existing.service;
    }

    // Dispose of old instance if it exists
    if (existing) {
      existing.service.dispose();
      this.instances.delete(instanceKey);
    }

    let service: IAIService;
    try {
      switch (provider) {
        case AIServiceProvider.Groq:
          service = new GroqService(apiKey, config);
          break;
        case AIServiceProvider.OpenAI:
          service = new OpenAIService(apiKey, config);
          break;
        case AIServiceProvider.Anthropic:
          service = new AnthropicService(apiKey, config);
          break;
        case AIServiceProvider.Gemini:
          service = new GeminiService(apiKey, config);
          break;
        default:
          logger.warn(
            `Unknown AI service provider: ${provider}. Returning EmptyAIService.`,
          );
          service = new EmptyAIService();
          break;
      }
    } catch (e) {
      logger.error(
        `Failed to create service for provider ${provider} with error: ${e}. Returning EmptyAIService.`,
      );
      service = new EmptyAIService();
    }

    this.instances.set(instanceKey, {
      service,
      apiKey,
      created: now,
    });

    return service;
  }

  static disposeAll(): void {
    for (const instance of this.instances.values()) {
      instance.service.dispose();
    }
    this.instances.clear();
  }

  private static cleanupExpiredInstances(now: number): void {
    for (const instanceKey of this.instances.keys()) {
      const instance = this.instances.get(instanceKey);
      if (instance && now - instance.created >= this.INSTANCE_TTL) {
        instance.service.dispose();
        this.instances.delete(instanceKey);
      }
    }
  }
}
