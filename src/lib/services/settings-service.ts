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
  use3rdParty?: boolean;
  customModelId?: string;
}

export interface ModelChoice {
  provider: AIServiceProvider;
  model: string;
}

export interface AdvancedSettings {
  maxRetries: number;
  retryDelay: number; // in milliseconds
  circuitBreakerThreshold: number;
  diffContextLines: number;
  defaultMaxOutputTokens: number;
  defaultSessionMaxDepth: number;
  defaultSessionMaxFanout: number;
  // SP2: Global concurrent execution limits (runtime semaphores, not per-parent counts)
  maxConcurrentActiveSessions: number; // default 4 — simultaneous LLM loops
  maxSuspendedSessions: number; // default 8 — sessions blocked on awaitAgent
  maxConcurrentActiveProcesses: number; // default 10 — simultaneous shell processes
  maxSuspendedProcesses: number; // default 20 — processes blocked on pollProcess
}

export interface DisplaySettings {
  metricDisplayMode: 'tooltip' | 'inline';
  prefillDisplayFormat: 'time' | 'tokensPerSecond';
  showTokenSpeed: boolean;
  compactMetrics: boolean;
  /** Controls tool call display verbosity. 'simple' hides params/results/errors for regular users. */
  toolDetailLevel: 'simple' | 'developer';
}

export type IsolationLevel = 'basic' | 'medium' | 'high';

export interface SystemSettings {
  maxFileUploadSizeMB: number;
  workspaceCapacityMB: number;
  webActionTimeoutSeconds: number;
  httpServerPort: number;
  httpServerExpose: boolean;
  mcpServerStartupTimeoutSeconds: number;
  mcpToolTimeoutSeconds: number;
  searchIndexFrequencyMinutes: number;
  activeSessionRetentionHours: number;
  shellIsolationLevel: IsolationLevel;
  skillsDirectory?: string;
}

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  preferredModel: ModelChoice;
  fallbackModel?: ModelChoice;
  windowSize: number;
  uiLanguage: string;
  toolCallGroupVisibleCount: number;
  agentHubUrl?: string;
  advanced: AdvancedSettings;
  display: DisplaySettings;
  system: SystemSettings;
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
  fallbackModel: undefined,
  windowSize: 20,
  uiLanguage: 'en',
  toolCallGroupVisibleCount: 4,
  agentHubUrl: '',
  advanced: {
    maxRetries: 1,
    retryDelay: 5000,
    circuitBreakerThreshold: 3,
    diffContextLines: 3,
    defaultMaxOutputTokens: 8192,
    defaultSessionMaxDepth: 0,
    defaultSessionMaxFanout: 0,
    maxConcurrentActiveSessions: 4,
    maxSuspendedSessions: 8,
    maxConcurrentActiveProcesses: 10,
    maxSuspendedProcesses: 20,
  },
  display: {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
    toolDetailLevel: 'simple',
  },
  system: {
    maxFileUploadSizeMB: 50,
    workspaceCapacityMB: 10,
    webActionTimeoutSeconds: 30,
    httpServerPort: 3030,
    httpServerExpose: false,
    mcpServerStartupTimeoutSeconds: 60,
    mcpToolTimeoutSeconds: 0,
    searchIndexFrequencyMinutes: 5,
    activeSessionRetentionHours: 24,
    shellIsolationLevel: 'medium',
    skillsDirectory: '',
  },
};

export interface ISettingsService {
  getSettings(): Promise<Settings>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
}

// Export the Rust implementation as the default service
import { RustSettingsService } from './rust-settings-service';
export const settingsService = new RustSettingsService();
