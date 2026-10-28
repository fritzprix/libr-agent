import {
  DEFAULT_SETTING,
  type AdvancedSettings,
  type Settings,
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

/** Complete settings for tests: frontend defaults plus the advanced fixture. */
export const TEST_SETTINGS: Settings = {
  ...DEFAULT_SETTING,
  advanced: TEST_ADVANCED_SETTINGS,
};
