/**
 * @file LibrAgent Global Logger System
 *
 * @description
 * This module provides a comprehensive logging solution for the application,
 * built on top of the `@tauri-apps/plugin-log` package.
 *
 * @features
 * - **File Logging**: Automatic file logging to standard platform-specific paths.
 * - **Log Backup**: Automatic backup of the log file on application startup.
 * - **Log Level Filtering**: Supports 'trace', 'debug', 'info', 'warn', and 'error' levels.
 * - **Persistent Configuration**: Saves logger settings (e.g., log level) to local storage.
 * - **Contextual Logging**: Allows creating logger instances with specific contexts (e.g., component names).
 *
 * @example
 * ```typescript
 * import { getLogger, logUtils } from '@/lib/logger';
 *
 * // In your app's entry point (e.g., main.tsx), this is called automatically.
 * await logUtils.initialize();
 *
 * // Using a context-specific logger in a component.
 * const logger = getLogger('MyComponent');
 * logger.info('Component initialized');
 *
 * // Changing logger settings.
 * await logUtils.setLogLevel('debug');
 * await logUtils.enableFileLogging(true);
 *
 * // Managing log files.
 * const logDir = await logUtils.getLogDirectory();
 * const files = await logUtils.listAllLogFiles();
 * await logUtils.backupNow();
 * ```
 */

import {
  debug,
  info,
  warn,
  error as logError,
  trace,
} from '@tauri-apps/plugin-log';
import { invoke } from '@tauri-apps/api/core';

/**
 * Defines the configuration options for the global logger.
 */
export interface LoggerConfig {
  /** If true, logs will be written to a file in the app's log directory. */
  enableFileLogging: boolean;
  /** If true, the current log file will be backed up when the app starts. */
  autoBackupOnStartup: boolean;
  /** The maximum number of backup log files to keep. */
  maxBackupFiles: number;
  /** The minimum level of logs to record. */
  logLevel: 'trace' | 'debug' | 'info' | 'warn' | 'error';
  /** The maximum length of a log message in characters. Messages longer than this will be truncated. */
  maxMessageLength: number;
}

// Default configuration
const DEFAULT_CONFIG: LoggerConfig = {
  enableFileLogging: true,
  autoBackupOnStartup: true,
  maxBackupFiles: 10,
  logLevel: 'info',
  maxMessageLength: 50000,
};

// Global configuration store
let globalLoggerConfig: LoggerConfig = { ...DEFAULT_CONFIG };

/**
 * Defines the interface for managing log files.
 */
export interface LogFileManager {
  /**
   * Gets the path to the directory where log files are stored.
   * @returns A promise that resolves to the log directory path.
   */
  getLogDirectory(): Promise<string>;
  /**
   * Creates a backup of the current log file.
   * @returns A promise that resolves to the path of the new backup file.
   */
  backupCurrentLog(): Promise<string>;
  /**
   * Clears the content of the current log file.
   * @returns A promise that resolves when the log has been cleared.
   */
  clearCurrentLog(): Promise<void>;
  /**
   * Lists all log files in the log directory.
   * @returns A promise that resolves to an array of log file names.
   */
  listLogFiles(): Promise<string[]>;
}

/**
 * An implementation of the `LogFileManager` interface that uses Tauri's `invoke`
 * to call Rust backend functions for file management.
 * @internal
 */
class TauriLogFileManager implements LogFileManager {
  async getLogDirectory(): Promise<string> {
    return await invoke<string>('get_app_logs_dir');
  }

  async backupCurrentLog(): Promise<string> {
    return await invoke<string>('backup_current_log');
  }

  async clearCurrentLog(): Promise<void> {
    await invoke('clear_current_log');
  }

  async listLogFiles(): Promise<string[]> {
    return await invoke<string[]>('list_log_files');
  }
}

/**
 * A singleton instance of the `TauriLogFileManager` for global use.
 */
export const logFileManager = new TauriLogFileManager();

/**
 * A static class that provides core logging functionalities.
 * It manages the logger's configuration and provides static methods for logging
 * at different levels.
 */
export class Logger {
  private static defaultContext = 'TauriAgent';
  private static hasBackedUpOnStartup = false;

