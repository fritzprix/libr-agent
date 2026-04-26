import { getLaunchLogLevel, isLoggerLevel } from './logger-config';
import { Logger } from './logger-core';
import { logFileManager } from './logger-file-manager';
import type { LoggerConfig } from './logger-types';

const LOGGER_CONFIG_STORAGE_KEY = 'libragent-logger-config';

function isStoredLoggerConfig(value: unknown): value is LoggerConfig {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const config = value as Record<string, unknown>;
  return (
    typeof config.enableFileLogging === 'boolean' &&
    typeof config.autoBackupOnStartup === 'boolean' &&
    typeof config.maxBackupFiles === 'number' &&
    isLoggerLevel(config.logLevel) &&
    typeof config.maxMessageLength === 'number'
  );
}

export const logUtils = {
  initialize: async (config?: Partial<LoggerConfig>): Promise<void> => {
    try {
      const savedConfig = await logUtils.loadConfig();
      if (savedConfig) {
        Logger.updateConfig(savedConfig);
      }
    } catch (error) {
      console.warn('Failed to load saved logger config:', error);
    }

    if (config) {
      Logger.updateConfig(config);
      await logUtils.saveConfig();
    }

    const launchLogLevel = await getLaunchLogLevel();
    if (launchLogLevel) {
      Logger.updateConfig({ logLevel: launchLogLevel });
    }

    await Logger.initialize();
  },

  updateConfig: async (config: Partial<LoggerConfig>): Promise<void> => {
    Logger.updateConfig(config);
    await logUtils.saveConfig();
  },

  getConfig: (): LoggerConfig => {
    return Logger.getConfig();
  },

  resetConfig: async (): Promise<void> => {
    Logger.resetConfig();
    await logUtils.saveConfig();
  },

  saveConfig: async (): Promise<void> => {
    try {
      localStorage.setItem(
        LOGGER_CONFIG_STORAGE_KEY,
        JSON.stringify(Logger.getConfig()),
      );
    } catch (error) {
      console.error('Failed to save logger config:', error);
    }
  },

  loadConfig: async (): Promise<LoggerConfig | null> => {
    try {
      const configStr = localStorage.getItem(LOGGER_CONFIG_STORAGE_KEY);
      if (!configStr) {
        return null;
      }

      const parsed: unknown = JSON.parse(configStr);
      return isStoredLoggerConfig(parsed) ? parsed : null;
    } catch (error) {
      console.error('Failed to load logger config:', error);
      return null;
    }
  },

  backupNow: async (): Promise<string> => {
    return await logFileManager.backupCurrentLog();
  },

  clearLogs: async (): Promise<void> => {
    await logFileManager.clearCurrentLog();
  },

  getLogDirectory: async (): Promise<string> => {
    return await logFileManager.getLogDirectory();
  },

  listAllLogFiles: async (): Promise<string[]> => {
    return await logFileManager.listLogFiles();
  },

  setLogLevel: async (level: LoggerConfig['logLevel']): Promise<void> => {
    await logUtils.updateConfig({ logLevel: level });
  },

  enableFileLogging: async (enabled: boolean = true): Promise<void> => {
    await logUtils.updateConfig({ enableFileLogging: enabled });
  },

  enableAutoBackup: async (enabled: boolean = true): Promise<void> => {
    await logUtils.updateConfig({ autoBackupOnStartup: enabled });
  },

  setMaxMessageLength: async (length: number): Promise<void> => {
    await logUtils.updateConfig({ maxMessageLength: length });
  },
};
