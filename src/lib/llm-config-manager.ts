import llmConfig from '../config/llm-config.json';

/**
 * Describes the capabilities and metadata of a specific language model.
 */
export interface ModelInfo {
  /** The unique identifier for the model, if available. */
  id?: string;
  /** The display name of the model. */
  name: string;
  /** The maximum context window size in tokens. */
  contextWindow: number;
  /** Indicates if the model supports reasoning or complex instruction following. */
  supportReasoning: boolean;
  /** Indicates if the model supports tool use. */
  supportTools: boolean;
  /** Indicates if the model supports streaming responses. */
  supportStreaming: boolean;
  /** The cost per million tokens for input and output. */
  cost: {
    /** Cost per million input tokens in USD. */
    input: number;
    /** Cost per million output tokens in USD. */
    output: number;
  };
  /** A brief description of the model. */
  description: string;
}

/**
 * Describes a language model provider, including its models and API configuration.
 */
export interface ProviderInfo {
  /** The unique identifier for the provider. */
  id?: string;
  /** The display name of the provider. */
  name: string;
  /** The name of the environment variable that holds the API key. */
  apiKeyEnvVar: string;
  /** The base URL for the provider's API. */
  baseUrl: string;
  /** A record of the models offered by this provider, keyed by model ID. */
  models: Record<string, ModelInfo>;
}

/**
 * Defines the configuration for a specific AI service, including model and parameters.
 */
export interface ServiceConfig {
  /** The ID of the provider to use. */
  provider: string;
  /** The ID of the model to use. */
  model: string;
  /** The sampling temperature. */
  temperature: number;
  /** The maximum number of tokens to generate. */
  maxTokens: number;
  /** The nucleus sampling probability. */
  topP: number;
  /** The frequency penalty. */
  frequencyPenalty: number;
  /** The presence penalty. */
  presencePenalty: number;
}

/**
 * Represents the root structure of the LLM configuration file.
 */
export interface LLMConfig {
  /** A record of all available providers, keyed by provider ID. */
  providers: Record<string, ProviderInfo>;
}

/**
 * Manages the configuration of LLM providers and models.
 * This class provides methods to access, filter, and validate LLM configurations.
 */
export class LLMConfigManager {
  private config: LLMConfig;

  /**
   * Initializes a new instance of the LLMConfigManager, loading the configuration
   * from the `llm-config.json` file.
   */
  constructor() {
    this.config = llmConfig as LLMConfig;
  }

  /**
   * Gets all configured providers.
   * @returns A record of all providers, keyed by their ID.
   */
  getProviders(): Record<string, ProviderInfo> {
    const result = Object.entries(this.config.providers)
      .map(([id, provider]) => ({ ...provider, id }))
      .reduce(
        (acc, v) => {
          acc[v.id] = v;
          return acc;
        },
        {} as Record<string, ProviderInfo>,
      );

    return result;
  }

  /**
   * Gets a specific provider by its ID.
   * @param providerId The ID of the provider to retrieve.
   * @returns The provider information, or null if not found.
   */
  getProvider(providerId: string): ProviderInfo | null {
    return this.config.providers[providerId] || null;
  }

  /**
   * Gets a list of all provider IDs.
   * @returns An array of provider ID strings.
   */
  getProviderIds(): string[] {
    return Object.keys(this.config.providers);
  }

  /**
   * Gets a specific model from a specific provider.
   * @param providerId The ID of the provider.
   * @param modelId The ID of the model.
   * @returns The model information, or null if not found.
   */
  getModel(providerId: string, modelId: string): ModelInfo | null {
    const provider = this.getProvider(providerId);
    return provider?.models[modelId] || null;
  }

  /**
   * Gets all models available for a specific provider.
   * @param providerId The ID of the provider.
   * @returns A record of models for the provider, or null if the provider is not found.
   */
  getModelsForProvider(providerId: string): Record<string, ModelInfo> | null {
    const provider = this.getProvider(providerId);
    return provider?.models || null;
  }

  /**
   * Gets a flattened list of all models from all providers.
   * @returns An array of objects, each containing the provider ID, model ID, and model info.
   */
  getAllModels(): Array<{
    providerId: string;
    modelId: string;
    model: ModelInfo;
  }> {
    const models: Array<{
      providerId: string;
      modelId: string;
      model: ModelInfo;
    }> = [];

    for (const [providerId, provider] of Object.entries(
      this.config.providers,
    )) {
      for (const [modelId, model] of Object.entries(provider.models)) {
        models.push({ providerId, modelId, model });
      }
    }

    return models;
  }