  /**
   * Updates the global logger configuration.
   * @param config A partial `LoggerConfig` object with the settings to update.
   */
  static updateConfig(config: Partial<LoggerConfig>): void {
    globalLoggerConfig = { ...globalLoggerConfig, ...config };
    console.log('Logger config updated:', globalLoggerConfig);
  }

  /**
   * Gets the current global logger configuration.
   * @returns A copy of the current `LoggerConfig`.
   */
  static getConfig(): LoggerConfig {
    return { ...globalLoggerConfig };
  }

  /**
   * Resets the logger configuration to its default values.
   */
  static resetConfig(): void {
    globalLoggerConfig = { ...DEFAULT_CONFIG };
    Logger.hasBackedUpOnStartup = false;
  }

  /**
   * Initializes the logger. This should be called once when the application starts.
   * It applies any initial configuration and performs a startup backup if enabled.
   * @param config Optional initial configuration to apply.
   */
  static async initialize(config?: Partial<LoggerConfig>): Promise<void> {
    if (config) {
      Logger.updateConfig(config);
    }

    if (globalLoggerConfig.enableFileLogging) {
      await Logger.performStartupBackup();
      console.log('📁 File logging enabled');
    }

    console.log('🚀 Logger initialized with config:', globalLoggerConfig);
  }

  /**
   * Performs the log file backup on startup, if enabled and not already done.
   * @private
   */
  private static async performStartupBackup(): Promise<void> {
    if (
      !globalLoggerConfig.autoBackupOnStartup ||
      Logger.hasBackedUpOnStartup
    ) {
      return;
    }

    try {
      const backupPath = await logFileManager.backupCurrentLog();
      console.log(`📄 Log backup created at startup: ${backupPath}`);
      Logger.hasBackedUpOnStartup = true;
    } catch (error) {
      // A failure to backup should not prevent the logger from initializing.
      console.warn('⚠️ Failed to create startup backup:', error);
    }
  }

  /**
   * Checks if a log message at a given level should be recorded,
   * based on the current global log level.
   * @param level The level of the message to check.
   * @returns True if the message should be logged, false otherwise.
   * @private
   */
  private static shouldLog(level: string): boolean {
    const levels = ['trace', 'debug', 'info', 'warn', 'error'];
    const currentLevelIndex = levels.indexOf(globalLoggerConfig.logLevel);
    const messageLevelIndex = levels.indexOf(level);
    return messageLevelIndex >= currentLevelIndex;
  }

  /**
   * Formats a log message and its arguments, and extracts a context.
   * If the last argument is a string, it is treated as the context.
   * Other arguments are stringified and appended to the message.
   * @param message The main log message.
   * @param args The array of arguments to log.
   * @param defaultContext The default context to use if none is provided in the arguments.
   * @returns An object containing the formatted message and the context.
   * @private
   */
  private static formatLogMessage(
    message: string,
    args: unknown[],
    defaultContext: string,
  ): { formattedMessage: string; context: string } {
    let actualContext = defaultContext;
    let logMessage = message;
    let logArgs = [...args];

    // Check if the last argument is a context string, and if so, use it as the context.
    if (logArgs.length > 0 && typeof logArgs[logArgs.length - 1] === 'string') {
      actualContext = logArgs.pop() as string;
    }

    // Format the message and remaining arguments
    if (logArgs.length > 0) {
      const formattedArgs = logArgs.map((arg) => {
        if (typeof arg === 'object' && arg !== null) {
          return JSON.stringify(arg);
        }
        return String(arg);
      });
      logMessage = `${logMessage} ${formattedArgs.join(' ')}`;
    }

    // Truncate message if it exceeds the maximum length
    if (logMessage.length > globalLoggerConfig.maxMessageLength) {
      logMessage =
        logMessage.substring(0, globalLoggerConfig.maxMessageLength - 3) +
        '...';
    }

    return { formattedMessage: logMessage, context: actualContext };
  }

  /**
   * Logs a debug message.
   * @param message The message to log.
   * @param args Additional arguments to log. If the last argument is a string, it will be used as the context.
   */
  static async debug(message: string, ...args: unknown[]): Promise<void> {
    if (!Logger.shouldLog('debug')) return;

    if (globalLoggerConfig.enableFileLogging) {
      await Logger.performStartupBackup();
    }

    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await debug(`[${context}] ${formattedMessage}`);
  }

