import llmConfig from '../config/llm-config.json';
import { getLogger } from './logger';

const logger = getLogger('LLMConfigManager');

export interface ModelInfo {
  id?: string;
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

export interface ProviderInfo {
  id?: string;
  name: string;
  apiKeyEnvVar: string;
  baseUrl: string;
  models: Record<string, ModelInfo>;
}

export interface ServiceConfig {
  provider: string;
  model: string;
  temperature: number;
  maxTokens: number;
  topP: number;
  frequencyPenalty: number;
  presencePenalty: number;
}

export interface LLMConfig {
  providers: Record<string, ProviderInfo>;
}

export class LLMConfigManager {
  private config: LLMConfig;

  constructor() {
    this.config = llmConfig as LLMConfig;
  }

  // Provider 관련 메서드
  getProviders(): Record<string, ProviderInfo> {
    logger.info('🔍 Raw config providers:', this.config.providers);
    
    const result = Object.entries(this.config.providers)
      .map(([id, provider]) => ({ ...provider, id }))
      .reduce(
        (acc, v) => {
          acc[v.id] = v;
          return acc;
        },
        {} as Record<string, ProviderInfo>,
      );
    
    logger.info('✅ Processed providers:', result);
    return result;
  }

  getProvider(providerId: string): ProviderInfo | null {
    return this.config.providers[providerId] || null;
  }

  getProviderIds(): string[] {
    return Object.keys(this.config.providers);
  }

  // Model 관련 메서드
  getModel(providerId: string, modelId: string): ModelInfo | null {
    const provider = this.getProvider(providerId);
    return provider?.models[modelId] || null;
  }

  getModelsForProvider(providerId: string): Record<string, ModelInfo> | null {
    const provider = this.getProvider(providerId);
    return provider?.models || null;
  }

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

  getServiceIds(): string[] {
    return Object.keys(this.config.providers);
  }

  // Langchain 모델 ID 생성
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

  // 모델 필터링 메서드
  getModelsWithTools(): Array<{
    providerId: string;
    modelId: string;
    model: ModelInfo;
  }> {
    return this.getAllModels().filter(({ model }) => model.supportTools);
  }

  getModelsWithReasoning(): Array<{
    providerId: string;
    modelId: string;
    model: ModelInfo;
  }> {
    return this.getAllModels().filter(({ model }) => model.supportReasoning);
  }

  getModelsByCostRange(
    maxInputCost: number,
    maxOutputCost: number,
  ): Array<{ providerId: string; modelId: string; model: ModelInfo }> {
    return this.getAllModels().filter(
      ({ model }) =>
        model.cost.input <= maxInputCost && model.cost.output <= maxOutputCost,
    );
  }

  // Configuration validation
  validateServiceConfig(serviceConfig: ServiceConfig): boolean {
    const provider = this.getProvider(serviceConfig.provider);
    if (!provider) return false;

    const model = provider.models[serviceConfig.model];
    if (!model) return false;

    return true;
  }

  // 추천 모델 선택
  recommendModel(requirements: {
    needsTools?: boolean;
    needsReasoning?: boolean;
    maxCost?: number;
    preferSpeed?: boolean;
    contextWindow?: number;
  }): { providerId: string; modelId: string; model: ModelInfo } | null {
    let candidates = this.getAllModels();

    // 필터링
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

    // 정렬 및 선택
    if (requirements.preferSpeed) {
      // 비용이 낮은 모델을 속도가 빠른 것으로 간주
      candidates.sort(
        (a, b) =>
          Math.max(a.model.cost.input, a.model.cost.output) -
          Math.max(b.model.cost.input, b.model.cost.output),
      );
    } else {
      // 컨텍스트 윈도우가 큰 순으로 정렬 (성능 우선)
      candidates.sort((a, b) => b.model.contextWindow - a.model.contextWindow);
    }

    return candidates[0];
  }
}

// 싱글톤 인스턴스 생성
export const llmConfigManager = new LLMConfigManager();
