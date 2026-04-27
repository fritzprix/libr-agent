import { invoke } from '@tauri-apps/api/core';

import type { LoggerConfig } from './logger-types';

export function isLoggerLevel(
  value: unknown,
): value is LoggerConfig['logLevel'] {
  return (
    value === 'trace' ||
    value === 'debug' ||
    value === 'info' ||
    value === 'warn' ||
    value === 'error'
  );
}

export async function getLaunchLogLevel(): Promise<
  LoggerConfig['logLevel'] | null
> {
  try {
    const level = await invoke<string>('get_launch_log_level');
    return isLoggerLevel(level) ? level : null;
  } catch (error) {
    console.warn('Failed to read launch log level override:', error);
    return null;
  }
}

const DEFAULT_CONFIG: LoggerConfig = {
  enableFileLogging: true,
  autoBackupOnStartup: true,
  maxBackupFiles: 10,
  logLevel: 'info',
  maxMessageLength: 50000,
};

let globalLoggerConfig: LoggerConfig = { ...DEFAULT_CONFIG };

export function getLoggerConfig(): LoggerConfig {
  return { ...globalLoggerConfig };
}

export function updateLoggerConfig(config: Partial<LoggerConfig>): void {
  globalLoggerConfig = { ...globalLoggerConfig, ...config };
}

export function resetLoggerConfig(): void {
  globalLoggerConfig = { ...DEFAULT_CONFIG };
}
