import { AIServiceProvider } from '@/lib/ai-service';
import { llmConfigManager } from '@/lib/llm-config-manager';
import {
  REPEATED_THINKING_MIN_PATTERN_LENGTH,
  REPEATED_THINKING_MIN_REPETITIONS,
} from '@/context/llm/repeatedTailDetector';

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
  /**
   * Client-side cap on streamed thinking tokens for this endpoint.
   * Unset or 0 means unlimited.
   * Cross-reference: mapped to `AIServiceConfig.reasoningBudget` in `@/lib/ai-service/types`.
   */
  reasoningBudget?: number;
  /**
   * Instruction injected on the next turn after a client-side budget abort.
   * Cross-reference: mapped to `AIServiceConfig.reasoningBudgetMessage` in `@/lib/ai-service/types`.
   */
  reasoningBudgetMessage?: string;
}

/** User-defined OpenAI-compatible endpoint (vLLM, LM Studio, LocalAI, etc.). */
export interface CustomOpenAIProvider {
  /** Stable cuid used in session provider strings as `custom:<id>`. */
  id: string;
  /** Display name shown in Settings and the model picker. */
  name: string;
  /** OpenAI-compatible base URL (e.g. http://192.168.1.100:8000/v1). */
  baseUrl: string;
  /** Optional API key for authenticated endpoints. */
  apiKey?: string;
  /** Optional manual model IDs when /v1/models is unavailable. */
  models?: string[];
  /**
   * Client-side cap on streamed thinking tokens for this endpoint.
   * Unset or 0 means unlimited.
   */
  reasoningBudget?: number;
  /** Instruction injected on the next turn after a client-side budget abort. */
  reasoningBudgetMessage?: string;
}

export interface ModelChoice {
  /** Builtin AIServiceProvider id or `custom:<id>` for custom providers. */
  provider: string;
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
  loopPreventionThreshold: number; // default 3 — consecutive identical (call, outcome) streak before soft recovery
  /** Gap after soft recovery before hard break. Default 2 so Soft→Escalate→Hard can fire for error loops. */
  loopPreventionHardBreakOffset: number;
  thinkingLoopMinPatternLength: number; // default 256 — minimum repeating sequence length for thinking loops
  thinkingLoopMinRepetitions: number; // default 4 — minimum repetitions for thinking loops
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
  shellIsolationLevel: IsolationLevel;
  shellRuntimeBootstrap: boolean;
  skillsDirectory?: string;
  /**
   * When true (default), inhibit system idle sleep while LibrAgent is running.
   * Does not keep the display forced on.
   */
  preventSleepDuringAgentWork: boolean;
}

export interface ExperimentalSettings {
  inlineAudioAttachment: boolean;
}

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  /** Additional OpenAI-compatible providers beyond the builtin openai slot. */
  customProviders: CustomOpenAIProvider[];
  preferredModel: ModelChoice;
  fallbackModel?: ModelChoice;
  /**
   * When true, send `temperature` on AI service requests.
   * When false (default), omit temperature so provider/serving-engine defaults apply.
   */
  temperatureOverrideEnabled: boolean;
  /** Temperature used only when `temperatureOverrideEnabled` is true. Range 0–2. */
  temperature: number;
  contextStrategy: ContextStrategy;
  windowSize: number;
  maxInputContext: number;
  uiLanguage: string;
  toolCallGroupVisibleCount: number;
  agentHubUrl?: string;
  advanced: AdvancedSettings;
  display: DisplaySettings;
  system: SystemSettings;
  experimental: ExperimentalSettings;
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
  customProviders: [],
  preferredModel: {
    provider: DEFAULT_MODEL?.providerId || 'openai',
    model: DEFAULT_MODEL?.modelId || '',
  },
  fallbackModel: undefined,
  temperatureOverrideEnabled: false,
  temperature: 0.7,
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
    loopPreventionHardBreakOffset: 2,
    thinkingLoopMinPatternLength: REPEATED_THINKING_MIN_PATTERN_LENGTH,
    thinkingLoopMinRepetitions: REPEATED_THINKING_MIN_REPETITIONS,
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
    mcpServerStartupTimeoutSeconds: 30,
    mcpToolTimeoutSeconds: 0,
    searchIndexFrequencyMinutes: 5,
    scheduledTaskMinimumIntervalMinutes: 0,
    shellIsolationLevel: 'medium',
    shellRuntimeBootstrap: false,
    skillsDirectory: '',
    preventSleepDuringAgentWork: true,
  },
  experimental: {
    inlineAudioAttachment: true,
  },
};

export interface ISettingsService {
  getSettings(): Promise<Settings>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
}

// Export the Rust implementation as the default service
import { RustSettingsService } from './rust-settings-service';
export const settingsService = new RustSettingsService();
