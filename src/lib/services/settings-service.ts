import { AIServiceProvider } from '@/lib/ai-service';
import type { AdvancedSettings } from '@/lib/generated/AdvancedSettings';
import type { ExperimentalSettings } from '@/lib/generated/ExperimentalSettings';
import type { ToolLoopRecoveryPolicy } from '@/lib/generated/ToolLoopRecoveryPolicy';
import { llmConfigManager } from '@/lib/llm-config-manager';
import type { ColorTheme } from '@/lib/generated/ColorTheme';
import type { DisplaySettings } from '@/lib/generated/DisplaySettings';
import type { MessageLayoutStyle } from '@/lib/generated/MessageLayoutStyle';

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
}

export interface ModelChoice {
  /** Builtin AIServiceProvider id or `custom:<id>` for custom providers. */
  provider: string;
  model: string;
}

/** Sub-agent model override per task complexity. `null` / omitted = inherit preferredModel. */
export type ComplexityLevel = 'low' | 'normal' | 'high';

export type ComplexityModelMapping = {
  low?: ModelChoice | null;
  normal?: ModelChoice | null;
  high?: ModelChoice | null;
};

export const COMPLEXITY_LEVELS: ComplexityLevel[] = ['low', 'normal', 'high'];

export function normalizeComplexityModelMapping(
  value: unknown,
): ComplexityModelMapping {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return {};
  }
  const obj = value as Record<string, unknown>;
  const result: ComplexityModelMapping = {};
  for (const level of COMPLEXITY_LEVELS) {
    const entry = obj[level];
    if (entry === null || entry === undefined) {
      result[level] = null;
      continue;
    }
    if (typeof entry !== 'object' || Array.isArray(entry)) {
      result[level] = null;
      continue;
    }
    const model = (entry as Record<string, unknown>).model;
    const provider = (entry as Record<string, unknown>).provider;
    if (
      typeof model === 'string' &&
      model.trim() &&
      typeof provider === 'string' &&
      provider.trim()
    ) {
      result[level] = { model: model.trim(), provider: provider.trim() };
    } else {
      result[level] = null;
    }
  }
  return result;
}

/** Normalized by Rust (`get_advanced_settings`); generated from the Rust model. */
export type { AdvancedSettings };

/** Normalized by Rust (`get_display_settings`); generated from the Rust model. */
export type { ColorTheme, DisplaySettings, MessageLayoutStyle };

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

/** Normalized by Rust (`get_experimental_settings`); generated from the Rust model. */
export type { ExperimentalSettings, ToolLoopRecoveryPolicy };

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  /** Additional OpenAI-compatible providers beyond the builtin openai slot. */
  customProviders: CustomOpenAIProvider[];
  preferredModel: ModelChoice;
  fallbackModel?: ModelChoice;
  /** Per-complexity model overrides for sub-agent spawn / message routing. */
  complexityModelMapping: ComplexityModelMapping;
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

/**
 * Frontend defaults. `advanced`, `display` and `experimental` are not here: Rust owns their
 * schema and defaults, `get_advanced_settings` / `get_display_settings` /
 * `get_experimental_settings` always return the complete object, and `SettingsProvider` renders nothing that
 * reads settings before it arrives.
 */
export const DEFAULT_SETTING: Omit<
  Settings,
  'advanced' | 'display' | 'experimental'
> = {
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
  complexityModelMapping: {},
  temperatureOverrideEnabled: false,
  temperature: 0.7,
  contextStrategy: 'compact',
  windowSize: 20,
  maxInputContext: 49152,
  uiLanguage: 'en',
  toolCallGroupVisibleCount: 4,
  agentHubUrl: '',
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
};

export interface ISettingsService {
  getSettings(): Promise<Settings>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
}

// Export the Rust implementation as the default service
import { RustSettingsService } from './rust-settings-service';
export const settingsService = new RustSettingsService();
