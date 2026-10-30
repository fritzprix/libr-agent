import {
  DEFAULT_SETTING,
  type AdvancedSettings,
  type DisplaySettings,
  type ExperimentalSettings,
  type Settings,
  type SystemSettings,
} from '@/lib/services/settings-service';

/**
 * Test data only. The app never has a frontend `advanced` default: Rust owns it
 * and returns it from `get_advanced_settings`. Values mirror the Rust defaults.
 */
export const TEST_ADVANCED_SETTINGS: AdvancedSettings = {
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
  thinkingLoopMinPatternLength: 256,
  thinkingLoopMinRepetitions: 4,
  maxRecentMediaMessages: 1,
  thinkingEffort: 'off',
};

/** Test data only: Rust owns `display` (`get_display_settings`). */
export const TEST_DISPLAY_SETTINGS: DisplaySettings = {
  metricDisplayMode: 'inline',
  prefillDisplayFormat: 'time',
  showTokenSpeed: true,
  compactMetrics: false,
  toolDetailLevel: 'simple',
  fontFamily: 'Pretendard',
  uiScale: 1.1,
  messageLayout: 'document',
  colorTheme: 'neutral',
  showFeatureTips: true,
};

/** Test data only: Rust owns `experimental` (`get_experimental_settings`). */
export const TEST_EXPERIMENTAL_SETTINGS: ExperimentalSettings = {
  inlineAudioAttachment: true,
  toolLoopRecoveryPolicy: 'resampleThenBreak',
  toolLoopMaxResampleRetries: 2,
};

/** Test data only: Rust owns `system` (`get_system_settings`). */
export const TEST_SYSTEM_SETTINGS: SystemSettings = {
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
};

/** Complete settings for tests: frontend defaults plus the Rust-owned fixtures. */
export const TEST_SETTINGS: Settings = {
  ...DEFAULT_SETTING,
  advanced: TEST_ADVANCED_SETTINGS,
  display: TEST_DISPLAY_SETTINGS,
  experimental: TEST_EXPERIMENTAL_SETTINGS,
  system: TEST_SYSTEM_SETTINGS,
};