  /**
   * Logs an info message.
   * @param message The message to log.
   * @param args Additional arguments to log. If the last argument is a string, it will be used as the context.
   */
  static async info(message: string, ...args: unknown[]): Promise<void> {
    if (!Logger.shouldLog('info')) return;

    if (globalLoggerConfig.enableFileLogging) {
      await Logger.performStartupBackup();
    }

    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await info(`[${context}] ${formattedMessage}`);
  }

  /**
   * Logs a warning message.
   * @param message The message to log.
   * @param args Additional arguments to log. If the last argument is a string, it will be used as the context.
   */
  static async warn(message: string, ...args: unknown[]): Promise<void> {
    if (!Logger.shouldLog('warn')) return;

    if (globalLoggerConfig.enableFileLogging) {
      await Logger.performStartupBackup();
    }

    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await warn(`[${context}] ${formattedMessage}`);
  }

  /**
   * Logs an error message. If the last argument is an `Error` object, its message will be appended.
   * @param message The message to log.
   * @param args Additional arguments to log. If the last argument is a string, it will be used as the context.
   */
  static async error(message: string, ...args: unknown[]): Promise<void> {
    if (!Logger.shouldLog('error')) return;

    if (globalLoggerConfig.enableFileLogging) {
      await Logger.performStartupBackup();
    }

    let errorObj: Error | undefined;
    let remainingArgs = [...args];

    // Check if the last argument is an Error object
    if (
      remainingArgs.length > 0 &&
      remainingArgs[remainingArgs.length - 1] instanceof Error
    ) {
      const popped = remainingArgs.pop();
      if (popped instanceof Error) {
        errorObj = popped;
      }
    }

    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      remainingArgs,
      Logger.defaultContext,
    );
    const errorMsg = errorObj
      ? `${formattedMessage}: ${errorObj.message}`
      : formattedMessage;
    await logError(`[${context}] ${errorMsg}`);
  }

  /**
   * Logs a trace message.
   * @param message The message to log.
   * @param args Additional arguments to log. If the last argument is a string, it will be used as the context.
   */
  static async trace(message: string, ...args: unknown[]): Promise<void> {
    if (!Logger.shouldLog('trace')) return;

    if (globalLoggerConfig.enableFileLogging) {
      await Logger.performStartupBackup();
    }

    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await trace(`[${context}] ${formattedMessage}`);
  }
}

/**
 * A convenience object that provides global access to the static `Logger` methods.
 * This allows for simple, context-less logging from anywhere in the application.
 */
export const log = {
  debug: (message: string, ...args: unknown[]) =>
    Logger.debug(message, ...args),
  info: (message: string, ...args: unknown[]) => Logger.info(message, ...args),
  warn: (message: string, ...args: unknown[]) => Logger.warn(message, ...args),
  error: (message: string, ...args: unknown[]) =>
    Logger.error(message, ...args),
  trace: (message: string, ...args: unknown[]) =>
    Logger.trace(message, ...args),
};

/**
 * Creates a logger instance with a specific context.
 * This is the recommended way to log from within specific components or modules.
 * @param contextName The name of the context to use for this logger instance.
 * @returns An object with logging methods (`debug`, `info`, `warn`, `error`, `trace`) that will automatically include the context.
 */
export function getLogger(contextName: string) {
  return {
    debug: (message: string, ...args: unknown[]) =>
      Logger.debug(message, ...args, contextName),
    info: (message: string, ...args: unknown[]) =>
      Logger.info(message, ...args, contextName),
    warn: (message: string, ...args: unknown[]) =>
      Logger.warn(message, ...args, contextName),
    error: (message: string, ...args: unknown[]) =>
      Logger.error(message, ...args, contextName),
    trace: (message: string, ...args: unknown[]) =>
      Logger.trace(message, ...args, contextName),
  };
}

/**
 * A collection of utility functions for managing the logger's configuration and log files.
 */
