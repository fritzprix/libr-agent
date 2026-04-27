import {
  getLoggerConfig,
  resetLoggerConfig,
  updateLoggerConfig,
} from './logger-config';
import { logFileManager } from './logger-file-manager';
import {
  debug as queueDebug,
  error as queueError,
  info as queueInfo,
  trace as queueTrace,
  warn as queueWarn,
} from './logger-queue';
import { LogLevel, type LoggerConfig } from './logger-types';

const LOG_LEVEL_ORDER: LoggerConfig['logLevel'][] = [
  'trace',
  'debug',
  'info',
  'warn',
  'error',
];

function shouldLog(level: LoggerConfig['logLevel']): boolean {
  const config = getLoggerConfig();
  return (
    LOG_LEVEL_ORDER.indexOf(level) >= LOG_LEVEL_ORDER.indexOf(config.logLevel)
  );
}

function formatLogMessage(
  message: string,
  args: unknown[],
  defaultContext: string,
): { formattedMessage: string; context: string } {
  const config = getLoggerConfig();
  let actualContext = defaultContext;
  let logMessage = message;
  const logArgs = [...args];

  if (logArgs.length > 0 && typeof logArgs[logArgs.length - 1] === 'string') {
    actualContext = logArgs.pop() as string;
  }

  if (logArgs.length > 0) {
    const formattedArgs = logArgs.map((arg) => {
      if (arg instanceof Error) {
        return `${arg.name}: ${arg.message}`;
      }
      if (typeof arg === 'object' && arg !== null) {
        try {
          return JSON.stringify(arg);
        } catch {
          return String(arg);
        }
      }
      return String(arg);
    });
    logMessage = `${logMessage} ${formattedArgs.join(' ')}`;
  }

  if (logMessage.length > config.maxMessageLength) {
    logMessage = `${logMessage.substring(0, config.maxMessageLength - 3)}...`;
  }

  return { formattedMessage: logMessage, context: actualContext };
}

async function ensureStartupBackup(
  hasBackedUpOnStartup: boolean,
): Promise<boolean> {
  const config = getLoggerConfig();
  if (
    !config.enableFileLogging ||
    !config.autoBackupOnStartup ||
    hasBackedUpOnStartup
  ) {
    return hasBackedUpOnStartup;
  }

  try {
    await logFileManager.backupCurrentLog();
    return true;
  } catch (error) {
    console.warn('⚠️ Failed to create startup backup:', error);
    return hasBackedUpOnStartup;
  }
}

export class Logger {
  private static defaultContext = 'TauriAgent';
  private static hasBackedUpOnStartup = false;

  static updateConfig(config: Partial<LoggerConfig>): void {
    updateLoggerConfig(config);
  }

  static getConfig(): LoggerConfig {
    return getLoggerConfig();
  }

  static resetConfig(): void {
    resetLoggerConfig();
    Logger.hasBackedUpOnStartup = false;
  }

  static async initialize(config?: Partial<LoggerConfig>): Promise<void> {
    if (config) {
      Logger.updateConfig(config);
    }

    Logger.hasBackedUpOnStartup = await ensureStartupBackup(
      Logger.hasBackedUpOnStartup,
    );
  }

  private static async logWithLevel(
    level: LoggerConfig['logLevel'],
    writer: (message: string) => void,
    message: string,
    args: unknown[],
  ): Promise<void> {
    if (!shouldLog(level)) {
      return;
    }

    Logger.hasBackedUpOnStartup = await ensureStartupBackup(
      Logger.hasBackedUpOnStartup,
    );

    const { formattedMessage, context } = formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    writer(`[${context}] ${formattedMessage}`);
  }

  static async debug(message: string, ...args: unknown[]): Promise<void> {
    await Logger.logWithLevel(LogLevel.Debug, queueDebug, message, args);
  }

  static async info(message: string, ...args: unknown[]): Promise<void> {
    await Logger.logWithLevel(LogLevel.Info, queueInfo, message, args);
  }

  static async warn(message: string, ...args: unknown[]): Promise<void> {
    await Logger.logWithLevel(LogLevel.Warn, queueWarn, message, args);
  }

  static async trace(message: string, ...args: unknown[]): Promise<void> {
    await Logger.logWithLevel(LogLevel.Trace, queueTrace, message, args);
  }

  static async error(message: string, ...args: unknown[]): Promise<void> {
    if (!shouldLog(LogLevel.Error)) {
      return;
    }

    Logger.hasBackedUpOnStartup = await ensureStartupBackup(
      Logger.hasBackedUpOnStartup,
    );

    let errorObj: Error | undefined;
    const remainingArgs = [...args];

    if (
      remainingArgs.length > 0 &&
      remainingArgs[remainingArgs.length - 1] instanceof Error
    ) {
      const popped = remainingArgs.pop();
      if (popped instanceof Error) {
        errorObj = popped;
      }
    }

    const { formattedMessage, context } = formatLogMessage(
      message,
      remainingArgs,
      Logger.defaultContext,
    );
    const errorMessage = errorObj
      ? `${formattedMessage}: ${errorObj.message}`
      : formattedMessage;

    queueError(`[${context}] ${errorMessage}`);
  }
}

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
