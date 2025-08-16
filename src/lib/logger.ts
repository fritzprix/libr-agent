/**
 * SynapticFlow 글로벌 로거 시스템
 *
 * 특징:
 * - 파일 로깅 자동 지원 (플랫폼별 표준 경로)
 * - 시작 시 자동 백업
 * - 로그 레벨 필터링
 * - 설정 영구 저장
 * - 컨텍스트별 로깅
 *
 * @example
 * ```typescript
 * import { getLogger, logUtils } from '@/lib/logger';
 *
 * // 앱 시작 시 (main.tsx에서 자동 호출됨)
 * await logUtils.initialize();
 *
 * // 컨텍스트별 로거 사용
 * const logger = getLogger('MyComponent');
 * logger.info('Component initialized');
 *
 * // 설정 변경
 * await logUtils.setLogLevel('debug');
 * await logUtils.enableFileLogging(true);
 *
 * // 로그 파일 관리
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

// 글로벌 로거 설정 인터페이스
export interface LoggerConfig {
  enableFileLogging: boolean;
  autoBackupOnStartup: boolean;
  maxBackupFiles: number;
  logLevel: 'trace' | 'debug' | 'info' | 'warn' | 'error';
}

// 기본 설정
const DEFAULT_CONFIG: LoggerConfig = {
  enableFileLogging: true,
  autoBackupOnStartup: true,
  maxBackupFiles: 10,
  logLevel: 'info',
};

// 글로벌 설정 저장소
let globalLoggerConfig: LoggerConfig = { ...DEFAULT_CONFIG };

// 로그 파일 관리 인터페이스
export interface LogFileManager {
  getLogDirectory(): Promise<string>;
  backupCurrentLog(): Promise<string>;
  clearCurrentLog(): Promise<void>;
  listLogFiles(): Promise<string[]>;
}

// 로그 파일 관리 클래스 구현
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

// 전역 로그 파일 매니저 인스턴스
export const logFileManager = new TauriLogFileManager();

export class Logger {
  private static defaultContext = 'TauriAgent';
  private static hasBackedUpOnStartup = false;

  // 글로벌 로거 설정 업데이트
  static updateConfig(config: Partial<LoggerConfig>): void {
    globalLoggerConfig = { ...globalLoggerConfig, ...config };
    console.log('Logger config updated:', globalLoggerConfig);
  }

  // 현재 설정 반환
  static getConfig(): LoggerConfig {
    return { ...globalLoggerConfig };
  }

  // 설정을 기본값으로 리셋
  static resetConfig(): void {
    globalLoggerConfig = { ...DEFAULT_CONFIG };
    Logger.hasBackedUpOnStartup = false;
  }

  // 로거 초기화 (앱 시작 시 호출)
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

  // 시작 시 한 번만 백업 수행
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
      // 백업 실패는 로깅을 방해하지 않음
      console.warn('⚠️ Failed to create startup backup:', error);
    }
  }

  // 로그 레벨 체크
  private static shouldLog(level: string): boolean {
    const levels = ['trace', 'debug', 'info', 'warn', 'error'];
    const currentLevelIndex = levels.indexOf(globalLoggerConfig.logLevel);
    const messageLevelIndex = levels.indexOf(level);
    return messageLevelIndex >= currentLevelIndex;
  }

  private static formatLogMessage(
    message: string,
    args: unknown[],
    defaultContext: string,
  ): { formattedMessage: string; context: string } {
    let actualContext = defaultContext;
    let logMessage = message;
    let logArgs = [...args];

    // Check if the last argument is a context string
    if (logArgs.length > 0 && typeof logArgs[logArgs.length - 1] === 'string') {
      actualContext = logArgs.pop() as string; // Remove and use as context
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
    return { formattedMessage: logMessage, context: actualContext };
  }

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

// Convenience functions for common logging patterns (global logger)
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

// Function to get a context-specific logger instance
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

// 로그 파일 관리를 위한 유틸리티 함수들
export const logUtils = {
  // 로거 초기화 (앱 시작 시 호출)
  initialize: async (config?: Partial<LoggerConfig>): Promise<void> => {
    // 먼저 저장된 설정을 로드 시도
    try {
      const savedConfig = await logUtils.loadConfig();
      if (savedConfig) {
        Logger.updateConfig(savedConfig);
      }
    } catch (error) {
      console.warn('Failed to load saved logger config:', error);
    }

    // 전달된 설정이 있으면 덮어쓰기
    if (config) {
      Logger.updateConfig(config);
      // 새 설정 저장
      await logUtils.saveConfig();
    }

    await Logger.initialize();
  },

  // 설정 업데이트 및 저장
  updateConfig: async (config: Partial<LoggerConfig>): Promise<void> => {
    Logger.updateConfig(config);
    await logUtils.saveConfig();
  },

  // 현재 설정 가져오기
  getConfig: (): LoggerConfig => {
    return Logger.getConfig();
  },

  // 설정 리셋
  resetConfig: async (): Promise<void> => {
    Logger.resetConfig();
    await logUtils.saveConfig();
  },

  // 설정을 로컬 스토리지에 저장
  saveConfig: async (): Promise<void> => {
    try {
      const config = Logger.getConfig();
      localStorage.setItem(
        'synaptic-flow-logger-config',
        JSON.stringify(config),
      );
    } catch (error) {
      console.error('Failed to save logger config:', error);
    }
  },

  // 로컬 스토리지에서 설정 로드
  loadConfig: async (): Promise<LoggerConfig | null> => {
    try {
      const configStr = localStorage.getItem('synaptic-flow-logger-config');
      if (configStr) {
        return JSON.parse(configStr) as LoggerConfig;
      }
    } catch (error) {
      console.error('Failed to load logger config:', error);
    }
    return null;
  },

  // 수동으로 현재 로그 백업
  backupNow: async (): Promise<string> => {
    return await logFileManager.backupCurrentLog();
  },

  // 현재 로그 파일 초기화
  clearLogs: async (): Promise<void> => {
    await logFileManager.clearCurrentLog();
  },

  // 로그 디렉토리 경로 가져오기
  getLogDirectory: async (): Promise<string> => {
    return await logFileManager.getLogDirectory();
  },

  // 모든 로그 파일 목록 가져오기
  listAllLogFiles: async (): Promise<string[]> => {
    return await logFileManager.listLogFiles();
  },

  // 로그 레벨별 편의 함수들
  setLogLevel: async (level: LoggerConfig['logLevel']): Promise<void> => {
    await logUtils.updateConfig({ logLevel: level });
  },

  enableFileLogging: async (enabled: boolean = true): Promise<void> => {
    await logUtils.updateConfig({ enableFileLogging: enabled });
  },

  enableAutoBackup: async (enabled: boolean = true): Promise<void> => {
    await logUtils.updateConfig({ autoBackupOnStartup: enabled });
  },
};
