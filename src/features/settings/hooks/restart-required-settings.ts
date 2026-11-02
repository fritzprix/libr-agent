import type { Settings } from '@/context/SettingsContext';

type SettingsSnapshot = Pick<Settings, 'system' | 'advanced'>;

// The backend reads these once at startup (src-tauri/src/lifecycle/app_setup),
// so a saved change applies only after an app restart.
const NETWORK_RESTART_KEYS = ['httpServerPort', 'httpServerExpose'] as const;
const RESTART_REQUIRED_SYSTEM_KEYS = [
  ...NETWORK_RESTART_KEYS,
  'searchIndexFrequencyMinutes',
  'webActionTimeoutSeconds',
] as const;
const RESTART_REQUIRED_ADVANCED_KEYS = [
  'maxConcurrentActiveSessions',
  'maxSuspendedSessions',
  'maxConcurrentActiveProcesses',
  'maxSuspendedProcesses',
] as const;

function hasChanged<T>(current: T, saved: T, keys: readonly (keyof T)[]) {
  return keys.some((key) => current[key] !== saved[key]);
}

/** HTTP server port/expose changed; drives the Network section notice. */
export function networkSettingsChanged(
  current: SettingsSnapshot,
  saved: SettingsSnapshot,
): boolean {
  return hasChanged(current.system, saved.system, NETWORK_RESTART_KEYS);
}

/** Any startup-only setting changed; saving it needs an app restart. */
export function restartRequiredSettingsChanged(
  current: SettingsSnapshot,
  saved: SettingsSnapshot,
): boolean {
  return (
    hasChanged(current.system, saved.system, RESTART_REQUIRED_SYSTEM_KEYS) ||
    hasChanged(current.advanced, saved.advanced, RESTART_REQUIRED_ADVANCED_KEYS)
  );
}
