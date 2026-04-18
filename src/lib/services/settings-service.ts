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
  diffContextLines: number;
  defaultMaxOutputTokens: number;
  toolResultInlineLimitBytes: number;
  defaultSessionMaxDepth: number;
  defaultSessionMaxFanout: number;
  // SP2: Global concurrent execution limits (runtime semaphores, not per-parent counts)
  maxConcurrentActiveSessions: number; // default 4 — simultaneous LLM loops
  maxSuspendedSessions: number; // default 8 — sessions blocked on awaitAgent
  maxConcurrentActiveProcesses: number; // default 10 — simultaneous shell processes
  maxSuspendedProcesses: number; // default 20 — processes blocked on pollProcess
  loopPreventionThreshold: number; // default 3 - number of identical tool calls to trigger natural recovery
}

export interface DisplaySettings {
  metricDisplayMode: 'tooltip' | 'inline';
  prefillDisplayFormat: 'time' | 'tokensPerSecond';
  showTokenSpeed: boolean;
  compactMetrics: boolean;
  /** Controls tool call display verbosity. 'simple' hides params/results/errors for regular users. */
  toolDetailLevel: 'simple' | 'developer';
  fontFamily: string;
}

export type IsolationLevel = 'basic' | 'medium' | 'high';

/** Context management strategy:
 * - 'window': sliding window — keep the N most recent messages (existing behavior)
 * - 'compact': async compaction — summarize old turns, keep recent window
 */
export type ContextStrategy = 'window' | 'compact';

export interface SystemSettings {
  maxFileUploadSizeMB: number;
  webActionTimeoutSeconds: number;
  httpServerPort: number;
  httpServerExpose: boolean;
  mcpServerStartupTimeoutSeconds: number;
  mcpToolTimeoutSeconds: number;
  searchIndexFrequencyMinutes: number;
  scheduledTaskMinimumIntervalMinutes: number;
  maxScheduledTaskGroups: number;
  shellIsolationLevel: IsolationLevel;
  skillsDirectory?: string;
}

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  preferredModel: ModelChoice;
  fallbackModel?: ModelChoice;
  contextStrategy: ContextStrategy;
  windowSize: number;
  maxInputContext: number;
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
  contextStrategy: 'compact',
  windowSize: 20,
  maxInputContext: 49152,
  uiLanguage: 'en',
  toolCallGroupVisibleCount: 4,
  agentHubUrl: '',
  advanced: {
    maxRetries: 1,
    retryDelay: 5000,
    diffContextLines: 3,
    defaultMaxOutputTokens: 8192,
    toolResultInlineLimitBytes: 16 * 1024,
    defaultSessionMaxDepth: 0,
    defaultSessionMaxFanout: 0,
    maxConcurrentActiveSessions: 4,
    maxSuspendedSessions: 8,
    maxConcurrentActiveProcesses: 10,
    maxSuspendedProcesses: 20,
    loopPreventionThreshold: 3,
  },
  display: {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
    toolDetailLevel: 'simple',
    fontFamily: 'Pretendard',
  },
  system: {
    maxFileUploadSizeMB: 50,
    webActionTimeoutSeconds: 30,
    httpServerPort: 3030,
    httpServerExpose: false,
    mcpServerStartupTimeoutSeconds: 60,
    mcpToolTimeoutSeconds: 0,
    searchIndexFrequencyMinutes: 5,
    scheduledTaskMinimumIntervalMinutes: 0,
    maxScheduledTaskGroups: 10,
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
