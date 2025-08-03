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

  dispose(): void;
}