export const logUtils = {
  /**
   * Initializes the logger, loading any saved configuration from local storage
   * and applying any provided initial configuration. This should be called once on app startup.
   * @param config Optional initial configuration to apply.
   */
  initialize: async (config?: Partial<LoggerConfig>): Promise<void> => {
    // First, try to load any saved configuration.
    try {
      const savedConfig = await logUtils.loadConfig();
      if (savedConfig) {
        Logger.updateConfig(savedConfig);
      }
    } catch (error) {
      console.warn('Failed to load saved logger config:', error);
    }

    // If an initial config is provided, overwrite the loaded config with it.
    if (config) {
      Logger.updateConfig(config);
      // Save the new configuration.
      await logUtils.saveConfig();
    }

    await Logger.initialize();
  },

  /**
   * Updates the logger configuration and saves it to persistent storage.
   * @param config A partial `LoggerConfig` object with the settings to update.
   */
  updateConfig: async (config: Partial<LoggerConfig>): Promise<void> => {
    Logger.updateConfig(config);
    await logUtils.saveConfig();
  },

  /**
   * Gets the current logger configuration.
   * @returns The current `LoggerConfig`.
   */
  getConfig: (): LoggerConfig => {
    return Logger.getConfig();
  },

  /**
   * Resets the logger configuration to its default values and saves the change.
   */
  resetConfig: async (): Promise<void> => {
    Logger.resetConfig();
    await logUtils.saveConfig();
  },

  /**
   * Saves the current logger configuration to local storage.
   */
  saveConfig: async (): Promise<void> => {
    try {
      const config = Logger.getConfig();
      localStorage.setItem(
        'libragent-logger-config',
        JSON.stringify(config),
      );
    } catch (error) {
      console.error('Failed to save logger config:', error);
    }
  },

  /**
   * Loads the logger configuration from local storage.
   * @returns A promise that resolves to the loaded `LoggerConfig`, or null if not found or on error.
   */
  loadConfig: async (): Promise<LoggerConfig | null> => {
    try {
      const configStr = localStorage.getItem('libragent-logger-config');
      if (configStr) {
        return JSON.parse(configStr) as LoggerConfig;
      }
    } catch (error) {
      console.error('Failed to load logger config:', error);
    }
    return null;
  },

  /**
   * Manually triggers a backup of the current log file.
   * @returns A promise that resolves to the path of the new backup file.
   */
  backupNow: async (): Promise<string> => {
    return await logFileManager.backupCurrentLog();
  },

  /**
   * Clears the content of the current log file.
   */
  clearLogs: async (): Promise<void> => {
    await logFileManager.clearCurrentLog();
  },

  /**
   * Gets the path to the log directory.
   * @returns A promise that resolves to the log directory path.
   */
  getLogDirectory: async (): Promise<string> => {
    return await logFileManager.getLogDirectory();
  },

  /**
   * Lists all log files in the log directory.
   * @returns A promise that resolves to an array of log file names.
   */
  listAllLogFiles: async (): Promise<string[]> => {
    return await logFileManager.listLogFiles();
  },

  /**
   * A convenience function to set the log level.
   * @param level The log level to set.
   */
  setLogLevel: async (level: LoggerConfig['logLevel']): Promise<void> => {
    await logUtils.updateConfig({ logLevel: level });
  },

  /**
   * A convenience function to enable or disable file logging.
   * @param enabled If true, file logging is enabled. Defaults to true.
   */
  enableFileLogging: async (enabled: boolean = true): Promise<void> => {
    await logUtils.updateConfig({ enableFileLogging: enabled });
  },

  /**
   * A convenience function to enable or disable automatic log backup on startup.
   * @param enabled If true, auto backup is enabled. Defaults to true.
   */
  enableAutoBackup: async (enabled: boolean = true): Promise<void> => {
    await logUtils.updateConfig({ autoBackupOnStartup: enabled });
  },

  /**
   * A convenience function to set the maximum message length for log messages.
   * Messages longer than this limit will be truncated with "...".
   * @param length The maximum number of characters allowed in a log message.
   */
  setMaxMessageLength: async (length: number): Promise<void> => {
    await logUtils.updateConfig({ maxMessageLength: length });
  },
};
