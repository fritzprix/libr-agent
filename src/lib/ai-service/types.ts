import type { ModelInfo } from '../llm-config-manager';
import type { MCPTool, SamplingOptions, SamplingResponse } from '../mcp-types';
import type { Message } from '@/models/chat';

export interface AIServiceConfig {
  timeout?: number;
  maxRetries?: number;
  retryDelay?: number;
  defaultModel?: string;
  maxTokens?: number;
  temperature?: number;
  tools?: MCPTool[];
}

export enum AIServiceProvider {
  Groq = 'groq',
  OpenAI = 'openai',
  Anthropic = 'anthropic',
  Gemini = 'gemini',
  Fireworks = 'fireworks',
  Cerebras = 'cerebras',
  Ollama = 'ollama',
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

  /**
   * MCP sampling API - 단일 프롬프트에서 텍스트 생성
   */
  sampleText(
    prompt: string,
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
    },
  ): Promise<SamplingResponse>;

  /**
   * Returns the list of supported models for this service.
   * For services like OpenAI/Anthropic, this returns static config data.
   * For services like Ollama, this may query the server dynamically.
   */
  listModels(): Promise<ModelInfo[]>;

  dispose(): void;
}
