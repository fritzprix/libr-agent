import { AIServiceProvider } from '@/lib/ai-service';
import { llmConfigManager } from '@/lib/llm-config-manager';

export interface SafetySetting {
  category: string;
  threshold: string;
}

export interface ServiceConfig {
  apiKey?: string;
  baseUrl?: string;
  safetySettings?: SafetySetting[];
}

export interface ModelChoice {
  provider: AIServiceProvider;
  model: string;
}

export interface AdvancedSettings {
  maxRetries: number;
  retryDelay: number; // in milliseconds
  circuitBreakerThreshold: number;
}

export interface DisplaySettings {
  metricDisplayMode: 'tooltip' | 'inline';
  prefillDisplayFormat: 'time' | 'tokensPerSecond';
  showTokenSpeed: boolean;
  compactMetrics: boolean;
}

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  preferredModel: ModelChoice;
  windowSize: number;
  uiLanguage: string;
  toolCallGroupVisibleCount: number;
  agentHubUrl?: string;
  advanced: AdvancedSettings;
  display: DisplaySettings;
}

const DEFAULT_MODEL = llmConfigManager.recommendModel({});

export const DEFAULT_SETTING: Settings = {
  serviceConfigs: Object.values(AIServiceProvider).reduce(
    (acc, provider) => {
      acc[provider] = {};
      return acc;
    },
    {} as Record<AIServiceProvider, ServiceConfig>,
  ),
  preferredModel: {
    provider: (DEFAULT_MODEL?.providerId || 'openai') as AIServiceProvider,
    model: DEFAULT_MODEL?.modelId || '',
  },
  windowSize: 20,
  uiLanguage: 'en',
  toolCallGroupVisibleCount: 4,
  agentHubUrl: '',
  advanced: {
    maxRetries: 1,
    retryDelay: 5000,
    circuitBreakerThreshold: 3,
  },
  display: {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
  },
};

export interface ISettingsService {
  getSettings(): Promise<Settings>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
}

// Export the Rust implementation as the default service
import { RustSettingsService } from './rust-settings-service';
export const settingsService = new RustSettingsService();
