import { AIServiceProvider } from '@/lib/ai-service';
import type { AdvancedSettings } from '@/lib/generated/AdvancedSettings';
import type { ExperimentalSettings } from '@/lib/generated/ExperimentalSettings';
import type { ToolLoopRecoveryPolicy } from '@/lib/generated/ToolLoopRecoveryPolicy';
import { llmConfigManager } from '@/lib/llm-config-manager';
import { DEFAULT_UI_SCALE, normalizeUiScale } from '@/lib/ui-scale';

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

/** Chat message chrome: classic bubbles vs full-width coding-agent document stream. */
export type MessageLayoutStyle = 'document' | 'bubble';

export type ColorTheme = 'neutral' | 'amber' | 'violet' | 'ocean' | 'forest';
export const COLOR_THEMES: ColorTheme[] = [
  'neutral',
  'amber',
  'violet',
  'ocean',
  'forest',
];

export function isColorTheme(val: unknown): val is ColorTheme {
  return (
    typeof val === 'string' && (COLOR_THEMES as readonly string[]).includes(val)
  );
}

export interface DisplaySettings {
  metricDisplayMode: 'tooltip' | 'inline';
  prefillDisplayFormat: 'time' | 'tokensPerSecond';
  showTokenSpeed: boolean;
  compactMetrics: boolean;
  /** Controls tool call display verbosity. 'simple' hides params/results/errors for regular users. */
  toolDetailLevel: 'simple' | 'developer';
  fontFamily: string;
  /** Webview zoom factor; one of `UI_SCALE_STEPS`. */
  uiScale: number;
  /**
   * Message layout style for the agent chat transcript.
   * - `document`: full-width document stream optimized for code/diffs (default)
   * - `bubble`: classic messenger bubbles
   */
  messageLayout: MessageLayoutStyle;
  colorTheme: ColorTheme;
  /** When true, show release/hub feature discovery tips on the agent hub. */
  showFeatureTips: boolean;
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
 * Frontend defaults. `advanced` and `experimental` are not here: Rust owns their
 * schema and defaults, `get_advanced_settings` / `get_experimental_settings`
 * always return the complete object, and `SettingsProvider` renders nothing that
 * reads settings before it arrives.
 */
export const DEFAULT_SETTING: Omit<Settings, 'advanced' | 'experimental'> = {
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
  display: {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
    toolDetailLevel: 'simple',
    fontFamily: 'Pretendard',
    uiScale: DEFAULT_UI_SCALE,
    messageLayout: 'document',
    colorTheme: 'neutral',
    showFeatureTips: true,
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
};

export function isMessageLayoutStyle(
  value: unknown,
): value is MessageLayoutStyle {
  return value === 'document' || value === 'bubble';
}

/**
 * Merge a stored displaySettings blob with defaults so newly added fields
 * (e.g. messageLayout) are present for older DB values.
 */
export function normalizeDisplaySettings(
  stored: unknown,
  defaults: DisplaySettings = DEFAULT_SETTING.display,
): DisplaySettings {
  const blob =
    typeof stored === 'object' && stored !== null
      ? (stored as Partial<DisplaySettings>)
      : {};

  return {
    metricDisplayMode:
      blob.metricDisplayMode === 'tooltip' ||
      blob.metricDisplayMode === 'inline'
        ? blob.metricDisplayMode
        : defaults.metricDisplayMode,
    prefillDisplayFormat:
      blob.prefillDisplayFormat === 'time' ||
      blob.prefillDisplayFormat === 'tokensPerSecond'
        ? blob.prefillDisplayFormat
        : defaults.prefillDisplayFormat,
    showTokenSpeed:
      typeof blob.showTokenSpeed === 'boolean'
        ? blob.showTokenSpeed
        : defaults.showTokenSpeed,
    compactMetrics:
      typeof blob.compactMetrics === 'boolean'
        ? blob.compactMetrics
        : defaults.compactMetrics,
    toolDetailLevel:
      blob.toolDetailLevel === 'simple' || blob.toolDetailLevel === 'developer'
        ? blob.toolDetailLevel
        : defaults.toolDetailLevel,
    fontFamily:
      typeof blob.fontFamily === 'string' && blob.fontFamily.length > 0
        ? blob.fontFamily
        : defaults.fontFamily,
    uiScale:
      blob.uiScale === undefined
        ? defaults.uiScale
        : normalizeUiScale(blob.uiScale),
    messageLayout: isMessageLayoutStyle(blob.messageLayout)
      ? blob.messageLayout
      : defaults.messageLayout,
    colorTheme: isColorTheme(blob.colorTheme)
      ? blob.colorTheme
      : defaults.colorTheme,
    showFeatureTips:
      typeof blob.showFeatureTips === 'boolean'
        ? blob.showFeatureTips
        : defaults.showFeatureTips,
  };
}

export interface ISettingsService {
  getSettings(): Promise<Settings>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
}

// Export the Rust implementation as the default service
import { RustSettingsService } from './rust-settings-service';
export const settingsService = new RustSettingsService();
