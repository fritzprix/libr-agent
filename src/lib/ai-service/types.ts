import type { ModelInfo } from '../llm-config-manager';

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
    messages: import('@/models/chat').Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: import('../mcp-types').MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;

  /**
   * Returns the list of supported models for this service.
   * For services like OpenAI/Anthropic, this returns static config data.
   * For services like Ollama, this may query the server dynamically.
   */
  listModels(): Promise<ModelInfo[]>;

  dispose(): void;
}