  /**
   * Gets a list of all service IDs, which are equivalent to provider IDs.
   * @returns An array of service ID strings.
   */
  getServiceIds(): string[] {
    return Object.keys(this.config.providers);
  }

  /**
   * Generates a LangChain-compatible model identifier.
   * @param providerId The ID of the provider.
   * @param modelId The ID of the model.
   * @returns A string in the format `langchain_provider:modelId`.
   * @throws An error if the provider is unknown.
   */
  getLangchainModelId(providerId: string, modelId: string): string {
    const providerMap: Record<string, string> = {
      openai: 'openai',
      anthropic: 'anthropic',
      groq: 'groq',
      google: 'google-genai',
    };

    const langchainProvider = providerMap[providerId];
    if (!langchainProvider) {
      throw new Error(`Unknown provider: ${providerId}`);
    }

    return `${langchainProvider}:${modelId}`;
  }

  /**
   * Filters all models to find those that support tool use.
   * @returns An array of models that support tools.
   */
  getModelsWithTools(): Array<{
    providerId: string;
    modelId: string;
    model: ModelInfo;
  }> {
    return this.getAllModels().filter(({ model }) => model.supportTools);
  }

  /**
   * Filters all models to find those that support reasoning.
   * @returns An array of models that support reasoning.
   */
  getModelsWithReasoning(): Array<{
    providerId: string;
    modelId: string;
    model: ModelInfo;
  }> {
    return this.getAllModels().filter(({ model }) => model.supportReasoning);
  }

  /**
   * Filters models based on a maximum cost for input and output tokens.
   * @param maxInputCost The maximum cost per million input tokens.
   * @param maxOutputCost The maximum cost per million output tokens.
   * @returns An array of models that fall within the specified cost range.
   */
  getModelsByCostRange(
    maxInputCost: number,
    maxOutputCost: number,
  ): Array<{ providerId: string; modelId: string; model: ModelInfo }> {
    return this.getAllModels().filter(
      ({ model }) =>
        model.cost.input <= maxInputCost && model.cost.output <= maxOutputCost,
    );
  }

  /**
   * Validates a service configuration to ensure the provider and model exist.
   * @param serviceConfig The service configuration to validate.
   * @returns True if the configuration is valid, false otherwise.
   */
  validateServiceConfig(serviceConfig: ServiceConfig): boolean {
    const provider = this.getProvider(serviceConfig.provider);
    if (!provider) return false;

    const model = provider.models[serviceConfig.model];
    if (!model) return false;

    return true;
  }

  /**
   * Recommends a model based on a set of requirements.
   * It filters the available models and sorts them based on the specified criteria.
   *
   * @param requirements The requirements for the model, such as tool support, cost, and speed.
   * @returns The recommended model and its provider, or null if no suitable model is found.
   */
  recommendModel(requirements: {
    needsTools?: boolean;
    needsReasoning?: boolean;
    maxCost?: number;
    preferSpeed?: boolean;
    contextWindow?: number;
  }): { providerId: string; modelId: string; model: ModelInfo } | null {
    let candidates = this.getAllModels();

    // Filter candidates based on requirements
    if (requirements.needsTools) {
      candidates = candidates.filter(({ model }) => model.supportTools);
    }

    if (requirements.needsReasoning) {
      candidates = candidates.filter(({ model }) => model.supportReasoning);
    }

    if (requirements.maxCost !== undefined) {
      candidates = candidates.filter(
        ({ model }) =>
          Math.max(model.cost.input, model.cost.output) <=
          requirements.maxCost!,
      );
    }

    if (requirements.contextWindow !== undefined) {
      candidates = candidates.filter(
        ({ model }) => model.contextWindow >= requirements.contextWindow!,
      );
    }

    if (candidates.length === 0) return null;

    // Sort and select the best candidate
    if (requirements.preferSpeed) {
      // Consider lower-cost models to be faster
      candidates.sort(
        (a, b) =>
          Math.max(a.model.cost.input, a.model.cost.output) -
          Math.max(b.model.cost.input, b.model.cost.output),
      );
    } else {
      // Sort by context window size in descending order (performance priority)
      candidates.sort((a, b) => b.model.contextWindow - a.model.contextWindow);
    }

    return candidates[0];
  }
}

/**
 * A singleton instance of the LLMConfigManager, providing global access to LLM configurations.
 */
export const llmConfigManager = new LLMConfigManager();
