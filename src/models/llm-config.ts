// This file defines TypeScript interfaces for the LLM config JSON structure in llm-config.json

export interface LLMConfig {
  providers: Record<string, LLMProviderConfig>;
  serviceConfigs: Record<string, LLMServiceConfig>;
  preferences: LLMPreferences;
}

export interface LLMProviderConfig {
  name: string;
  apiKeyEnvVar: string;
  baseUrl: string;
  models: Record<string, LLMModelConfig>;
}

export interface LLMModelConfig {
  name: string;
  contextWindow: number;
  supportReasoning: boolean;
  supportTools: boolean;
  supportStreaming: boolean;
  cost: {
    input: number;
    output: number;
  };
  description: string;
}

export interface LLMServiceConfig {
  provider: string;
  model: string;
  temperature: number;
  maxTokens: number;
  topP: number;
  frequencyPenalty: number;
  presencePenalty: number;
}

export interface LLMPreferences {
  defaultService: string;
  fallbackService: string;
  retryAttempts: number;
  timeoutMs: number;
  enableCaching: boolean;
  enableLogging: boolean;
  costOptimization: boolean;
  autoModelSelection: boolean;
}
